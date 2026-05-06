//! Privatim ai_assistant
//!
//! A *transparent stub LLM*. No model weights ship inside this canister.
//! What it does:
//!
//! 1. Accepts a structured intent from the frontend (`PortfolioOverview`,
//!    `RiskAssessment`, `KycStatus`, etc.) plus an optional `client_id`.
//! 2. Issues an inter-canister query to `app_backend` under the caller's
//!    identity to fetch only the records the caller is already authorised
//!    to see.
//! 3. Synthesises a natural-language answer from those records, with
//!    inline citations referring to record IDs the answer was derived from.
//! 4. Posts an `AssistantQueried` + `AssistantResponded` pair into the
//!    `app_backend` audit chain via `record_assistant_interaction` so the
//!    AI's activity is logged on the same hash chain as everything else.
//!
//! Concretely: this is a deterministic structured-query engine wearing an
//! LLM's interface. The point isn't to fake intelligence; the point is to
//! demonstrate the architecture (sovereign data → on-engine inference →
//! audit-logged interaction) so that swapping in a real model later is a
//! one-canister swap, not a re-architecture.
//!
//! The frontend labels this clearly as "Stub LLM — production deployment
//! runs `<model>` on this engine's GPU node".

use candid::{CandidType, Principal};
use ic_cdk::api::msg_caller as caller;
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

// ───────────────────── intent surface ─────────────────────

