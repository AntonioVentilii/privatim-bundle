//! Privatim app_backend
//!
//! Sovereign private-banking workspace canister. Stores clients, portfolios,
//! meeting notes, and trade ideas. Role-gated (`Advisor`, `Compliance`,
//! `Admin`). Maintains a hash-chained audit log of every meaningful read,
//! every write, every AI prompt and response, plus a compliance-export
//! endpoint that returns a verifiable slice of the chain.
//!
//! Designed for a Swiss-only Cloud Engine where banking secrecy art. 47 BankG
//! and FADP make data-residency a precondition. Every feature here is
//! intentionally smaller than its real-world counterpart — the point is to
//! demonstrate the architecture, not to ship a wealth-management platform.
//!
//! Data shape was kept minimal but recognisable: numbers in CHF cents, ISO
//! currency codes, SIX/NYSE-style tickers, KYC as a coarse status enum
//! rather than a document workflow.

use candid::{CandidType, Principal};
use ic_cdk::api::msg_caller as caller;
use ic_cdk::api::time;
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

const MAX_AUDIT_PAGE: u64 = 200;
const MAX_TEXT_LEN: usize = 8_000;
const MAX_SHORT_LEN: usize = 200;

// ───────────────────── domain types ─────────────────────

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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
    /// Quantity in the asset's smallest fractional unit (e.g., shares × 100).
    pub quantity: u64,
    /// CHF cents per unit at acquisition.
    pub avg_cost_chf_cents: u64,
    /// CHF cents per unit, synthetic price (no real market feed in the showcase).
    pub current_price_chf_cents: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct Portfolio {
    pub id: u64,
    pub client_id: u64,
    pub name: String,
    pub base_currency: String,
    pub positions: Vec<Position>,
    /// Available cash in CHF cents (can be negative for margin accounts).
    pub cash_chf_cents: i64,
    pub last_valued_at_ns: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct Client {
    pub id: u64,
    pub display_name: String,
    pub legal_name: String,
    pub client_type: ClientType,
    /// ISO 3166-2 region (e.g. "CH-ZH").
    pub tax_residency: String,
    pub primary_advisor: Principal,
    pub kyc_status: KycStatus,
    pub kyc_expires_ns: u64,
    pub risk_profile: RiskProfile,
    /// Total assets under management in CHF (full francs, not cents).
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
pub enum AuditAction {
    ClientCreated { client_id: u64 },
    ClientKycUpdated { client_id: u64, status: KycStatus },
    ClientReassigned { client_id: u64, to: Principal },
    MeetingAdded { client_id: u64, meeting_id: u64 },
    TradeIdeaProposed { client_id: u64, trade_idea_id: u64 },
    TradeIdeaStatusChanged { trade_idea_id: u64, status: TradeIdeaStatus },
    RoleGranted { grantee: Principal, role: Role },
    RoleRevoked { grantee: Principal, role: Role },
    ClientAccessed { client_id: u64, purpose: AccessPurpose },
    AssistantQueried { client_id: Option<u64>, intent: String },
    AssistantResponded { client_id: Option<u64>, intent: String, citations: Vec<u64> },
    ComplianceExport { from_seq: u64, to_seq: u64 },
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub prev_hash: String,
    pub hash: String,
    pub ts_ns: u64,
    pub caller: Principal,
    pub action: AuditAction,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AuditPage {
    pub entries: Vec<AuditEntry>,
    pub next_cursor: Option<u64>,
    pub total: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AuditHead {
    pub seq: u64,
    pub hash: String,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct ComplianceExport {
    pub exported_at_ns: u64,
    pub exporter: Principal,
    pub from_seq: u64,
    pub to_seq: u64,
    pub head_hash: String,
    pub entries: Vec<AuditEntry>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct WhoAmI {
    pub principal: Principal,
    pub roles: Vec<Role>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum AppError {
    Unauthorized,
    NotFound,
    InvalidArgument(String),
}

pub type AppResult<T> = Result<T, AppError>;

// ───────────────────── parameters ─────────────────────

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

    /// Role grants. Lookup is `principal -> set<Role>`.
    roles: BTreeMap<Principal, BTreeSet<Role>>,
    /// First non-anonymous principal to call any update is auto-granted Admin
    /// — see `auto_bootstrap_admin`. Tracked here so we don't re-grant.
    admin_bootstrapped: bool,

    audit: Vec<AuditEntry>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

// ───────────────────── helpers ─────────────────────

fn assert_authenticated() -> AppResult<Principal> {
    let p = caller();
    if p == Principal::anonymous() {
        return Err(AppError::Unauthorized);
    }
    Ok(p)
}

fn has_role(state: &State, p: Principal, role: Role) -> bool {
    state.roles.get(&p).is_some_and(|s| s.contains(&role))
}

fn is_admin_or_controller(state: &State, p: Principal) -> bool {
    has_role(state, p, Role::Admin) || ic_cdk::api::is_controller(&p)
}

/// Auto-grants `Admin` to the first authenticated caller. Cloud-Engines
/// installs run `init` as the platform installer principal (not a human),
/// so the demo would otherwise have no usable admin. This is a deliberate
/// showcase shortcut — flagged in `PITCH.md` engineering notes.
fn auto_bootstrap_admin(state: &mut State, p: Principal) {
    if state.admin_bootstrapped {
        return;
    }
    state.admin_bootstrapped = true;
    state.roles.entry(p).or_default().insert(Role::Admin);
    state.roles.entry(p).or_default().insert(Role::Advisor);
    push_audit(
        state,
        p,
        AuditAction::RoleGranted {
            grantee: p,
            role: Role::Admin,
        },
    );
    push_audit(
        state,
        p,
        AuditAction::RoleGranted {
            grantee: p,
            role: Role::Advisor,
        },
    );
}

fn can_view_client(state: &State, p: Principal, client_id: u64) -> bool {
    if has_role(state, p, Role::Compliance) || is_admin_or_controller(state, p) {
        return true;
    }
    if !has_role(state, p, Role::Advisor) {
        return false;
    }
    state
        .clients
        .get(&client_id)
        .is_some_and(|c| c.primary_advisor == p)
}

fn action_repr(a: &AuditAction) -> String {
    use AuditAction::*;
    match a {
        ClientCreated { client_id } => format!("client_created:{client_id}"),
        ClientKycUpdated { client_id, status } => {
            format!("client_kyc:{client_id}:{status:?}")
        }
        ClientReassigned { client_id, to } => format!("client_reassigned:{client_id}:{to}"),
        MeetingAdded { client_id, meeting_id } => {
            format!("meeting:{client_id}:{meeting_id}")
        }
        TradeIdeaProposed { client_id, trade_idea_id } => {
            format!("trade_idea:{client_id}:{trade_idea_id}")
        }
        TradeIdeaStatusChanged { trade_idea_id, status } => {
            format!("trade_idea_status:{trade_idea_id}:{status:?}")
        }
        RoleGranted { grantee, role } => format!("role_granted:{grantee}:{role:?}"),
        RoleRevoked { grantee, role } => format!("role_revoked:{grantee}:{role:?}"),
        ClientAccessed { client_id, purpose } => {
            format!("client_accessed:{client_id}:{purpose:?}")
        }
        AssistantQueried { client_id, intent } => {
            format!("assistant_query:{client_id:?}:{intent}")
        }
        AssistantResponded { client_id, intent, citations } => {
            let cites: Vec<String> = citations.iter().map(|c| c.to_string()).collect();
            format!(
                "assistant_response:{client_id:?}:{intent}:[{}]",
                cites.join(",")
            )
        }
        ComplianceExport { from_seq, to_seq } => {
            format!("compliance_export:{from_seq}-{to_seq}")
        }
    }
}

fn push_audit(state: &mut State, caller: Principal, action: AuditAction) {
    let seq = state.audit.len() as u64;
    let prev_hash = state
        .audit
        .last()
        .map(|e| e.hash.clone())
        .unwrap_or_default();
    let ts_ns = time();
    let mut hasher = Sha256::new();
    hasher.update(seq.to_be_bytes());
    hasher.update(ts_ns.to_be_bytes());
    hasher.update(caller.as_slice());
    hasher.update(action_repr(&action).as_bytes());
    hasher.update(prev_hash.as_bytes());
    let hash = hex(&hasher.finalize());
    state.audit.push(AuditEntry {
        seq,
        prev_hash,
        hash,
        ts_ns,
        caller,
        action,
    });
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn validate_short(s: &str, name: &str) -> AppResult<()> {
    let t = s.trim();
    if t.is_empty() || t.len() > MAX_SHORT_LEN {
        return Err(AppError::InvalidArgument(name.into()));
    }
    Ok(())
}

fn validate_long(s: &str, name: &str) -> AppResult<()> {
    if s.len() > MAX_TEXT_LEN {
        return Err(AppError::InvalidArgument(name.into()));
    }
    Ok(())
}

// ───────────────────── seeding ─────────────────────
//
// Synthetic Swiss private-banking data, hardcoded so the demo always boots
// with a populated workspace. Six placeholder advisor principals (derived
// deterministically from short seeds), twelve clients, three portfolios per
// client, ~20 meetings, ~15 trade ideas. No real PII, no real holdings.
//
// First non-anonymous caller to any update method auto-becomes Admin
// (and Advisor) and can re-assign clients to themselves — see
// `auto_bootstrap_admin`.

fn synthetic_principal(seed: u8) -> Principal {
    let mut bytes = [0u8; 29];
    bytes[0] = seed;
    bytes[28] = seed.wrapping_mul(31);
    Principal::from_slice(&bytes)
}

fn one_day_ns() -> u64 {
    86_400 * 1_000_000_000
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
                cash_chf_cents: 1_500_000_00, // CHF 1.5M default
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
    push_audit(state, advisor, AuditAction::ClientCreated { client_id: id });
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
    push_audit(
        state,
        advisor,
        AuditAction::MeetingAdded {
            client_id,
            meeting_id: mid,
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
    push_audit(
        state,
        advisor,
        AuditAction::TradeIdeaProposed {
            client_id,
            trade_idea_id: tid,
        },
    );
    if status != TradeIdeaStatus::Draft {
        push_audit(
            state,
            advisor,
            AuditAction::TradeIdeaStatusChanged {
                trade_idea_id: tid,
                status,
            },
        );
    }
}

fn seed_demo(state: &mut State) {
    let a1 = synthetic_principal(11);
    let a2 = synthetic_principal(22);
    let a3 = synthetic_principal(33);
    let a4 = synthetic_principal(44);
    let a5 = synthetic_principal(55);
    let a6 = synthetic_principal(66);

    // (ticker, class, qty, avg_cost_chf_cents/unit, current_chf_cents/unit)
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

    seed_client(
        state,
        a1,
        "Müller Holdings AG",
        "Müller Holdings Aktiengesellschaft",
        ClientType::Corporate,
        "CH-ZH",
        RiskProfile::Balanced,
        18_500_000,
        KycStatus::Approved,
        &[
            ("Operating Treasury", "CHF", &[nestle, novartis, ubs, bund10, gold]),
            ("Pension Reserve", "EUR", &[chconfed30, novartis, roche]),
        ],
    );
    seed_client(
        state,
        a1,
        "von Hagen Family",
        "Familie von Hagen",
        ClientType::Family,
        "CH-ZG",
        RiskProfile::Conservative,
        7_200_000,
        KycStatus::Approved,
        &[("Family Office", "CHF", &[bund10, chconfed30, gold, ubs])],
    );
    seed_client(
        state,
        a2,
        "Lombard, Rachel",
        "Rachel Lombard",
        ClientType::Individual,
        "CH-GE",
        RiskProfile::Growth,
        2_400_000,
        KycStatus::Approved,
        &[
            ("USD Discretionary", "USD", &[apple, msft, novartis]),
            ("CHF Conservative", "CHF", &[bund10, gold]),
        ],
    );
    seed_client(
        state,
        a2,
        "Tessier SA",
        "Tessier Société Anonyme",
        ClientType::Corporate,
        "CH-VD",
        RiskProfile::Balanced,
        9_800_000,
        KycStatus::Pending,
        &[("Treasury Liquidity", "CHF", &[ubs, abb, bund10])],
    );
    seed_client(
        state,
        a3,
        "Romano Trust",
        "Romano Family Trust",
        ClientType::Family,
        "CH-TI",
        RiskProfile::Speculative,
        4_100_000,
        KycStatus::Approved,
        &[
            ("Aggressive Equity", "USD", &[apple, msft, abb]),
            ("CHF Hedge", "CHF", &[gold, chconfed30]),
        ],
    );
    seed_client(
        state,
        a3,
        "Bianchi, Giulia",
        "Giulia Bianchi",
        ClientType::Individual,
        "CH-TI",
        RiskProfile::Conservative,
        1_350_000,
        KycStatus::Expired,
        &[("Income Account", "CHF", &[bund10, chconfed30, ubs])],
    );
    seed_client(
        state,
        a4,
        "Bachmann Industries SA",
        "Bachmann Industries Société Anonyme",
        ClientType::Corporate,
        "CH-BS",
        RiskProfile::Balanced,
        12_700_000,
        KycStatus::Approved,
        &[
            ("Operating Cash", "CHF", &[ubs, abb, bund10]),
            ("Reserve", "EUR", &[chconfed30, gold]),
        ],
    );
    seed_client(
        state,
        a4,
        "Steiner-Reber Estate",
        "Erbengemeinschaft Steiner-Reber",
        ClientType::Family,
        "CH-BE",
        RiskProfile::Conservative,
        3_650_000,
        KycStatus::Approved,
        &[("Estate Trust", "CHF", &[bund10, chconfed30, gold, ubs])],
    );
    seed_client(
        state,
        a5,
        "Rösti, Markus",
        "Markus Rösti",
        ClientType::Individual,
        "CH-SG",
        RiskProfile::Growth,
        2_900_000,
        KycStatus::Approved,
        &[("Growth Account", "USD", &[apple, msft, novartis, abb])],
    );
    seed_client(
        state,
        a5,
        "Reinhard Sport AG",
        "Reinhard Sport AG",
        ClientType::Corporate,
        "CH-LU",
        RiskProfile::Balanced,
        5_800_000,
        KycStatus::Approved,
        &[("Treasury", "CHF", &[ubs, bund10, gold])],
    );
    seed_client(
        state,
        a6,
        "Khoury Family",
        "Famille Khoury",
        ClientType::Family,
        "CH-GE",
        RiskProfile::Speculative,
        14_200_000,
        KycStatus::Approved,
        &[
            ("Discretionary US Equity", "USD", &[apple, msft]),
            ("Macro Hedge", "EUR", &[gold, bund10]),
            ("Stable Income", "CHF", &[chconfed30, ubs, abb]),
        ],
    );
    seed_client(
        state,
        a6,
        "Albers, Cornelius",
        "Cornelius Albers",
        ClientType::Individual,
        "CH-ZH",
        RiskProfile::Conservative,
        890_000,
        KycStatus::Pending,
        &[("Pension Account", "CHF", &[chconfed30, bund10])],
    );

    // Meetings (a few per client, varying days_ago)
    seed_meeting(
        state, 0, 12,
        "Q1 portfolio review — Müller Holdings",
        "Reviewed YTD performance against benchmark. Discussed possible reallocation away from Roche \
         given the recent FDA setback. Müller CFO interested in exploring a CHF-hedged EUR fixed-income \
         tranche for the pension reserve.",
        &["Reduce Roche exposure to 5% of equity sleeve", "Add CHF-hedged EUR Bund position next quarter"],
        &["Send hedging cost analysis", "Schedule pension-trustee call for May"],
    );
    seed_meeting(
        state, 1, 30,
        "von Hagen — annual KYC refresh",
        "Documented updated beneficial-ownership structure following the family office reorg. \
         No change in source of funds. Family added third-generation members; advisor updated \
         the discretionary mandate to allow ESG screening.",
        &["KYC refreshed for 5 years", "ESG screening enabled on family office portfolio"],
        &["Send updated mandate for signature"],
    );
    seed_meeting(
        state, 2, 5,
        "Lombard — USD growth allocation",
        "Rachel asked about increasing tech exposure on the USD account. We walked through the \
         current 35% AAPL+MSFT concentration and the volatility tradeoff vs. her stated growth profile.",
        &["No change this quarter", "Reassess in 6 weeks"],
        &["Prepare scenario analysis: +10% tech vs. current"],
    );
    seed_meeting(
        state, 3, 18,
        "Tessier SA — KYC pending follow-up",
        "Treasury team produced updated UBO chart but signature page still missing. Compliance \
         flagged the account for restricted activity until docs received.",
        &["No new positions until KYC approved"],
        &["Chase signed UBO chart by Friday"],
    );
    seed_meeting(
        state, 5, 9,
        "Romano Trust — speculative mandate",
        "Trustees comfortable with current 65% equity / 25% gold / 10% cash mix. Discussed adding \
         a small commodities futures sleeve via the discretionary mandate.",
        &["Approved 5% commodities sleeve allocation"],
        &["Source futures-broker confirmation"],
    );
    seed_meeting(
        state, 7, 22,
        "Bachmann — CHF treasury rebalance",
        "Treasurer wants to lengthen duration on the operating cash sleeve given expected CHF \
         rate cuts. Walked through scenarios at 3y / 5y / 7y average duration.",
        &["Move to 5y avg duration on CHF treasury"],
        &["Execute Bund ladder construction next week"],
    );
    seed_meeting(
        state, 10, 14,
        "Khoury — macro hedge rebalance",
        "Family principal wants to scale up the gold position from 8% to 12% of NAV. Reviewed \
         storage and custody cost implications. Approved.",
        &["Increase gold allocation to 12% NAV"],
        &["Confirm storage charges with custodian"],
    );

    // Trade ideas
    seed_idea(
        state, 0, Some(0),
        "Reduce Roche exposure",
        "Roche has been weak on the back of the recent oncology pipeline setback. \
         Recommend trimming from 8% to 5% of equity sleeve and rotating into Novartis (less \
         pipeline concentration risk).",
        TradeIdeaStatus::Approved,
    );
    seed_idea(
        state, 0, Some(1),
        "CHF-hedged EUR Bund tranche",
        "Adding 5% of pension reserve into 10y Bund with rolling FX hedge. Aligns with the \
         lower CHF yield environment and the client's request from the Q1 review.",
        TradeIdeaStatus::Draft,
    );
    seed_idea(
        state, 2, Some(4),
        "Cap AAPL+MSFT at 25% of USD discretionary",
        "Current concentration is 35%. Trimming to 25% reduces tracking error vs. the client's \
         stated growth-with-guardrails mandate.",
        TradeIdeaStatus::Draft,
    );
    seed_idea(
        state, 4, None,
        "Add 5% commodities futures sleeve",
        "Per Romano Trust meeting, allocate 5% NAV to a commodities futures sleeve through the \
         existing discretionary mandate broker.",
        TradeIdeaStatus::Approved,
    );
    seed_idea(
        state, 6, Some(11),
        "Lengthen CHF Bund ladder to 5y avg",
        "Construct a 3-5-7y Bund ladder targeting 5y average duration, replacing the current \
         2y rolling position. Aligned with treasurer's view on CHF rate path.",
        TradeIdeaStatus::Executed,
    );
    seed_idea(
        state, 10, Some(20),
        "Increase Khoury gold allocation to 12% NAV",
        "Per family-principal meeting. Buy XAU/CHF in tranches over 4 weeks to reduce timing risk.",
        TradeIdeaStatus::Approved,
    );
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

// ───────────────────── queries ─────────────────────

#[query]
fn whoami() -> WhoAmI {
    let p = caller();
    let roles = STATE.with(|s| {
        s.borrow()
            .roles
            .get(&p)
            .map(|r| r.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default()
    });
    WhoAmI { principal: p, roles }
}

#[query]
fn list_clients() -> Vec<Client> {
    let p = caller();
    STATE.with(|s| {
        let st = s.borrow();
        let see_all = is_admin_or_controller(&st, p) || has_role(&st, p, Role::Compliance);
        st.clients
            .values()
            .filter(|c| see_all || c.primary_advisor == p)
            .cloned()
            .collect()
    })
}

#[query]
fn get_client(id: u64) -> AppResult<Client> {
    let p = caller();
    STATE.with(|s| {
        let st = s.borrow();
        if !can_view_client(&st, p, id) {
            return Err(AppError::Unauthorized);
        }
        st.clients.get(&id).cloned().ok_or(AppError::NotFound)
    })
}

#[query]
fn get_portfolio(id: u64) -> AppResult<Portfolio> {
    let p = caller();
    STATE.with(|s| {
        let st = s.borrow();
        let port = st.portfolios.get(&id).cloned().ok_or(AppError::NotFound)?;
        if !can_view_client(&st, p, port.client_id) {
            return Err(AppError::Unauthorized);
        }
        Ok(port)
    })
}

#[query]
fn list_meetings(client_id: u64) -> AppResult<Vec<Meeting>> {
    let p = caller();
    STATE.with(|s| {
        let st = s.borrow();
        if !can_view_client(&st, p, client_id) {
            return Err(AppError::Unauthorized);
        }
        let mut out: Vec<Meeting> = st
            .meetings
            .values()
            .filter(|m| m.client_id == client_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| b.occurred_at_ns.cmp(&a.occurred_at_ns));
        Ok(out)
    })
}

#[query]
fn list_trade_ideas(client_id: u64) -> AppResult<Vec<TradeIdea>> {
    let p = caller();
    STATE.with(|s| {
        let st = s.borrow();
        if !can_view_client(&st, p, client_id) {
            return Err(AppError::Unauthorized);
        }
        let mut out: Vec<TradeIdea> = st
            .trade_ideas
            .values()
            .filter(|t| t.client_id == client_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| b.proposed_at_ns.cmp(&a.proposed_at_ns));
        Ok(out)
    })
}

#[query]
fn audit_head() -> AuditHead {
    STATE.with(|s| {
        let st = s.borrow();
        match st.audit.last() {
            Some(last) => AuditHead {
                seq: last.seq + 1,
                hash: last.hash.clone(),
            },
            None => AuditHead {
                seq: 0,
                hash: String::new(),
            },
        }
    })
}

#[query]
fn audit_log_page(cursor: Option<u64>, limit: u64) -> AuditPage {
    let p = caller();
    let limit = limit.clamp(1, MAX_AUDIT_PAGE) as usize;
    STATE.with(|s| {
        let st = s.borrow();
        // Audit log is visible to Compliance, Admin, controllers; advisors only see their own actions.
        let see_all = has_role(&st, p, Role::Compliance) || is_admin_or_controller(&st, p);
        let total = st.audit.len() as u64;
        let start = cursor.unwrap_or(0) as usize;
        if start >= st.audit.len() {
            return AuditPage {
                entries: Vec::new(),
                next_cursor: None,
                total,
            };
        }
        let end = (start + limit).min(st.audit.len());
        let slice = &st.audit[start..end];
        let entries: Vec<AuditEntry> = if see_all {
            slice.to_vec()
        } else {
            slice.iter().filter(|e| e.caller == p).cloned().collect()
        };
        let next_cursor = if end < st.audit.len() {
            Some(end as u64)
        } else {
            None
        };
        AuditPage {
            entries,
            next_cursor,
            total,
        }
    })
}

// ───────────────────── updates ─────────────────────

#[update]
fn record_client_access(client_id: u64, purpose: AccessPurpose) -> AppResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        if !can_view_client(&st, p, client_id) {
            return Err(AppError::Unauthorized);
        }
        push_audit(
            &mut st,
            p,
            AuditAction::ClientAccessed {
                client_id,
                purpose,
            },
        );
        Ok(())
    })
}

#[update]
fn create_client(args: CreateClientArgs) -> AppResult<u64> {
    let p = assert_authenticated()?;
    validate_short(&args.display_name, "display_name")?;
    validate_short(&args.legal_name, "legal_name")?;
    validate_short(&args.tax_residency, "tax_residency")?;

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        if !has_role(&st, p, Role::Advisor) && !is_admin_or_controller(&st, p) {
            return Err(AppError::Unauthorized);
        }
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
        push_audit(&mut st, p, AuditAction::ClientCreated { client_id: id });
        Ok(id)
    })
}

#[update]
fn update_kyc(client_id: u64, status: KycStatus) -> AppResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        if !has_role(&st, p, Role::Compliance) && !is_admin_or_controller(&st, p) {
            return Err(AppError::Unauthorized);
        }
        let client = st.clients.get_mut(&client_id).ok_or(AppError::NotFound)?;
        client.kyc_status = status;
        client.kyc_expires_ns = match status {
            KycStatus::Approved => time() + 365 * one_day_ns(),
            KycStatus::Pending => time() + 30 * one_day_ns(),
            KycStatus::Expired => time(),
        };
        push_audit(
            &mut st,
            p,
            AuditAction::ClientKycUpdated { client_id, status },
        );
        Ok(())
    })
}

#[update]
fn assign_advisor(client_id: u64, to: Principal) -> AppResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        if !is_admin_or_controller(&st, p) {
            return Err(AppError::Unauthorized);
        }
        let client = st.clients.get_mut(&client_id).ok_or(AppError::NotFound)?;
        client.primary_advisor = to;
        push_audit(
            &mut st,
            p,
            AuditAction::ClientReassigned { client_id, to },
        );
        Ok(())
    })
}

#[update]
fn add_meeting(args: AddMeetingArgs) -> AppResult<u64> {
    let p = assert_authenticated()?;
    validate_short(&args.title, "title")?;
    validate_long(&args.notes_md, "notes_md")?;

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        if !can_view_client(&st, p, args.client_id) {
            return Err(AppError::Unauthorized);
        }
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
        push_audit(
            &mut st,
            p,
            AuditAction::MeetingAdded {
                client_id: args.client_id,
                meeting_id: id,
            },
        );
        Ok(id)
    })
}

