//! Privatim data canister
//!
//! Holds the bank's CRM core: clients, portfolios, meetings, trade ideas.
//! Authz is delegated to the `identity` canister; audit logging is delegated
//! to the `audit` canister. This canister knows nothing about roles or
//! hash chains directly — it just calls out.
//!
//! Pattern:
//!
//! - Read paths are **composite queries** that ask `identity.has_role` /
//!   `identity.assigned_clients` to scope visibility before returning data.
//!   ~100ms typical inter-canister latency.
//! - Write paths are regular updates. Each one:
//!     1. inter-canister query to `identity` for authz,
//!     2. local mutation,
//!     3. inter-canister update to `audit` to log the action.
//!
//! Configuration: principals of the identity and audit canisters are set
//! by controllers via `set_identity_canister` / `set_audit_canister` after
//! deploy (or auto-injected by the Cloud Engines installer via the
//! `PUBLIC_CANISTER_ID:identity` / `PUBLIC_CANISTER_ID:audit` env vars,
//! which we read in `init`).

use candid::{CandidType, Principal};
use ic_cdk::api::msg_caller as caller;
use ic_cdk::api::time;
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;

const MAX_TEXT_LEN: usize = 8_000;
const MAX_SHORT_LEN: usize = 200;

// ───────────────────── domain types ─────────────────────

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    Advisor,
    Compliance,
    Admin,
}

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClientType {
    Individual,
    Family,
    Corporate,
}

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum KycStatus {
    Pending,
    Approved,
    Expired,
}

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskProfile {
    Conservative,
    Balanced,
    Growth,
    Speculative,
}

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssetClass {
    Equity,
    FixedIncome,
    Cash,
    Fx,
    Commodity,
}

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum TradeIdeaStatus {
    Draft,
    Approved,
    Rejected,
    Executed,
}

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccessPurpose {
    ManualReview,
    TradeIdeaPreparation,
    AssistantQuery,
    ComplianceReview,
    KycRefresh,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct Position {
    pub ticker: String,
    pub asset_class: AssetClass,
    pub quantity: u64,
    pub avg_cost_chf_cents: u64,
    pub current_price_chf_cents: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct Portfolio {
    pub id: u64,
    pub client_id: u64,
    pub name: String,
    pub base_currency: String,
    pub positions: Vec<Position>,
    pub cash_chf_cents: i64,
    pub last_valued_at_ns: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct Client {
    pub id: u64,
    pub display_name: String,
    pub legal_name: String,
    pub client_type: ClientType,
    pub tax_residency: String,
    pub primary_advisor: Principal,
    pub kyc_status: KycStatus,
    pub kyc_expires_ns: u64,
    pub risk_profile: RiskProfile,
    pub aum_chf: u64,
    pub created_at_ns: u64,
    pub portfolio_ids: Vec<u64>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct Meeting {
    pub id: u64,
    pub client_id: u64,
    pub advisor: Principal,
    pub occurred_at_ns: u64,
    pub title: String,
    pub notes_md: String,
    pub decisions: Vec<String>,
    pub follow_ups: Vec<String>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct TradeIdea {
    pub id: u64,
    pub client_id: u64,
    pub portfolio_id: Option<u64>,
    pub proposed_by: Principal,
    pub proposed_at_ns: u64,
    pub title: String,
    pub rationale: String,
    pub status: TradeIdeaStatus,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum DataError {
    Unauthorized,
    NotFound,
    InvalidArgument(String),
    IdentityCanisterNotConfigured,
    AuditCanisterNotConfigured,
    UpstreamFailed(String),
}

pub type DataResult<T> = Result<T, DataError>;

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct CreateClientArgs {
    pub display_name: String,
    pub legal_name: String,
    pub client_type: ClientType,
    pub tax_residency: String,
    pub risk_profile: RiskProfile,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AddMeetingArgs {
    pub client_id: u64,
    pub title: String,
    pub notes_md: String,
    pub decisions: Vec<String>,
    pub follow_ups: Vec<String>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AddTradeIdeaArgs {
    pub client_id: u64,
    pub portfolio_id: Option<u64>,
    pub title: String,
    pub rationale: String,
}

// ───────────────────── audit-action mirror ─────────────────────
//
// Mirrors the AuditAction variant in the audit canister. Kept in lockstep
// when audit's surface changes. Only the variants this canister emits are
// listed.

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum AuditAction {
    ClientCreated { client_id: u64 },
    ClientKycUpdated { client_id: u64, status: KycStatus },
    ClientReassigned { client_id: u64, to: Principal },
    MeetingAdded { client_id: u64, meeting_id: u64 },
    TradeIdeaProposed { client_id: u64, trade_idea_id: u64 },
    TradeIdeaStatusChanged { trade_idea_id: u64, status: TradeIdeaStatus },
    ClientAccessed { client_id: u64, purpose: AccessPurpose },
}

// ───────────────────── state ─────────────────────

#[derive(Default, CandidType, Serialize, Deserialize)]
struct State {
    next_client_id: u64,
    next_portfolio_id: u64,
    next_meeting_id: u64,
    next_trade_idea_id: u64,

    clients: BTreeMap<u64, Client>,
    portfolios: BTreeMap<u64, Portfolio>,
    meetings: BTreeMap<u64, Meeting>,
    trade_ideas: BTreeMap<u64, TradeIdea>,

    identity_canister: Option<Principal>,
    audit_canister: Option<Principal>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn one_day_ns() -> u64 {
    86_400 * 1_000_000_000
}

// ───────────────────── helpers ─────────────────────

fn assert_authenticated() -> DataResult<Principal> {
    let p = caller();
    if p == Principal::anonymous() {
        return Err(DataError::Unauthorized);
    }
    Ok(p)
}

fn get_identity() -> DataResult<Principal> {
    STATE
        .with(|s| s.borrow().identity_canister)
        .ok_or(DataError::IdentityCanisterNotConfigured)
}

fn get_audit() -> DataResult<Principal> {
    STATE
        .with(|s| s.borrow().audit_canister)
        .ok_or(DataError::AuditCanisterNotConfigured)
}

async fn has_role(p: Principal, role: Role) -> DataResult<bool> {
    if ic_cdk::api::is_controller(&p) {
        return Ok(true);
    }
    let identity = get_identity()?;
    let res: (bool,) = ic_cdk::api::call::call(identity, "has_role", (p, role))
        .await
        .map_err(|e| DataError::UpstreamFailed(format!("identity.has_role: {e:?}")))?;
    Ok(res.0)
}

async fn is_assigned(advisor: Principal, client_id: u64) -> DataResult<bool> {
    let identity = get_identity()?;
    let res: (bool,) =
        ic_cdk::api::call::call(identity, "is_assigned", (advisor, client_id))
            .await
            .map_err(|e| DataError::UpstreamFailed(format!("identity.is_assigned: {e:?}")))?;
    Ok(res.0)
}

async fn assigned_clients(advisor: Principal) -> DataResult<Vec<u64>> {
    let identity = get_identity()?;
    let res: (Vec<u64>,) =
        ic_cdk::api::call::call(identity, "assigned_clients", (advisor,))
            .await
            .map_err(|e| DataError::UpstreamFailed(format!("identity.assigned_clients: {e:?}")))?;
    Ok(res.0)
}

async fn assign_client_in_identity(advisor: Principal, client_id: u64) -> DataResult<()> {
    let identity = get_identity()?;
    let _: (Result<(), candid::Reserved>,) =
        ic_cdk::api::call::call(identity, "assign_client", (advisor, client_id))
            .await
            .map_err(|e| DataError::UpstreamFailed(format!("identity.assign_client: {e:?}")))?;
    Ok(())
}

async fn record_audit(action: AuditAction, on_behalf_of: Principal) -> DataResult<()> {
    let audit = get_audit()?;
    let _: (Result<u64, candid::Reserved>,) =
        ic_cdk::api::call::call(audit, "append", (action, on_behalf_of))
            .await
            .map_err(|e| DataError::UpstreamFailed(format!("audit.append: {e:?}")))?;
    Ok(())
}

async fn can_view_client(p: Principal, client_id: u64) -> DataResult<bool> {
    if ic_cdk::api::is_controller(&p) {
        return Ok(true);
    }
    if has_role(p, Role::Compliance).await? || has_role(p, Role::Admin).await? {
        return Ok(true);
    }
    if !has_role(p, Role::Advisor).await? {
        return Ok(false);
    }
    is_assigned(p, client_id).await
}

fn validate_short(s: &str, name: &str) -> DataResult<()> {
    let t = s.trim();
    if t.is_empty() || t.len() > MAX_SHORT_LEN {
        return Err(DataError::InvalidArgument(name.into()));
    }
    Ok(())
}

fn validate_long(s: &str, name: &str) -> DataResult<()> {
    if s.len() > MAX_TEXT_LEN {
        return Err(DataError::InvalidArgument(name.into()));
    }
    Ok(())
}

// ───────────────────── seed data ─────────────────────

fn synthetic_principal(seed: u8) -> Principal {
    let mut bytes = [0u8; 29];
    bytes[0] = seed;
    bytes[28] = seed.wrapping_mul(31);
    Principal::from_slice(&bytes)
}

#[allow(clippy::too_many_arguments)]
fn seed_client(
    state: &mut State,
    advisor: Principal,
    display: &str,
    legal: &str,
    ctype: ClientType,
    tax_res: &str,
    risk: RiskProfile,
    aum_chf: u64,
    kyc: KycStatus,
    portfolios: &[(&str, &str, &[(&str, AssetClass, u64, u64, u64)])],
) {
    let now = time();
    let id = state.next_client_id;
    state.next_client_id += 1;
    let kyc_expires = match kyc {
        KycStatus::Pending => now + 30 * one_day_ns(),
        KycStatus::Approved => now + 365 * one_day_ns(),
        KycStatus::Expired => now.saturating_sub(30 * one_day_ns()),
    };
    let mut portfolio_ids = Vec::new();
    for (pname, pcurrency, positions) in portfolios {
        let pid = state.next_portfolio_id;
        state.next_portfolio_id += 1;
        let positions: Vec<Position> = positions
            .iter()
            .map(|(t, c, q, avg, cur)| Position {
                ticker: (*t).to_string(),
                asset_class: *c,
                quantity: *q,
                avg_cost_chf_cents: *avg,
                current_price_chf_cents: *cur,
            })
            .collect();
        state.portfolios.insert(
            pid,
            Portfolio {
                id: pid,
                client_id: id,
                name: (*pname).to_string(),
                base_currency: (*pcurrency).to_string(),
                positions,
                cash_chf_cents: 1_500_000_00,
                last_valued_at_ns: now,
            },
        );
        portfolio_ids.push(pid);
    }
    state.clients.insert(
        id,
        Client {
            id,
            display_name: display.to_string(),
            legal_name: legal.to_string(),
            client_type: ctype,
            tax_residency: tax_res.to_string(),
            primary_advisor: advisor,
            kyc_status: kyc,
            kyc_expires_ns: kyc_expires,
            risk_profile: risk,
            aum_chf,
            created_at_ns: now,
            portfolio_ids,
        },
    );
}

fn seed_meeting(
    state: &mut State,
    client_id: u64,
    days_ago: u64,
    title: &str,
    notes: &str,
    decisions: &[&str],
    follow_ups: &[&str],
) {
    let advisor = state
        .clients
        .get(&client_id)
        .map(|c| c.primary_advisor)
        .unwrap_or(Principal::anonymous());
    let mid = state.next_meeting_id;
    state.next_meeting_id += 1;
    state.meetings.insert(
        mid,
        Meeting {
            id: mid,
            client_id,
            advisor,
            occurred_at_ns: time().saturating_sub(days_ago * one_day_ns()),
            title: title.to_string(),
            notes_md: notes.to_string(),
            decisions: decisions.iter().map(|s| (*s).to_string()).collect(),
            follow_ups: follow_ups.iter().map(|s| (*s).to_string()).collect(),
        },
    );
}

fn seed_idea(
    state: &mut State,
    client_id: u64,
    portfolio_id: Option<u64>,
    title: &str,
    rationale: &str,
    status: TradeIdeaStatus,
) {
    let advisor = state
        .clients
        .get(&client_id)
        .map(|c| c.primary_advisor)
        .unwrap_or(Principal::anonymous());
    let tid = state.next_trade_idea_id;
    state.next_trade_idea_id += 1;
    state.trade_ideas.insert(
        tid,
        TradeIdea {
            id: tid,
            client_id,
            portfolio_id,
            proposed_by: advisor,
            proposed_at_ns: time().saturating_sub(7 * one_day_ns()),
            title: title.to_string(),
            rationale: rationale.to_string(),
            status,
        },
    );
}

// ───────────────────── procedural-data generator ─────────────────────
//
// A tiny LCG seeded from a fixed constant so every fresh install gets
// the same procedurally-generated workspace. Adds 12 more clients on top
// of the hand-crafted 12, plus a sparse layer of random meetings and
// trade ideas. Total population after seed: ~24 clients, ~40 meetings,
// ~30 trade ideas, with realistic variance — some clients have lots of
// activity, some are nearly empty.

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn pct(&mut self) -> u8 {
        (self.next() % 100) as u8
    }
    fn pick<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        &slice[(self.next() % slice.len() as u64) as usize]
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

const FIRST_DE: &[&str] = &[
    "Hans", "Werner", "Beat", "Markus", "Daniel", "Stefan", "Andreas", "Christian",
    "Heinrich", "Walter", "Anna", "Maria", "Christine", "Claudia", "Sabine",
    "Petra", "Heidi", "Ursula",
];
const FIRST_FR: &[&str] = &[
    "Pierre", "Jean", "François", "André", "Marc", "Patrick", "Olivier", "Bernard",
    "Sophie", "Catherine", "Martine", "Isabelle", "Nathalie", "Véronique",
];
const FIRST_IT: &[&str] = &[
    "Marco", "Luca", "Gianni", "Alessandro", "Fabio", "Stefano", "Roberto",
    "Giulia", "Francesca", "Valentina",
];
const LAST_DE: &[&str] = &[
    "Müller", "Meier", "Schmid", "Keller", "Weber", "Huber", "Wyss", "Steiner",
    "Frei", "Brunner", "Berger", "Studer", "Käppeli", "Schlegel",
];
const LAST_FR: &[&str] = &[
    "Dubois", "Martin", "Bernard", "Petit", "Moreau", "Roux", "Lambert", "Vincent",
    "Bonnard", "Chappuis",
];
const LAST_IT: &[&str] = &[
    "Rossi", "Ferrari", "Bianchi", "Romano", "Conti", "Russo", "Esposito",
    "Greco", "Marini",
];

const CORPORATE_SUFFIXES: &[&str] = &[
    "AG", "SA", "GmbH", "SARL", "Holding AG", "Treuhand AG", "Industries SA",
    "Capital SA", "Invest AG", "Group SA",
];
const FAMILY_SUFFIXES: &[&str] = &[
    "Family", "Familie", "Famille", "Famiglia", "Trust", "Estate",
];

const RESIDENCIES: &[&str] = &[
    "CH-ZH", "CH-GE", "CH-VD", "CH-ZG", "CH-BS", "CH-BE", "CH-LU", "CH-SG",
    "CH-TI", "CH-AG",
];

const PORTFOLIO_NAMES: &[&str] = &[
    "Discretionary Mandate",
    "Treasury Liquidity",
    "Pension Reserve",
    "Income Account",
    "Growth Account",
    "Macro Hedge",
    "Family Office",
    "Stable Income",
    "Tactical Equity",
    "USD Discretionary",
    "EUR Balanced",
    "Capital Preservation",
];

const CURRENCIES: &[&str] = &["CHF", "USD", "EUR"];

#[allow(clippy::type_complexity)]
const TICKER_CATALOG: &[(&str, AssetClass, u64, u64)] = &[
    // (ticker, class, avg_cost_chf_cents/unit, current_price_chf_cents/unit)
    ("NESN.SW", AssetClass::Equity, 9_500_00, 10_220_00),
    ("NOVN.SW", AssetClass::Equity, 8_700_00, 9_350_00),
    ("ROG.SW", AssetClass::Equity, 24_500_00, 23_900_00),
    ("UBSG.SW", AssetClass::Equity, 25_30, 27_60),
    ("ABBN.SW", AssetClass::Equity, 38_90, 47_20),
    ("ZURN.SW", AssetClass::Equity, 480_50, 512_30),
    ("CSGN.SW", AssetClass::Equity, 12_40, 8_90),
    ("HOLN.SW", AssetClass::Equity, 64_20, 71_80),
    ("AAPL", AssetClass::Equity, 165_00, 198_50),
    ("MSFT", AssetClass::Equity, 320_00, 410_75),
    ("AMZN", AssetClass::Equity, 145_00, 178_30),
    ("GOOGL", AssetClass::Equity, 138_00, 165_40),
    ("TSLA", AssetClass::Equity, 225_00, 192_60),
    ("BUND-10Y", AssetClass::FixedIncome, 9_900_00, 9_785_00),
    ("CH-CONFED-30Y", AssetClass::FixedIncome, 9_700_00, 9_640_00),
    ("US-T-10Y", AssetClass::FixedIncome, 9_850_00, 9_710_00),
    ("EUR-BUND-5Y", AssetClass::FixedIncome, 9_950_00, 9_880_00),
    ("XAU/CHF", AssetClass::Commodity, 6_300_00, 6_580_00),
    ("WTI/CHF", AssetClass::Commodity, 7_200_00, 7_810_00),
];

const MEETING_TITLES: &[&str] = &[
    "Quarterly portfolio review",
    "Annual KYC refresh",
    "Strategy and rebalancing call",
    "Mandate update conversation",
    "Year-end tax planning",
    "Family office governance review",
    "Liquidity and cashflow planning",
    "Risk-profile reaffirmation",
    "Investment committee follow-up",
    "Custody and reporting briefing",
];

const MEETING_NOTES: &[&str] = &[
    "Reviewed YTD performance against benchmark. Portfolio tracking within tolerance; no major reallocation indicated. Discussed potential ESG screening tightening for next year.",
    "Walked through revised risk-profile questionnaire. Client confirmed preference for current mandate; updated suitability documentation accordingly.",
    "Reviewed cashflow projections for the next 12 months. Recommended building a liquidity buffer ahead of upcoming expected drawdowns. Client approved.",
    "Discussed treasurer's view on CHF rate path; agreed to lengthen duration on operating cash sleeve via Bund ladder construction.",
    "Reviewed family-office governance structure following recent reorganisation. Updated discretionary mandate to reflect new beneficial-ownership chart.",
    "Mark-to-market vs. declared AUM showed minor variance from cash sweeps; reconciled. No action required.",
    "Discussed adding a small commodities sleeve. Reviewed storage/custody implications; deferred to next committee meeting.",
    "Annual review of advisor fee schedule. No change requested.",
    "Reviewed concentrated tech exposure. Walked through volatility tradeoff vs. growth profile. No action this quarter; reassess in 6 weeks.",
    "Compliance follow-up on outstanding KYC documentation. Identified missing UBO chart signature page; chase scheduled.",
];

const MEETING_DECISIONS: &[&str] = &[
    "Maintain current allocation",
    "Reduce equity concentration to <30% NAV",
    "Add 5% commodities sleeve",
    "Move to 5y avg duration on CHF treasury",
    "Approve EUR-hedged Bund tranche",
    "Refresh KYC for 5 years",
    "Enable ESG screening",
    "No new positions until KYC approved",
    "Increase gold allocation to 12% NAV",
    "Cap AAPL+MSFT at 25% of USD discretionary",
];

const MEETING_FOLLOW_UPS: &[&str] = &[
    "Send updated mandate for signature",
    "Schedule pension-trustee call",
    "Source futures-broker confirmation",
    "Chase signed UBO chart",
    "Prepare scenario analysis: ±10% tech exposure",
    "Send hedging cost analysis",
    "Confirm storage charges with custodian",
    "Schedule IC meeting next quarter",
    "Document risk-profile reaffirmation",
    "Update suitability questionnaire",
];

const IDEA_TITLES: &[&str] = &[
    "Reduce single-stock concentration",
    "Add CHF-hedged EUR Bund tranche",
    "Lengthen duration to 5y average",
    "Increase gold allocation",
    "Cap tech exposure at 25%",
    "Rotate banks → consumer staples",
    "Trim Roche, add Novartis",
    "Add 5% commodities futures sleeve",
    "Liquidate underperforming equity sleeve",
    "Initiate USD treasury ladder",
    "Reduce European equity weight",
    "Open small AI-thematic position",
];

const IDEA_RATIONALES: &[&str] = &[
    "Current concentration sits above the policy ceiling. Trim to bring back within band.",
    "FX hedging cost remains modest; the carry-adjusted return looks attractive vs. CHF cash.",
    "Treasurer's view on rate path supports lengthening duration; current ladder is too short.",
    "Macro hedge has been underweight; gold is the preferred instrument given the client's mandate.",
    "Concentration risk vs. stated growth-with-guardrails profile; trim to reduce tracking error.",
    "Banks have run; consumer staples offer better defensives at current valuations.",
    "Pipeline concentration risk on Roche; rotate to Novartis as a partial hedge.",
    "Per the discretionary mandate's recent expansion. Sourced through existing futures broker.",
    "Equity sleeve has materially underperformed benchmark for two quarters; harvest losses.",
    "Higher US yields make a treasury ladder a sensible income complement to existing CHF positions.",
    "EU growth picture has weakened; reduce European equity weight in favor of US.",
    "Small AI-thematic position to capture upside without breaching concentration limits.",
];

fn rand_client_name(rng: &mut Lcg, ctype: ClientType) -> (String, String) {
    let lang = rng.pct();
    let (firsts, lasts) = if lang < 50 {
        (FIRST_DE, LAST_DE)
    } else if lang < 80 {
        (FIRST_FR, LAST_FR)
    } else {
        (FIRST_IT, LAST_IT)
    };
    let last = rng.pick(lasts);
    match ctype {
        ClientType::Individual => {
            let first = rng.pick(firsts);
            (format!("{last}, {first}"), format!("{first} {last}"))
        }
        ClientType::Family => {
            let suffix = rng.pick(FAMILY_SUFFIXES);
            (format!("{last} {suffix}"), format!("{last} {suffix}"))
        }
        ClientType::Corporate => {
            let suffix = rng.pick(CORPORATE_SUFFIXES);
            (
                format!("{last} {suffix}"),
                format!("{last} {suffix} (legal entity)"),
            )
        }
    }
}

fn rand_position(rng: &mut Lcg) -> Position {
    let (ticker, class, avg, cur) = *rng.pick(TICKER_CATALOG);
    let qty = rng.range(50, 5_000);
    Position {
        ticker: ticker.into(),
        asset_class: class,
        quantity: qty,
        avg_cost_chf_cents: avg,
        current_price_chf_cents: cur,
    }
}

fn seed_random_clients(state: &mut State, rng: &mut Lcg, n: usize) {
    let now = time();
    for _ in 0..n {
        // Distribution: 50% individual, 25% family, 25% corporate.
        let ctype = match rng.pct() {
            0..=49 => ClientType::Individual,
            50..=74 => ClientType::Family,
            _ => ClientType::Corporate,
        };
        let (display, legal) = rand_client_name(rng, ctype);

        let kyc = match rng.pct() {
            0..=69 => KycStatus::Approved,
            70..=89 => KycStatus::Pending,
            _ => KycStatus::Expired,
        };
        let kyc_expires = match kyc {
            KycStatus::Pending => now + 30 * one_day_ns(),
            KycStatus::Approved => now + 365 * one_day_ns(),
            KycStatus::Expired => now.saturating_sub((rng.range(7, 90)) * one_day_ns()),
        };

        let risk = match rng.pct() {
            0..=29 => RiskProfile::Conservative,
            30..=64 => RiskProfile::Balanced,
            65..=89 => RiskProfile::Growth,
            _ => RiskProfile::Speculative,
        };

        let residency = (*rng.pick(RESIDENCIES)).to_string();
        let advisor = synthetic_principal((rng.next() % 200) as u8);
        let aum_chf = rng.range(500_000, 30_000_000);

        let n_portfolios = rng.range(1, 3) as usize;
        let mut portfolio_ids = Vec::new();
        for _ in 0..n_portfolios {
            let pid = state.next_portfolio_id;
            state.next_portfolio_id += 1;
            let pname = (*rng.pick(PORTFOLIO_NAMES)).to_string();
            let pcurr = (*rng.pick(CURRENCIES)).to_string();
            let n_positions = rng.range(2, 5) as usize;
            let positions: Vec<Position> = (0..n_positions).map(|_| rand_position(rng)).collect();
            let cash = (rng.range(50_000, 3_000_000) as i64) * 100; // CHF cents
            state.portfolios.insert(
                pid,
                Portfolio {
                    id: pid,
                    client_id: state.next_client_id,
                    name: pname,
                    base_currency: pcurr,
                    positions,
                    cash_chf_cents: cash,
                    last_valued_at_ns: now,
                },
            );
            portfolio_ids.push(pid);
        }

        let id = state.next_client_id;
        state.next_client_id += 1;
        state.clients.insert(
            id,
            Client {
                id,
                display_name: display,
                legal_name: legal,
                client_type: ctype,
                tax_residency: residency,
                primary_advisor: advisor,
                kyc_status: kyc,
                kyc_expires_ns: kyc_expires,
                risk_profile: risk,
                aum_chf,
                created_at_ns: now.saturating_sub(rng.range(0, 720) * one_day_ns()),
                portfolio_ids,
            },
        );
    }
}

fn seed_random_meetings(state: &mut State, rng: &mut Lcg, count: usize) {
    let now = time();
    let client_ids: Vec<u64> = state.clients.keys().copied().collect();
    if client_ids.is_empty() {
        return;
    }
    for _ in 0..count {
        let client_id = *rng.pick(&client_ids);
        let advisor = state
            .clients
            .get(&client_id)
            .map(|c| c.primary_advisor)
            .unwrap_or(Principal::anonymous());
        let mid = state.next_meeting_id;
        state.next_meeting_id += 1;
        let title = (*rng.pick(MEETING_TITLES)).to_string();
        let notes = (*rng.pick(MEETING_NOTES)).to_string();
        let n_dec = rng.range(0, 3) as usize;
        let n_fu = rng.range(0, 3) as usize;
        let decisions: Vec<String> = (0..n_dec)
            .map(|_| (*rng.pick(MEETING_DECISIONS)).to_string())
            .collect();
        let follow_ups: Vec<String> = (0..n_fu)
            .map(|_| (*rng.pick(MEETING_FOLLOW_UPS)).to_string())
            .collect();
        let days_ago = rng.range(1, 240);
        state.meetings.insert(
            mid,
            Meeting {
                id: mid,
                client_id,
                advisor,
                occurred_at_ns: now.saturating_sub(days_ago * one_day_ns()),
                title,
                notes_md: notes,
                decisions,
                follow_ups,
            },
        );
    }
}

fn seed_random_ideas(state: &mut State, rng: &mut Lcg, count: usize) {
    let now = time();
    let client_ids: Vec<u64> = state.clients.keys().copied().collect();
    if client_ids.is_empty() {
        return;
    }
    for _ in 0..count {
        let client_id = *rng.pick(&client_ids);
        let advisor = state
            .clients
            .get(&client_id)
            .map(|c| c.primary_advisor)
            .unwrap_or(Principal::anonymous());
        let portfolio_id = state
            .clients
            .get(&client_id)
            .and_then(|c| c.portfolio_ids.first().copied());
        let tid = state.next_trade_idea_id;
        state.next_trade_idea_id += 1;
        let title = (*rng.pick(IDEA_TITLES)).to_string();
        let rationale = (*rng.pick(IDEA_RATIONALES)).to_string();
        let status = match rng.pct() {
            0..=39 => TradeIdeaStatus::Draft,
            40..=69 => TradeIdeaStatus::Approved,
            70..=89 => TradeIdeaStatus::Executed,
            _ => TradeIdeaStatus::Rejected,
        };
        let days_ago = rng.range(1, 90);
        state.trade_ideas.insert(
            tid,
            TradeIdea {
                id: tid,
                client_id,
                portfolio_id,
                proposed_by: advisor,
                proposed_at_ns: now.saturating_sub(days_ago * one_day_ns()),
                title,
                rationale,
                status,
            },
        );
    }
}

// ───────────────────── seed_demo (hand-crafted + procedural) ─────────────

fn seed_demo(state: &mut State) {
    let a1 = synthetic_principal(11);
    let a2 = synthetic_principal(22);
    let a3 = synthetic_principal(33);
    let a4 = synthetic_principal(44);
    let a5 = synthetic_principal(55);
    let a6 = synthetic_principal(66);

    let nestle = ("NESN.SW", AssetClass::Equity, 1_000, 9_500_00, 10_220_00);
    let novartis = ("NOVN.SW", AssetClass::Equity, 800, 8_700_00, 9_350_00);
    let roche = ("ROG.SW", AssetClass::Equity, 600, 24_500_00, 23_900_00);
    let ubs = ("UBSG.SW", AssetClass::Equity, 2_500, 25_30, 27_60);
    let abb = ("ABBN.SW", AssetClass::Equity, 1_500, 38_90, 47_20);
    let apple = ("AAPL", AssetClass::Equity, 400, 165_00, 198_50);
    let msft = ("MSFT", AssetClass::Equity, 250, 320_00, 410_75);
    let bund10 = ("BUND-10Y", AssetClass::FixedIncome, 5, 9_900_00, 9_785_00);
    let chconfed30 = ("CH-CONFED-30Y", AssetClass::FixedIncome, 3, 9_700_00, 9_640_00);
    let gold = ("XAU/CHF", AssetClass::Commodity, 50, 6_300_00, 6_580_00);

    seed_client(state, a1, "Müller Holdings AG", "Müller Holdings Aktiengesellschaft",
        ClientType::Corporate, "CH-ZH", RiskProfile::Balanced, 18_500_000, KycStatus::Approved,
        &[("Operating Treasury", "CHF", &[nestle, novartis, ubs, bund10, gold]),
          ("Pension Reserve", "EUR", &[chconfed30, novartis, roche])]);
    seed_client(state, a1, "von Hagen Family", "Familie von Hagen",
        ClientType::Family, "CH-ZG", RiskProfile::Conservative, 7_200_000, KycStatus::Approved,
        &[("Family Office", "CHF", &[bund10, chconfed30, gold, ubs])]);
    seed_client(state, a2, "Lombard, Rachel", "Rachel Lombard",
        ClientType::Individual, "CH-GE", RiskProfile::Growth, 2_400_000, KycStatus::Approved,
        &[("USD Discretionary", "USD", &[apple, msft, novartis]),
          ("CHF Conservative", "CHF", &[bund10, gold])]);
    seed_client(state, a2, "Tessier SA", "Tessier Société Anonyme",
        ClientType::Corporate, "CH-VD", RiskProfile::Balanced, 9_800_000, KycStatus::Pending,
        &[("Treasury Liquidity", "CHF", &[ubs, abb, bund10])]);
    seed_client(state, a3, "Romano Trust", "Romano Family Trust",
        ClientType::Family, "CH-TI", RiskProfile::Speculative, 4_100_000, KycStatus::Approved,
        &[("Aggressive Equity", "USD", &[apple, msft, abb]),
          ("CHF Hedge", "CHF", &[gold, chconfed30])]);
    seed_client(state, a3, "Bianchi, Giulia", "Giulia Bianchi",
        ClientType::Individual, "CH-TI", RiskProfile::Conservative, 1_350_000, KycStatus::Expired,
        &[("Income Account", "CHF", &[bund10, chconfed30, ubs])]);
    seed_client(state, a4, "Bachmann Industries SA", "Bachmann Industries Société Anonyme",
        ClientType::Corporate, "CH-BS", RiskProfile::Balanced, 12_700_000, KycStatus::Approved,
        &[("Operating Cash", "CHF", &[ubs, abb, bund10]),
          ("Reserve", "EUR", &[chconfed30, gold])]);
    seed_client(state, a4, "Steiner-Reber Estate", "Erbengemeinschaft Steiner-Reber",
        ClientType::Family, "CH-BE", RiskProfile::Conservative, 3_650_000, KycStatus::Approved,
        &[("Estate Trust", "CHF", &[bund10, chconfed30, gold, ubs])]);
    seed_client(state, a5, "Rösti, Markus", "Markus Rösti",
        ClientType::Individual, "CH-SG", RiskProfile::Growth, 2_900_000, KycStatus::Approved,
        &[("Growth Account", "USD", &[apple, msft, novartis, abb])]);
    seed_client(state, a5, "Reinhard Sport AG", "Reinhard Sport AG",
        ClientType::Corporate, "CH-LU", RiskProfile::Balanced, 5_800_000, KycStatus::Approved,
        &[("Treasury", "CHF", &[ubs, bund10, gold])]);
    seed_client(state, a6, "Khoury Family", "Famille Khoury",
        ClientType::Family, "CH-GE", RiskProfile::Speculative, 14_200_000, KycStatus::Approved,
        &[("Discretionary US Equity", "USD", &[apple, msft]),
          ("Macro Hedge", "EUR", &[gold, bund10]),
          ("Stable Income", "CHF", &[chconfed30, ubs, abb])]);
    seed_client(state, a6, "Albers, Cornelius", "Cornelius Albers",
        ClientType::Individual, "CH-ZH", RiskProfile::Conservative, 890_000, KycStatus::Pending,
        &[("Pension Account", "CHF", &[chconfed30, bund10])]);

    seed_meeting(state, 0, 12, "Q1 portfolio review — Müller Holdings",
        "Reviewed YTD performance against benchmark. Discussed possible reallocation away from Roche given the recent FDA setback. Müller CFO interested in exploring a CHF-hedged EUR fixed-income tranche for the pension reserve.",
        &["Reduce Roche exposure to 5% of equity sleeve", "Add CHF-hedged EUR Bund position next quarter"],
        &["Send hedging cost analysis", "Schedule pension-trustee call for May"]);
    seed_meeting(state, 1, 30, "von Hagen — annual KYC refresh",
        "Documented updated beneficial-ownership structure following the family office reorg. No change in source of funds. Family added third-generation members; advisor updated the discretionary mandate to allow ESG screening.",
        &["KYC refreshed for 5 years", "ESG screening enabled on family office portfolio"],
        &["Send updated mandate for signature"]);
    seed_meeting(state, 2, 5, "Lombard — USD growth allocation",
        "Rachel asked about increasing tech exposure on the USD account. We walked through the current 35% AAPL+MSFT concentration and the volatility tradeoff vs. her stated growth profile.",
        &["No change this quarter", "Reassess in 6 weeks"],
        &["Prepare scenario analysis: +10% tech vs. current"]);
    seed_meeting(state, 3, 18, "Tessier SA — KYC pending follow-up",
        "Treasury team produced updated UBO chart but signature page still missing. Compliance flagged the account for restricted activity until docs received.",
        &["No new positions until KYC approved"],
        &["Chase signed UBO chart by Friday"]);
    seed_meeting(state, 5, 9, "Romano Trust — speculative mandate",
        "Trustees comfortable with current 65% equity / 25% gold / 10% cash mix. Discussed adding a small commodities futures sleeve via the discretionary mandate.",
        &["Approved 5% commodities sleeve allocation"],
        &["Source futures-broker confirmation"]);
    seed_meeting(state, 7, 22, "Bachmann — CHF treasury rebalance",
        "Treasurer wants to lengthen duration on the operating cash sleeve given expected CHF rate cuts. Walked through scenarios at 3y / 5y / 7y average duration.",
        &["Move to 5y avg duration on CHF treasury"],
        &["Execute Bund ladder construction next week"]);
    seed_meeting(state, 10, 14, "Khoury — macro hedge rebalance",
        "Family principal wants to scale up the gold position from 8% to 12% of NAV. Reviewed storage and custody cost implications. Approved.",
        &["Increase gold allocation to 12% NAV"],
        &["Confirm storage charges with custodian"]);

    seed_idea(state, 0, Some(0), "Reduce Roche exposure",
        "Roche has been weak on the back of the recent oncology pipeline setback. Recommend trimming from 8% to 5% of equity sleeve and rotating into Novartis (less pipeline concentration risk).",
        TradeIdeaStatus::Approved);
    seed_idea(state, 0, Some(1), "CHF-hedged EUR Bund tranche",
        "Adding 5% of pension reserve into 10y Bund with rolling FX hedge.",
        TradeIdeaStatus::Draft);
    seed_idea(state, 2, Some(4), "Cap AAPL+MSFT at 25% of USD discretionary",
        "Current concentration is 35%. Trimming to 25% reduces tracking error vs. the client's stated growth-with-guardrails mandate.",
        TradeIdeaStatus::Draft);
    seed_idea(state, 4, None, "Add 5% commodities futures sleeve",
        "Per Romano Trust meeting, allocate 5% NAV to a commodities futures sleeve through the existing discretionary mandate broker.",
        TradeIdeaStatus::Approved);
    seed_idea(state, 6, Some(11), "Lengthen CHF Bund ladder to 5y avg",
        "Construct a 3-5-7y Bund ladder targeting 5y average duration, replacing the current 2y rolling position.",
        TradeIdeaStatus::Executed);
    seed_idea(state, 10, Some(20), "Increase Khoury gold allocation to 12% NAV",
        "Per family-principal meeting. Buy XAU/CHF in tranches over 4 weeks to reduce timing risk.",
        TradeIdeaStatus::Approved);

    // Procedural layer: ~12 more clients, ~30 meetings, ~25 trade ideas with
    // realistic variance — some clients have 0 meetings/ideas, some have
    // many. Deterministic seed so every install is reproducible.
    let mut rng = Lcg::new(0xCAFE_BABE_C0DE_F00D);
    seed_random_clients(state, &mut rng, 12);
    seed_random_meetings(state, &mut rng, 30);
    seed_random_ideas(state, &mut rng, 25);
}

// ───────────────────── lifecycle ─────────────────────

#[init]
fn init() {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        seed_demo(&mut st);
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    STATE.with(|s| {
        let bytes = candid::encode_one(&*s.borrow()).expect("encode state");
        ic_cdk::storage::stable_save((bytes,)).expect("stable_save");
    });
}

#[post_upgrade]
fn post_upgrade() {
    let (bytes,): (Vec<u8>,) =
        ic_cdk::storage::stable_restore().unwrap_or_else(|_| (Vec::new(),));
    if bytes.is_empty() {
        return;
    }
    let restored: State = candid::decode_one(&bytes).expect("decode state");
    STATE.with(|s| *s.borrow_mut() = restored);
}

// ───────────────────── config ─────────────────────

#[update]
fn set_identity_canister(p: Principal) -> DataResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(DataError::Unauthorized);
    }
    STATE.with(|s| s.borrow_mut().identity_canister = Some(p));
    Ok(())
}

#[update]
fn set_audit_canister(p: Principal) -> DataResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(DataError::Unauthorized);
    }
    STATE.with(|s| s.borrow_mut().audit_canister = Some(p));
    Ok(())
}

#[query]
fn config() -> (Option<Principal>, Option<Principal>) {
    STATE.with(|s| {
        let st = s.borrow();
        (st.identity_canister, st.audit_canister)
    })
}

#[update]
fn reset_demo() -> DataResult<u64> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(DataError::Unauthorized);
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        let identity = st.identity_canister;
        let audit = st.audit_canister;
        *st = State::default();
        st.identity_canister = identity;
        st.audit_canister = audit;
        seed_demo(&mut st);
        Ok(st.clients.len() as u64)
    })
}

// ───────────────────── public reads ─────────────────────

#[query(composite = true)]
async fn list_clients() -> Vec<Client> {
    let p = caller();
    let see_all = if ic_cdk::api::is_controller(&p) {
        true
    } else {
        let identity_opt = STATE.with(|s| s.borrow().identity_canister);
        match identity_opt {
            Some(identity) => {
                let compliance: Result<(bool,), _> =
                    ic_cdk::api::call::call(identity, "has_role", (p, Role::Compliance)).await;
                let admin: Result<(bool,), _> =
                    ic_cdk::api::call::call(identity, "has_role", (p, Role::Admin)).await;
                compliance.map(|r| r.0).unwrap_or(false)
                    || admin.map(|r| r.0).unwrap_or(false)
            }
            None => false,
        }
    };
    if see_all {
        STATE.with(|s| s.borrow().clients.values().cloned().collect())
    } else {
        let identity_opt = STATE.with(|s| s.borrow().identity_canister);
        let assigned: Vec<u64> = match identity_opt {
            Some(identity) => {
                ic_cdk::api::call::call::<_, (Vec<u64>,)>(identity, "assigned_clients", (p,))
                    .await
                    .map(|r| r.0)
                    .unwrap_or_default()
            }
            None => Vec::new(),
        };
        STATE.with(|s| {
            let st = s.borrow();
            assigned
                .into_iter()
                .filter_map(|id| st.clients.get(&id).cloned())
                .collect()
        })
    }
}

#[query(composite = true)]
async fn get_client(id: u64) -> DataResult<Client> {
    let p = caller();
    if !can_view_client(p, id).await? {
        return Err(DataError::Unauthorized);
    }
    STATE.with(|s| s.borrow().clients.get(&id).cloned().ok_or(DataError::NotFound))
}

#[query(composite = true)]
async fn get_portfolio(id: u64) -> DataResult<Portfolio> {
    let p = caller();
    let port = STATE
        .with(|s| s.borrow().portfolios.get(&id).cloned())
        .ok_or(DataError::NotFound)?;
    if !can_view_client(p, port.client_id).await? {
        return Err(DataError::Unauthorized);
    }
    Ok(port)
}

#[query(composite = true)]
async fn list_meetings(client_id: u64) -> DataResult<Vec<Meeting>> {
    let p = caller();
    if !can_view_client(p, client_id).await? {
        return Err(DataError::Unauthorized);
    }
    Ok(STATE.with(|s| {
        let st = s.borrow();
        let mut out: Vec<Meeting> = st
            .meetings
            .values()
            .filter(|m| m.client_id == client_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| b.occurred_at_ns.cmp(&a.occurred_at_ns));
        out
    }))
}

#[query(composite = true)]
async fn list_trade_ideas(client_id: u64) -> DataResult<Vec<TradeIdea>> {
    let p = caller();
    if !can_view_client(p, client_id).await? {
        return Err(DataError::Unauthorized);
    }
    Ok(STATE.with(|s| {
        let st = s.borrow();
        let mut out: Vec<TradeIdea> = st
            .trade_ideas
            .values()
            .filter(|t| t.client_id == client_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| b.proposed_at_ns.cmp(&a.proposed_at_ns));
        out
    }))
}

// ───────────────────── public writes ─────────────────────

#[update]
async fn record_client_access(client_id: u64, purpose: AccessPurpose) -> DataResult<()> {
    let p = assert_authenticated()?;
    if !can_view_client(p, client_id).await? {
        return Err(DataError::Unauthorized);
    }
    record_audit(AuditAction::ClientAccessed { client_id, purpose }, p).await
}

#[update]
async fn create_client(args: CreateClientArgs) -> DataResult<u64> {
    let p = assert_authenticated()?;
    validate_short(&args.display_name, "display_name")?;
    validate_short(&args.legal_name, "legal_name")?;
    validate_short(&args.tax_residency, "tax_residency")?;

    if !has_role(p, Role::Advisor).await? && !has_role(p, Role::Admin).await? {
        return Err(DataError::Unauthorized);
    }

    let id = STATE.with(|s| {
        let mut st = s.borrow_mut();
        let now = time();
        let id = st.next_client_id;
        st.next_client_id += 1;
        st.clients.insert(
            id,
            Client {
                id,
                display_name: args.display_name.trim().into(),
                legal_name: args.legal_name.trim().into(),
                client_type: args.client_type,
                tax_residency: args.tax_residency.trim().into(),
                primary_advisor: p,
                kyc_status: KycStatus::Pending,
                kyc_expires_ns: now + 30 * one_day_ns(),
                risk_profile: args.risk_profile,
                aum_chf: 0,
                created_at_ns: now,
                portfolio_ids: Vec::new(),
            },
        );
        id
    });

    // Best-effort: register the assignment in identity. Failure is logged
    // but doesn't break the create — admin can fix up later.
    let _ = assign_client_in_identity(p, id).await;
    record_audit(AuditAction::ClientCreated { client_id: id }, p).await?;
    Ok(id)
}

#[update]
async fn update_kyc(client_id: u64, status: KycStatus) -> DataResult<()> {
    let p = assert_authenticated()?;
    if !has_role(p, Role::Compliance).await? && !has_role(p, Role::Admin).await? {
        return Err(DataError::Unauthorized);
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        let client = st.clients.get_mut(&client_id).ok_or(DataError::NotFound)?;
        client.kyc_status = status;
        client.kyc_expires_ns = match status {
            KycStatus::Approved => time() + 365 * one_day_ns(),
            KycStatus::Pending => time() + 30 * one_day_ns(),
            KycStatus::Expired => time(),
        };
        Ok::<(), DataError>(())
    })?;
    record_audit(AuditAction::ClientKycUpdated { client_id, status }, p).await
}

#[update]
async fn assign_advisor(client_id: u64, to: Principal) -> DataResult<()> {
    let p = assert_authenticated()?;
    if !has_role(p, Role::Admin).await? {
        return Err(DataError::Unauthorized);
    }
    let prev = STATE.with(|s| {
        let mut st = s.borrow_mut();
        let client = st.clients.get_mut(&client_id).ok_or(DataError::NotFound)?;
        let prev = client.primary_advisor;
        client.primary_advisor = to;
        Ok::<Principal, DataError>(prev)
    })?;
    let _ = assign_client_in_identity(to, client_id).await;
    record_audit(AuditAction::ClientReassigned { client_id, to }, p).await?;
    let _ = prev;
    Ok(())
}

#[update]
async fn add_meeting(args: AddMeetingArgs) -> DataResult<u64> {
    let p = assert_authenticated()?;
    validate_short(&args.title, "title")?;
    validate_long(&args.notes_md, "notes_md")?;
    if !can_view_client(p, args.client_id).await? {
        return Err(DataError::Unauthorized);
    }
    let id = STATE.with(|s| {
        let mut st = s.borrow_mut();
        let id = st.next_meeting_id;
        st.next_meeting_id += 1;
        st.meetings.insert(
            id,
            Meeting {
                id,
                client_id: args.client_id,
                advisor: p,
                occurred_at_ns: time(),
                title: args.title.trim().into(),
                notes_md: args.notes_md,
                decisions: args.decisions,
                follow_ups: args.follow_ups,
            },
        );
        id
    });
    record_audit(
        AuditAction::MeetingAdded {
            client_id: args.client_id,
            meeting_id: id,
        },
        p,
    )
    .await?;
    Ok(id)
}

#[update]
async fn add_trade_idea(args: AddTradeIdeaArgs) -> DataResult<u64> {
    let p = assert_authenticated()?;
    validate_short(&args.title, "title")?;
    validate_long(&args.rationale, "rationale")?;
    if !can_view_client(p, args.client_id).await? {
        return Err(DataError::Unauthorized);
    }
    let id = STATE.with(|s| {
        let mut st = s.borrow_mut();
        let id = st.next_trade_idea_id;
        st.next_trade_idea_id += 1;
        st.trade_ideas.insert(
            id,
            TradeIdea {
                id,
                client_id: args.client_id,
                portfolio_id: args.portfolio_id,
                proposed_by: p,
                proposed_at_ns: time(),
                title: args.title.trim().into(),
                rationale: args.rationale,
                status: TradeIdeaStatus::Draft,
            },
        );
        id
    });
    record_audit(
        AuditAction::TradeIdeaProposed {
            client_id: args.client_id,
            trade_idea_id: id,
        },
        p,
    )
    .await?;
    Ok(id)
}

#[update]
async fn set_trade_idea_status(id: u64, status: TradeIdeaStatus) -> DataResult<()> {
    let p = assert_authenticated()?;
    let client_id = STATE.with(|s| {
        let st = s.borrow();
        st.trade_ideas
            .get(&id)
            .map(|i| i.client_id)
            .ok_or(DataError::NotFound)
    })?;
    if !can_view_client(p, client_id).await? {
        return Err(DataError::Unauthorized);
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        let idea = st.trade_ideas.get_mut(&id).ok_or(DataError::NotFound)?;
        idea.status = status;
        Ok::<(), DataError>(())
    })?;
    record_audit(
        AuditAction::TradeIdeaStatusChanged {
            trade_idea_id: id,
            status,
        },
        p,
    )
    .await
}

ic_cdk::export_candid!();