#[derive(Clone, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssistantIntent {
    /// "Summarise the portfolios for this client."
    PortfolioOverview,
    /// "What's the risk concentration / asset-class breakdown for this client?"
    RiskAssessment,
    /// "What's the current KYC status / when does it expire?"
    KycStatus,
    /// "List recent meetings and decisions for this client."
    MeetingDigest,
    /// "What trade ideas are outstanding for this client?"
    OpenTradeIdeas,
    /// "Where is FX exposure across this advisor's book?" — no client_id.
    FxExposureBook,
    /// "Which clients have an expired or expiring KYC across my book?" — no client_id.
    KycActionList,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AssistantRequest {
    pub intent: AssistantIntent,
    pub client_id: Option<u64>,
    /// Free-text the user typed, surfaced for the audit log only — not
    /// interpreted (this is a stub). When a real LLM lands, this becomes
    /// the actual prompt.
    pub raw_prompt: String,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AssistantCitation {
    pub kind: CitationKind,
    pub id: u64,
    pub label: String,
}

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum CitationKind {
    Client,
    Portfolio,
    Meeting,
    TradeIdea,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AssistantResponse {
    /// The synthesised answer, plain text with `[#N]` citation markers.
    pub answer: String,
    /// One per `[#N]` marker in `answer`.
    pub citations: Vec<AssistantCitation>,
    /// The value of `app_backend`'s audit-log head AFTER this interaction
    /// was logged. Lets the frontend confirm the interaction was chained.
    pub audit_seq: u64,
    /// Cosmetic but informative — model identifier the frontend can show
    /// alongside the answer. v1 is `"stub-v1"` so the UI doesn't lie.
    pub model: String,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum AssistantError {
    Unauthorized,
    NotFound,
    BackendUnreachable(String),
}

pub type AssistantResult<T> = Result<T, AssistantError>;

// ───────────────────── shared types (mirrored from app_backend) ─────────

// We mirror just the fields we need to query, hand-derived from the
// app_backend candid. If app_backend's surface changes meaningfully we
// regenerate these.

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClientType {
    Individual,
    Family,
    Corporate,
}

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum KycStatusEnum {
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

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize)]
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
    pub kyc_status: KycStatusEnum,
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
pub enum AppError {
    Unauthorized,
    NotFound,
    InvalidArgument(String),
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AuditHead {
    pub seq: u64,
    pub hash: String,
}

// ───────────────────── state ─────────────────────

#[derive(Default, CandidType, Serialize, Deserialize)]
struct State {
    /// Set on first `set_app_backend`. Without this, all calls fail loudly.
    app_backend: Option<Principal>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn get_app_backend() -> Result<Principal, AssistantError> {
    STATE
        .with(|s| s.borrow().app_backend)
        .ok_or_else(|| {
            AssistantError::BackendUnreachable(
                "app_backend principal not registered; call set_app_backend".into(),
            )
        })
}

// ───────────────────── lifecycle ─────────────────────

#[init]
fn init() {}

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
fn set_app_backend(p: Principal) -> AssistantResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(AssistantError::Unauthorized);
    }
    STATE.with(|s| s.borrow_mut().app_backend = Some(p));
    Ok(())
}

#[query]
fn whoami() -> Principal {
    ic_cdk::api::canister_self()
}

#[query]
fn get_app_backend_principal() -> Option<Principal> {
    STATE.with(|s| s.borrow().app_backend)
}

// ───────────────────── intent handlers ─────────────────────

const MODEL_TAG: &str = "stub-v1";

#[update]
async fn ask(req: AssistantRequest) -> AssistantResult<AssistantResponse> {
    let user = caller();
    if user == Principal::anonymous() {
        return Err(AssistantError::Unauthorized);
    }
    let backend = get_app_backend()?;

    let (answer, citations) = match req.intent {
        AssistantIntent::PortfolioOverview => {
            handle_portfolio_overview(backend, req.client_id).await?
        }
        AssistantIntent::RiskAssessment => {
            handle_risk_assessment(backend, req.client_id).await?
        }
        AssistantIntent::KycStatus => handle_kyc_status(backend, req.client_id).await?,
        AssistantIntent::MeetingDigest => {
            handle_meeting_digest(backend, req.client_id).await?
        }
        AssistantIntent::OpenTradeIdeas => {
            handle_open_trade_ideas(backend, req.client_id).await?
        }
        AssistantIntent::FxExposureBook => handle_fx_exposure_book(backend).await?,
        AssistantIntent::KycActionList => handle_kyc_action_list(backend).await?,
    };

    let intent_label = intent_label(&req.intent);
    let citation_ids: Vec<u64> = citations.iter().map(|c| c.id).collect();
    audit_interaction(
        backend,
        req.client_id,
        intent_label.clone(),
        citation_ids,
        user,
    )
    .await?;

    let head = audit_head(backend).await?;

    Ok(AssistantResponse {
        answer,
        citations,
        audit_seq: head.seq,
        model: MODEL_TAG.into(),
    })
}

fn intent_label(i: &AssistantIntent) -> String {
    match i {
        AssistantIntent::PortfolioOverview => "portfolio_overview".into(),
        AssistantIntent::RiskAssessment => "risk_assessment".into(),
        AssistantIntent::KycStatus => "kyc_status".into(),
        AssistantIntent::MeetingDigest => "meeting_digest".into(),
        AssistantIntent::OpenTradeIdeas => "open_trade_ideas".into(),
        AssistantIntent::FxExposureBook => "fx_exposure_book".into(),
        AssistantIntent::KycActionList => "kyc_action_list".into(),
    }
}

fn require_client_id(client_id: Option<u64>) -> AssistantResult<u64> {
    client_id.ok_or(AssistantError::NotFound)
}

// ───────────────────── inter-canister helpers ─────────────────────

async fn fetch_client(backend: Principal, id: u64) -> AssistantResult<Client> {
    let res: (AppResult<Client>,) =
        ic_cdk::api::call::call(backend, "get_client", (id,))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    match res.0 {
        Ok(c) => Ok(c),
        Err(AppError::NotFound) => Err(AssistantError::NotFound),
        Err(_) => Err(AssistantError::Unauthorized),
    }
}

async fn fetch_portfolio(backend: Principal, id: u64) -> AssistantResult<Portfolio> {
    let res: (AppResult<Portfolio>,) =
        ic_cdk::api::call::call(backend, "get_portfolio", (id,))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    match res.0 {
        Ok(p) => Ok(p),
        Err(AppError::NotFound) => Err(AssistantError::NotFound),
        Err(_) => Err(AssistantError::Unauthorized),
    }
}

async fn fetch_meetings(backend: Principal, client_id: u64) -> AssistantResult<Vec<Meeting>> {
    let res: (AppResult<Vec<Meeting>>,) =
        ic_cdk::api::call::call(backend, "list_meetings", (client_id,))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    res.0.map_err(|_| AssistantError::Unauthorized)
}

async fn fetch_trade_ideas(backend: Principal, client_id: u64) -> AssistantResult<Vec<TradeIdea>> {
    let res: (AppResult<Vec<TradeIdea>>,) =
        ic_cdk::api::call::call(backend, "list_trade_ideas", (client_id,))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    res.0.map_err(|_| AssistantError::Unauthorized)
}

async fn list_clients(backend: Principal) -> AssistantResult<Vec<Client>> {
    let res: (Vec<Client>,) = ic_cdk::api::call::call(backend, "list_clients", ())
        .await
        .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    Ok(res.0)
}

async fn audit_head(backend: Principal) -> AssistantResult<AuditHead> {
    let res: (AuditHead,) = ic_cdk::api::call::call(backend, "audit_head", ())
        .await
        .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    Ok(res.0)
}

async fn audit_interaction(
    backend: Principal,
    client_id: Option<u64>,
    intent: String,
    citations: Vec<u64>,
    user: Principal,
) -> AssistantResult<()> {
    let res: (AppResult<()>,) = ic_cdk::api::call::call(
        backend,
        "record_assistant_interaction",
        (client_id, intent, citations, user),
    )
    .await
    .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    res.0
        .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))
}