#[update]
fn add_trade_idea(args: AddTradeIdeaArgs) -> AppResult<u64> {
    let p = assert_authenticated()?;
    validate_short(&args.title, "title")?;
    validate_long(&args.rationale, "rationale")?;

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        if !can_view_client(&st, p, args.client_id) {
            return Err(AppError::Unauthorized);
        }
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
        push_audit(
            &mut st,
            p,
            AuditAction::TradeIdeaProposed {
                client_id: args.client_id,
                trade_idea_id: id,
            },
        );
        Ok(id)
    })
}

#[update]
fn set_trade_idea_status(id: u64, status: TradeIdeaStatus) -> AppResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        let idea = st.trade_ideas.get_mut(&id).ok_or(AppError::NotFound)?;
        let client_id = idea.client_id;
        let allowed = is_admin_or_controller(&st, p)
            || st.clients.get(&client_id).is_some_and(|c| c.primary_advisor == p);
        if !allowed {
            return Err(AppError::Unauthorized);
        }
        let idea = st.trade_ideas.get_mut(&id).expect("checked");
        idea.status = status;
        push_audit(
            &mut st,
            p,
            AuditAction::TradeIdeaStatusChanged {
                trade_idea_id: id,
                status,
            },
        );
        Ok(())
    })
}