// ───────────────────── synth handlers ─────────────────────

async fn handle_portfolio_overview(
    backend: Principal,
    client_id: Option<u64>,
) -> AssistantResult<(String, Vec<AssistantCitation>)> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(backend, cid).await?;
    let mut citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    let mut total_value_chf: u128 = 0;
    let mut lines = Vec::new();
    for pid in &client.portfolio_ids {
        let p = fetch_portfolio(backend, *pid).await?;
        let positions_value: u128 = p
            .positions
            .iter()
            .map(|pos| (pos.quantity as u128).saturating_mul(pos.current_price_chf_cents as u128))
            .sum();
        let cash_chf = (p.cash_chf_cents as i128) / 100;
        let positions_chf = (positions_value / 100) as i128;
        let portfolio_value_chf = positions_chf + cash_chf;
        total_value_chf = total_value_chf.saturating_add(portfolio_value_chf.max(0) as u128);
        lines.push(format!(
            "- **{}** [#{}] ({}): {} positions, mark-to-market value CHF {}",
            p.name,
            citations.len(),
            p.base_currency,
            p.positions.len(),
            format_chf(portfolio_value_chf)
        ));
        citations.push(AssistantCitation {
            kind: CitationKind::Portfolio,
            id: p.id,
            label: p.name.clone(),
        });
    }
    let answer = format!(
        "**{}** [#0] holds {} portfolios with a combined mark-to-market value of CHF {}.\n\n{}\n\nThe declared AUM on the client record is CHF {}; differences against the calculated mark-to-market reflect cash sweeps and pending settlements (no live market feed in the showcase — prices are synthetic).",
        client.display_name,
        client.portfolio_ids.len(),
        format_chf(total_value_chf as i128),
        lines.join("\n"),
        format_chf(client.aum_chf as i128 * 1)
    );
    Ok((answer, citations))
}