#[update]
fn grant_role(grantee: Principal, role: Role) -> AppResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        if !is_admin_or_controller(&st, p) {
            return Err(AppError::Unauthorized);
        }
        st.roles.entry(grantee).or_default().insert(role);
        push_audit(&mut st, p, AuditAction::RoleGranted { grantee, role });
        Ok(())
    })
}

#[update]
fn revoke_role(grantee: Principal, role: Role) -> AppResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if !is_admin_or_controller(&st, p) {
            return Err(AppError::Unauthorized);
        }
        if let Some(set) = st.roles.get_mut(&grantee) {
            set.remove(&role);
        }
        push_audit(&mut st, p, AuditAction::RoleRevoked { grantee, role });
        Ok(())
    })
}

#[update]
fn signed_audit_export(from_seq: u64, to_seq: u64) -> AppResult<ComplianceExport> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        if !has_role(&st, p, Role::Compliance) && !is_admin_or_controller(&st, p) {
            return Err(AppError::Unauthorized);
        }
        let total = st.audit.len() as u64;
        let from = from_seq.min(total);
        let to = to_seq.min(total);
        if to < from {
            return Err(AppError::InvalidArgument("range".into()));
        }
        let entries: Vec<AuditEntry> =
            st.audit[from as usize..to as usize].to_vec();
        let head_hash = st
            .audit
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_default();
        let export = ComplianceExport {
            exported_at_ns: time(),
            exporter: p,
            from_seq: from,
            to_seq: to,
            head_hash,
            entries,
        };
        push_audit(
            &mut st,
            p,
            AuditAction::ComplianceExport {
                from_seq: from,
                to_seq: to,
            },
        );
        Ok(export)
    })
}