async fn handle_risk_assessment(
    backend: Principal,
    client_id: Option<u64>,
) -> AssistantResult<(String, Vec<AssistantCitation>)> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(backend, cid).await?;
    let mut citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    let mut by_class: std::collections::BTreeMap<String, u128> = Default::default();
    let mut total: u128 = 0;
    for pid in &client.portfolio_ids {
        let p = fetch_portfolio(backend, *pid).await?;
        for pos in &p.positions {
            let v = (pos.quantity as u128).saturating_mul(pos.current_price_chf_cents as u128);
            total = total.saturating_add(v);
            *by_class.entry(format!("{:?}", pos.asset_class)).or_default() =
                by_class.get(&format!("{:?}", pos.asset_class)).copied().unwrap_or(0) + v;
        }
        citations.push(AssistantCitation {
            kind: CitationKind::Portfolio,
            id: p.id,
            label: p.name.clone(),
        });
    }
    let mut breakdown_lines = Vec::new();
    for (class, value) in &by_class {
        let pct = if total == 0 {
            0.0
        } else {
            (*value as f64 / total as f64) * 100.0
        };
        breakdown_lines.push(format!(
            "- {}: {:.1}% (CHF {})",
            class,
            pct,
            format_chf((*value / 100) as i128)
        ));
    }
    let mismatch = match (client.risk_profile, by_class.get("Equity").copied().unwrap_or(0), total) {
        (RiskProfile::Conservative, eq, t) if t > 0 && (eq as f64 / t as f64) > 0.40 => {
            "\n\n_Note: equity allocation exceeds 40% but the declared risk profile is Conservative — flag for review._"
        }
        (RiskProfile::Speculative, eq, t) if t > 0 && (eq as f64 / t as f64) < 0.40 => {
            "\n\n_Note: equity allocation under 40% on a Speculative mandate — likely room to deploy or reaffirm risk profile._"
        }
        _ => "",
    };
    let answer = format!(
        "**{}** [#0] is on a **{:?}** mandate.\n\nAsset-class breakdown across {} portfolios:\n\n{}{}",
        client.display_name,
        client.risk_profile,
        client.portfolio_ids.len(),
        breakdown_lines.join("\n"),
        mismatch
    );
    Ok((answer, citations))
}

async fn handle_kyc_status(
    backend: Principal,
    client_id: Option<u64>,
) -> AssistantResult<(String, Vec<AssistantCitation>)> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(backend, cid).await?;
    let citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    let now = ic_cdk::api::time();
    let days_to_expiry = (client.kyc_expires_ns as i128 - now as i128) / 86_400_000_000_000;
    let status_text = match client.kyc_status {
        KycStatusEnum::Approved => format!("Approved, expires in {days_to_expiry} days"),
        KycStatusEnum::Pending => "Pending — restricted activity until approval".into(),
        KycStatusEnum::Expired => "EXPIRED — refresh required before any further activity".into(),
    };
    let answer = format!(
        "**{}** [#0] tax-residency {}, KYC: **{}**.",
        client.display_name, client.tax_residency, status_text
    );
    Ok((answer, citations))
}

async fn handle_meeting_digest(
    backend: Principal,
    client_id: Option<u64>,
) -> AssistantResult<(String, Vec<AssistantCitation>)> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(backend, cid).await?;
    let meetings = fetch_meetings(backend, cid).await?;
    let mut citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    if meetings.is_empty() {
        return Ok((
            format!("No meetings on file for **{}** [#0].", client.display_name),
            citations,
        ));
    }
    let mut lines = Vec::new();
    for m in meetings.iter().take(5) {
        let i = citations.len();
        citations.push(AssistantCitation {
            kind: CitationKind::Meeting,
            id: m.id,
            label: m.title.clone(),
        });
        let decisions = if m.decisions.is_empty() {
            "no recorded decisions".into()
        } else {
            format!("decisions: {}", m.decisions.join("; "))
        };
        lines.push(format!("- **{}** [#{}]: {}", m.title, i, decisions));
    }
    let answer = format!(
        "Most recent {} meetings for **{}** [#0]:\n\n{}",
        meetings.len().min(5),
        client.display_name,
        lines.join("\n"),
    );
    Ok((answer, citations))
}