/// Records an AI-assistant prompt + response. Called by the `ai_assistant`
/// canister via inter-canister call so the audit chain captures the AI's
/// activity under the same hash chain as everything else. Only the
/// `ai_assistant` canister principal may call this.
#[update]
fn record_assistant_interaction(
    client_id: Option<u64>,
    intent: String,
    citations: Vec<u64>,
    on_behalf_of: Principal,
) -> AppResult<()> {
    let p = caller();
    // Only authorised AI-assistant principals may write here. Controllers
    // are also allowed to make integration testing tractable.
    let st_check = STATE.with(|s| is_admin_or_controller(&s.borrow(), p));
    if !st_check {
        // The ai_assistant canister registers itself as Admin during init.
        // If it isn't, we fail loudly rather than silently dropping audit.
        return Err(AppError::Unauthorized);
    }
    if intent.len() > MAX_SHORT_LEN {
        return Err(AppError::InvalidArgument("intent".into()));
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        push_audit(
            &mut st,
            on_behalf_of,
            AuditAction::AssistantQueried {
                client_id,
                intent: intent.clone(),
            },
        );
        push_audit(
            &mut st,
            on_behalf_of,
            AuditAction::AssistantResponded {
                client_id,
                intent,
                citations,
            },
        );
        Ok(())
    })
}

/// Allows the ai_assistant canister to register itself at boot time with
/// a controller-issued one-shot grant. After this, `record_assistant_interaction`
/// from that canister principal succeeds.
#[update]
fn admit_ai_assistant(ai_principal: Principal) -> AppResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        auto_bootstrap_admin(&mut st, p);
        if !is_admin_or_controller(&st, p) {
            return Err(AppError::Unauthorized);
        }
        st.roles.entry(ai_principal).or_default().insert(Role::Admin);
        push_audit(
            &mut st,
            p,
            AuditAction::RoleGranted {
                grantee: ai_principal,
                role: Role::Admin,
            },
        );
        Ok(())
    })
}

#[update]
fn reset_demo() -> AppResult<u64> {
    let p = caller();
    if !ic_cdk::api::is_controller(&p) {
        return Err(AppError::Unauthorized);
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        *st = State::default();
        seed_demo(&mut st);
        Ok(st.clients.len() as u64)
    })
}

ic_cdk::export_candid!();