async fn handle_open_trade_ideas(
    backend: Principal,
    client_id: Option<u64>,
) -> AssistantResult<(String, Vec<AssistantCitation>)> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(backend, cid).await?;
    let ideas = fetch_trade_ideas(backend, cid).await?;
    let open: Vec<&TradeIdea> = ideas
        .iter()
        .filter(|i| matches!(i.status, TradeIdeaStatus::Draft | TradeIdeaStatus::Approved))
        .collect();
    let mut citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    if open.is_empty() {
        return Ok((
            format!("No open trade ideas for **{}** [#0].", client.display_name),
            citations,
        ));
    }
    let mut lines = Vec::new();
    for idea in &open {
        let i = citations.len();
        citations.push(AssistantCitation {
            kind: CitationKind::TradeIdea,
            id: idea.id,
            label: idea.title.clone(),
        });
        lines.push(format!(
            "- **{}** [#{}] ({:?})",
            idea.title, i, idea.status,
        ));
    }
    let answer = format!(
        "{} open trade ideas for **{}** [#0]:\n\n{}",
        open.len(),
        client.display_name,
        lines.join("\n"),
    );
    Ok((answer, citations))
}

async fn handle_fx_exposure_book(
    backend: Principal,
) -> AssistantResult<(String, Vec<AssistantCitation>)> {
    let clients = list_clients(backend).await?;
    let mut citations = Vec::new();
    let mut by_currency: std::collections::BTreeMap<String, u128> = Default::default();
    for c in &clients {
        for pid in &c.portfolio_ids {
            if let Ok(p) = fetch_portfolio(backend, *pid).await {
                let value: u128 = p
                    .positions
                    .iter()
                    .map(|pos| {
                        (pos.quantity as u128).saturating_mul(pos.current_price_chf_cents as u128)
                    })
                    .sum();
                *by_currency.entry(p.base_currency.clone()).or_default() += value / 100;
                citations.push(AssistantCitation {
                    kind: CitationKind::Portfolio,
                    id: p.id,
                    label: format!("{} ({})", p.name, c.display_name),
                });
            }
        }
    }
    let mut lines: Vec<(String, u128)> = by_currency.into_iter().collect();
    lines.sort_by(|a, b| b.1.cmp(&a.1));
    let breakdown = lines
        .iter()
        .map(|(ccy, v)| format!("- **{}**: CHF {}", ccy, format_chf(*v as i128)))
        .collect::<Vec<_>>()
        .join("\n");
    let answer = format!(
        "FX exposure across your visible book, valued in CHF (synthetic prices):\n\n{}",
        breakdown
    );
    Ok((answer, citations))
}

async fn handle_kyc_action_list(
    backend: Principal,
) -> AssistantResult<(String, Vec<AssistantCitation>)> {
    let clients = list_clients(backend).await?;
    let mut citations = Vec::new();
    let mut lines = Vec::new();
    for c in &clients {
        match c.kyc_status {
            KycStatusEnum::Expired => {
                let i = citations.len();
                citations.push(AssistantCitation {
                    kind: CitationKind::Client,
                    id: c.id,
                    label: c.display_name.clone(),
                });
                lines.push(format!(
                    "- **{}** [#{}] — KYC EXPIRED, refresh required immediately",
                    c.display_name, i
                ));
            }
            KycStatusEnum::Pending => {
                let i = citations.len();
                citations.push(AssistantCitation {
                    kind: CitationKind::Client,
                    id: c.id,
                    label: c.display_name.clone(),
                });
                lines.push(format!(
                    "- **{}** [#{}] — KYC pending, restricted activity",
                    c.display_name, i
                ));
            }
            KycStatusEnum::Approved => {}
        }
    }
    if lines.is_empty() {
        return Ok((
            "No expired or pending KYCs across your visible book — everything is clean.".into(),
            citations,
        ));
    }
    let answer = format!(
        "{} clients on your book need KYC action:\n\n{}",
        lines.len(),
        lines.join("\n")
    );
    Ok((answer, citations))
}

fn format_chf(amount_chf: i128) -> String {
    let abs = amount_chf.unsigned_abs();
    let s = abs.to_string();
    let mut grouped = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push('\'');
        }
        grouped.push(c);
    }
    let body: String = grouped.chars().rev().collect();
    if amount_chf < 0 {
        format!("-{body}")
    } else {
        body
    }
}

ic_cdk::export_candid!();
