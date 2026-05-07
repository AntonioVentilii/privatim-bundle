//! Privatim ai_assistant
//!
//! Real LLM client. No model weights ship inside this canister — inference
//! runs on the engine's GPU node, reached via an HTTPS outcall to a
//! configurable base URL (e.g. `https://[ipv6]:11500`). The remote endpoint
//! exposes `POST /v1/agent/run` with a single-turn schema (prompt +
//! preamble + context + max_turns).
//!
//! 1. Accepts a structured intent from the frontend (`PortfolioOverview`,
//!    `RiskAssessment`, …) plus an optional `client_id`.
//! 2. Calls `data` (5-canister architecture) under the caller's identity to
//!    fetch only the records the caller is already authorised to see.
//! 3. Builds a single preamble (system prompt) bundling role, intent
//!    instructions, and the JSON-serialised records. Calls the LLM via
//!    `ic_cdk::management_canister::http_request`.
//! 4. Returns the LLM's text output along with citations derived from the
//!    records that were fetched (the model is told to refer to them by
//!    `[#N]` markers — citations live canister-side regardless of what
//!    the model says).
//! 5. Records `AssistantQueried` + `AssistantResponded` directly on the
//!    `audit` canister so the AI's activity lives on the same hash chain
//!    as everything else.

use candid::{CandidType, Principal};
use ic_cdk::api::msg_caller as caller;
use ic_cdk::management_canister::{HttpHeader, HttpMethod, HttpRequestArgs, http_request};
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

// ───────────────────── intent surface ─────────────────────

#[derive(Clone, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssistantIntent {
    PortfolioOverview,
    RiskAssessment,
    KycStatus,
    MeetingDigest,
    OpenTradeIdeas,
    FxExposureBook,
    KycActionList,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct AssistantRequest {
    pub intent: AssistantIntent,
    pub client_id: Option<u64>,
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
    pub answer: String,
    pub citations: Vec<AssistantCitation>,
    pub audit_seq: u64,
    pub model: String,
    /// Wall-clock time the canister spent computing the response, in
    /// milliseconds. Includes inter-canister calls to `data` and
    /// `audit`. Lets the UI render an honest "inferred in N ms" badge
    /// alongside `model` — when this stub is replaced with a real LLM,
    /// the same field carries the inference time of the model itself.
    pub inference_ms: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum AssistantError {
    Unauthorized,
    NotFound,
    BackendUnreachable(String),
    NotConfigured(String),
}

pub type AssistantResult<T> = Result<T, AssistantError>;

// ───────────────────── shared types (mirrored from data) ─────────────

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
pub struct AuditHead {
    pub seq: u64,
    pub hash: String,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum AuditAction {
    AssistantQueried { client_id: Option<u64>, intent: String },
    AssistantResponded {
        client_id: Option<u64>,
        intent: String,
        citations: Vec<u64>,
    },
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum AuditError {
    Unauthorized,
    InvalidArgument(String),
    IdentityCanisterNotConfigured,
}

// ───────────────────── state ─────────────────────

#[derive(Default, CandidType, Serialize, Deserialize)]
struct State {
    identity_canister: Option<Principal>,
    data_canister: Option<Principal>,
    audit_canister: Option<Principal>,
    registered_with_identity: bool,
    /// Base URL of the on-engine LLM HTTP endpoint, e.g.
    /// `https://[ipv6]:11500`. The canister appends `/v1/agent/run` and
    /// posts a `RunRequest` JSON body. Set via env var
    /// `PUBLIC_LLM_BASE_URL` at install time, or rotated at runtime via
    /// `set_llm_base_url` (controller-only). When `None`, `ask` returns
    /// `NotConfigured("llm")`.
    llm_base_url: Option<String>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn get_data() -> AssistantResult<Principal> {
    STATE
        .with(|s| s.borrow().data_canister)
        .ok_or_else(|| AssistantError::NotConfigured("data".into()))
}

fn get_audit() -> AssistantResult<Principal> {
    STATE
        .with(|s| s.borrow().audit_canister)
        .ok_or_else(|| AssistantError::NotConfigured("audit".into()))
}

fn get_llm_base_url() -> AssistantResult<String> {
    STATE
        .with(|s| s.borrow().llm_base_url.clone())
        .ok_or_else(|| AssistantError::NotConfigured("llm".into()))
}

// ───────────────────── lifecycle ─────────────────────

fn read_principal_env(name: &str) -> Option<Principal> {
    if !ic_cdk::api::env_var_name_exists(name) {
        return None;
    }
    Principal::from_text(ic_cdk::api::env_var_value(name)).ok()
}

fn read_string_env(name: &str) -> Option<String> {
    if !ic_cdk::api::env_var_name_exists(name) {
        return None;
    }
    let v = ic_cdk::api::env_var_value(name);
    if v.is_empty() { None } else { Some(v) }
}

#[init]
fn init() {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if let Some(p) = read_principal_env("PUBLIC_CANISTER_ID:identity") {
            st.identity_canister = Some(p);
        }
        if let Some(p) = read_principal_env("PUBLIC_CANISTER_ID:data") {
            st.data_canister = Some(p);
        }
        if let Some(p) = read_principal_env("PUBLIC_CANISTER_ID:audit") {
            st.audit_canister = Some(p);
        }
        if let Some(url) = read_string_env("PUBLIC_LLM_BASE_URL") {
            st.llm_base_url = Some(url);
        }
    });
    // Self-registration with identity happens lazily on the first `ask`
    // (init can't do inter-canister calls — IC0504). See
    // `ensure_registered_with_identity`.
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
fn set_data_canister(p: Principal) -> AssistantResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(AssistantError::Unauthorized);
    }
    STATE.with(|s| s.borrow_mut().data_canister = Some(p));
    Ok(())
}

#[update]
fn set_audit_canister(p: Principal) -> AssistantResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(AssistantError::Unauthorized);
    }
    STATE.with(|s| s.borrow_mut().audit_canister = Some(p));
    Ok(())
}

/// Sets the base URL of the on-engine LLM HTTP endpoint (controller-only).
/// Empty string clears the configuration. Returns `Unauthorized` for
/// non-controllers. Persists across upgrades.
#[update]
fn set_llm_base_url(url: String) -> AssistantResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(AssistantError::Unauthorized);
    }
    STATE.with(|s| {
        s.borrow_mut().llm_base_url = if url.trim().is_empty() {
            None
        } else {
            Some(url.trim().to_string())
        };
    });
    Ok(())
}

#[query]
fn whoami() -> Principal {
    ic_cdk::api::canister_self()
}

/// Backwards-compatible config getter — returns (data, audit). New code
/// should prefer `llm_config` for the full picture.
#[query]
fn config() -> (Option<Principal>, Option<Principal>) {
    STATE.with(|s| {
        let st = s.borrow();
        (st.data_canister, st.audit_canister)
    })
}

/// Returns the currently configured LLM base URL (or `None` if unset).
#[query]
fn llm_config() -> Option<String> {
    STATE.with(|s| s.borrow().llm_base_url.clone())
}

// ───────────────────── intent handlers ─────────────────────

/// Hard cap on the agent's reasoning turns. The schema doc recommends 30.
const DEFAULT_MAX_TURNS: usize = 30;

/// Self-registers this canister's principal with the identity canister
/// on first authenticated call. Idempotent: subsequent runs after the
/// first success are no-ops. Required because `init` can't make
/// inter-canister calls (IC0504), so we defer the registration to the
/// first user-driven update.
async fn ensure_registered_with_identity() {
    let already = STATE.with(|s| s.borrow().registered_with_identity);
    if already {
        return;
    }
    let identity = STATE.with(|s| s.borrow().identity_canister);
    if let Some(identity) = identity {
        let res: Result<(Result<(), candid::Reserved>,), _> =
            ic_cdk::api::call::call(identity, "register_ai_assistant_self", ()).await;
        // Mark registered on success OR on AlreadyBootstrapped (the slot
        // is filled, possibly by an earlier run, possibly by a manual
        // admit). Either way, `data._for` will accept us.
        if res.is_ok() {
            STATE.with(|s| s.borrow_mut().registered_with_identity = true);
        }
    }
}

#[update]
async fn ask(req: AssistantRequest) -> AssistantResult<AssistantResponse> {
    let user = caller();
    if user == Principal::anonymous() {
        return Err(AssistantError::Unauthorized);
    }
    let started_at_ns = ic_cdk::api::time();
    ensure_registered_with_identity().await;
    let data = get_data()?;
    let audit = get_audit()?;
    // Fail fast if the LLM endpoint is unset — no point fetching records
    // we can't synthesise into an answer. The unconfigured-fallback path
    // is intentionally absent: this canister is the LLM client, period.
    let llm_base_url = get_llm_base_url()?;

    // Phase 1: gather records (carries the authz boundary). Each gatherer
    // returns the citations (in [#0..N] order) and a JSON-shaped records
    // block to splice into the preamble.
    let (citations, records_block, intent_description) = match req.intent {
        AssistantIntent::PortfolioOverview => {
            gather_portfolio_overview(data, user, req.client_id).await?
        }
        AssistantIntent::RiskAssessment => {
            gather_risk_assessment(data, user, req.client_id).await?
        }
        AssistantIntent::KycStatus => gather_kyc_status(data, user, req.client_id).await?,
        AssistantIntent::MeetingDigest => {
            gather_meeting_digest(data, user, req.client_id).await?
        }
        AssistantIntent::OpenTradeIdeas => {
            gather_open_trade_ideas(data, user, req.client_id).await?
        }
        AssistantIntent::FxExposureBook => gather_fx_exposure_book(data, user).await?,
        AssistantIntent::KycActionList => gather_kyc_action_list(data, user).await?,
    };

    // Phase 2: build prompt + preamble and call the LLM.
    let preamble = build_preamble(intent_description, &citations, &records_block);
    let prompt = if req.raw_prompt.trim().is_empty() {
        default_prompt(&req.intent).to_string()
    } else {
        req.raw_prompt.clone()
    };
    let (answer, model_tag) = call_llm(&llm_base_url, &preamble, &prompt).await?;

    // Phase 3: audit both sides of the exchange and stamp the response.
    let intent_label = intent_label(&req.intent);
    let citation_ids: Vec<u64> = citations.iter().map(|c| c.id).collect();
    record_audit(
        audit,
        AuditAction::AssistantQueried {
            client_id: req.client_id,
            intent: intent_label.clone(),
        },
        user,
    )
    .await?;
    record_audit(
        audit,
        AuditAction::AssistantResponded {
            client_id: req.client_id,
            intent: intent_label,
            citations: citation_ids,
        },
        user,
    )
    .await?;

    let head = audit_head(audit).await?;
    let elapsed_ns = ic_cdk::api::time().saturating_sub(started_at_ns);
    let inference_ms = elapsed_ns / 1_000_000;

    Ok(AssistantResponse {
        answer,
        citations,
        audit_seq: head.seq,
        model: model_tag,
        inference_ms,
    })
}

// ───────────────────── LLM client ─────────────────────

/// Mirrors `RunRequest` from the IC node's `/v1/agent/run` endpoint.
/// Schema: `{ prompt, preamble?, context?, max_turns? }`.
#[derive(Serialize)]
struct RunRequest<'a> {
    prompt: &'a str,
    preamble: &'a str,
    context: Vec<&'a str>,
    max_turns: usize,
}

/// Mirrors `RunResponse` from the same endpoint.
#[derive(Deserialize)]
struct RunResponse {
    response: String,
    #[allow(dead_code)]
    turns_used: usize,
    provider: String,
    model: String,
}

/// Mirrors `ErrorBody` from the same endpoint — same shape for 400/500/503.
#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

/// Calls `POST {base_url}/v1/agent/run` and returns `(answer, "<provider>/<model>")`.
/// All failure modes — JSON encode/decode, transport, non-2xx HTTP — surface
/// as `AssistantError::BackendUnreachable(...)` so the frontend can show a
/// single uniform error.
///
/// Per agreement, no cycle accounting or response-size capping is done
/// here: `ic_cdk::management_canister::http_request` attaches the cost
/// it computes, and the `max_response_bytes` field is left `None`
/// (defaulting to the IC's 2 MiB ceiling).
async fn call_llm(
    base_url: &str,
    preamble: &str,
    prompt: &str,
) -> AssistantResult<(String, String)> {
    let url = format!("{}/v1/agent/run", base_url.trim_end_matches('/'));

    let body = RunRequest {
        prompt,
        preamble,
        context: Vec::new(),
        max_turns: DEFAULT_MAX_TURNS,
    };
    let body_bytes = serde_json::to_vec(&body)
        .map_err(|e| AssistantError::BackendUnreachable(format!("encode RunRequest: {e}")))?;

    let req = HttpRequestArgs {
        url,
        max_response_bytes: None,
        method: HttpMethod::POST,
        headers: vec![HttpHeader {
            name: "Content-Type".into(),
            value: "application/json".into(),
        }],
        body: Some(body_bytes),
        transform: None,
        // Non-replicated outcall: only one replica makes the call. LLM
        // responses are non-deterministic — running the same prompt on
        // every replica in the subnet would produce different completions
        // and the consensus step would discard them all. Single-replica
        // mode is the right shape for this workload, at the cost of the
        // result being trusted from one node rather than agreed by all.
        is_replicated: Some(false),
    };

    let res = http_request(&req)
        .await
        .map_err(|e| AssistantError::BackendUnreachable(format!("http_request: {e:?}")))?;

    let status_u64 = res
        .status
        .0
        .to_u64_digits()
        .first()
        .copied()
        .unwrap_or(0);

    if !(200..300).contains(&status_u64) {
        // Try to surface the agent's own error message; fall back to raw text.
        let detail = serde_json::from_slice::<ErrorBody>(&res.body)
            .map(|e| e.error)
            .unwrap_or_else(|_| String::from_utf8_lossy(&res.body).to_string());
        return Err(AssistantError::BackendUnreachable(format!(
            "agent {status_u64}: {detail}"
        )));
    }

    let parsed: RunResponse = serde_json::from_slice(&res.body).map_err(|e| {
        AssistantError::BackendUnreachable(format!(
            "decode RunResponse: {e} (body: {})",
            String::from_utf8_lossy(&res.body)
        ))
    })?;

    Ok((parsed.response, format!("{}/{}", parsed.provider, parsed.model)))
}

/// Builds the full preamble (system prompt) that the LLM sees. Keeps
/// everything in one string per agreement — role, intent description,
/// citation rules, and the records block. The records block is a
/// numbered list using the same `[#N]` indices as the citations array
/// so the model's output can be cross-referenced canister-side.
fn build_preamble(
    intent_description: &str,
    citations: &[AssistantCitation],
    records_block: &str,
) -> String {
    let mut citation_index = String::new();
    for (i, c) in citations.iter().enumerate() {
        citation_index.push_str(&format!(
            "[#{i}] {kind:?} id={id} — {label}\n",
            i = i,
            kind = c.kind,
            id = c.id,
            label = c.label.replace('\n', " ")
        ));
    }
    if citation_index.is_empty() {
        citation_index.push_str("(no records)\n");
    }

    format!(
        "You are a Swiss private-banking assistant at Privatim. \
You answer questions strictly from the records provided below. \
Do not invent clients, portfolios, meetings, trade ideas, or numeric values. \
If the records do not contain the answer, say so plainly. \
Be concise (a few short paragraphs at most), factual, and professional. \
When you reference a record, cite it by its bracketed index marker, e.g. [#0], [#1].\n\
\n\
TASK: {intent_description}\n\
\n\
CITATION INDEX (use these markers in your answer):\n\
{citation_index}\n\
RECORDS (JSON, ordered to match the citation index):\n\
{records_block}\n",
    )
}

/// Per-intent default question used when the user submits an empty
/// `raw_prompt`. Keeps the demo flow useful when the advisor just clicks
/// Ask without typing.
fn default_prompt(intent: &AssistantIntent) -> &'static str {
    match intent {
        AssistantIntent::PortfolioOverview => {
            "Give me an overview of this client's portfolios and combined mark-to-market value."
        }
        AssistantIntent::RiskAssessment => {
            "Assess the risk of this client's holdings against their declared mandate."
        }
        AssistantIntent::KycStatus => {
            "Summarise the KYC status of this client and any action required."
        }
        AssistantIntent::MeetingDigest => {
            "Summarise the most recent meetings with this client, focusing on decisions and follow-ups."
        }
        AssistantIntent::OpenTradeIdeas => {
            "List the open trade ideas for this client and their current status."
        }
        AssistantIntent::FxExposureBook => {
            "Summarise FX exposure across my visible book, by currency, valued in CHF."
        }
        AssistantIntent::KycActionList => {
            "List the clients on my book that require KYC action and what is needed."
        }
    }
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

// All data reads from ai_assistant go through dedicated `_for(end_user)`
// update endpoints on `data`. The `end_user` is the principal that called
// `ai_assistant.ask` — data verifies caller=this_canister (via the
// principal it knows from `identity`), then performs authz against
// `end_user` exactly as the composite-query path would for a direct
// frontend call. This closes the authz hole that would otherwise exist
// if data exposed plain regular-query reads to all authenticated callers.

async fn fetch_client(
    data: Principal,
    end_user: Principal,
    id: u64,
) -> AssistantResult<Client> {
    let res: (DataResult<Client>,) =
        ic_cdk::api::call::call(data, "get_client_for", (end_user, id))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    match res.0 {
        Ok(c) => Ok(c),
        Err(DataError::NotFound) => Err(AssistantError::NotFound),
        Err(_) => Err(AssistantError::Unauthorized),
    }
}

async fn fetch_portfolio(
    data: Principal,
    end_user: Principal,
    id: u64,
) -> AssistantResult<Portfolio> {
    let res: (DataResult<Portfolio>,) =
        ic_cdk::api::call::call(data, "get_portfolio_for", (end_user, id))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    match res.0 {
        Ok(p) => Ok(p),
        Err(DataError::NotFound) => Err(AssistantError::NotFound),
        Err(_) => Err(AssistantError::Unauthorized),
    }
}

async fn fetch_meetings(
    data: Principal,
    end_user: Principal,
    client_id: u64,
) -> AssistantResult<Vec<Meeting>> {
    let res: (DataResult<Vec<Meeting>>,) =
        ic_cdk::api::call::call(data, "list_meetings_for", (end_user, client_id))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    res.0.map_err(|_| AssistantError::Unauthorized)
}

async fn fetch_trade_ideas(
    data: Principal,
    end_user: Principal,
    client_id: u64,
) -> AssistantResult<Vec<TradeIdea>> {
    let res: (DataResult<Vec<TradeIdea>>,) =
        ic_cdk::api::call::call(data, "list_trade_ideas_for", (end_user, client_id))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    res.0.map_err(|_| AssistantError::Unauthorized)
}

async fn list_clients(
    data: Principal,
    end_user: Principal,
) -> AssistantResult<Vec<Client>> {
    let res: (Vec<Client>,) =
        ic_cdk::api::call::call(data, "list_clients_for", (end_user,))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    Ok(res.0)
}

async fn audit_head(audit: Principal) -> AssistantResult<AuditHead> {
    let res: (AuditHead,) = ic_cdk::api::call::call(audit, "audit_head", ())
        .await
        .map_err(|e| AssistantError::BackendUnreachable(format!("{e:?}")))?;
    Ok(res.0)
}

async fn record_audit(
    audit: Principal,
    action: AuditAction,
    on_behalf_of: Principal,
) -> AssistantResult<()> {
    let res: (Result<u64, AuditError>,) =
        ic_cdk::api::call::call(audit, "append", (action, on_behalf_of))
            .await
            .map_err(|e| AssistantError::BackendUnreachable(format!("audit.append: {e:?}")))?;
    res.0
        .map(|_| ())
        .map_err(|e| AssistantError::BackendUnreachable(format!("audit.append: {e:?}")))
}

// ───────────────────── record gatherers ─────────────────────
//
// Each gatherer fetches the records the caller is authorised to see
// (the authz boundary lives here, in the `_for(end_user)` calls), then
// returns:
//   - `Vec<AssistantCitation>`: the canonical citation list returned to
//     the frontend, indexed [#0..N];
//   - `String`: a JSON-formatted records block to splice into the LLM
//     preamble. Index in the JSON array matches the citation index;
//   - `&'static str`: a one-line task description used by `build_preamble`.
//
// The model never sees raw IDs or principals beyond what's in the JSON
// block — and the JSON block only contains records the user already had
// authorisation to read. This is the same security pattern the stub
// handlers used; only the synthesis step has changed.

type Gathered = (Vec<AssistantCitation>, String, &'static str);

async fn gather_portfolio_overview(
    data: Principal,
    end_user: Principal,
    client_id: Option<u64>,
) -> AssistantResult<Gathered> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(data, end_user, cid).await?;
    let mut citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    let mut records = Vec::new();
    let mut total_value_chf: i128 = 0;
    records.push(serde_json::json!({
        "marker": "[#0]",
        "kind": "Client",
        "id": client.id,
        "display_name": client.display_name,
        "legal_name": client.legal_name,
        "tax_residency": client.tax_residency,
        "client_type": format!("{:?}", client.client_type),
        "risk_profile": format!("{:?}", client.risk_profile),
        "kyc_status": format!("{:?}", client.kyc_status),
        "declared_aum_chf": client.aum_chf,
        "declared_aum_chf_pretty": format_chf(client.aum_chf as i128),
        "portfolio_count": client.portfolio_ids.len(),
    }));
    for pid in &client.portfolio_ids {
        let p = fetch_portfolio(data, end_user, *pid).await?;
        let positions_value: u128 = p
            .positions
            .iter()
            .map(|pos| (pos.quantity as u128).saturating_mul(pos.current_price_chf_cents as u128))
            .sum();
        let cash_chf = (p.cash_chf_cents as i128) / 100;
        let positions_chf = (positions_value / 100) as i128;
        let portfolio_value_chf = positions_chf + cash_chf;
        total_value_chf = total_value_chf.saturating_add(portfolio_value_chf);
        let marker = format!("[#{}]", citations.len());
        records.push(serde_json::json!({
            "marker": marker,
            "kind": "Portfolio",
            "id": p.id,
            "name": p.name,
            "base_currency": p.base_currency,
            "position_count": p.positions.len(),
            "cash_chf": cash_chf,
            "positions_value_chf": positions_chf,
            "mark_to_market_chf": portfolio_value_chf,
            "mark_to_market_chf_pretty": format_chf(portfolio_value_chf),
        }));
        citations.push(AssistantCitation {
            kind: CitationKind::Portfolio,
            id: p.id,
            label: p.name.clone(),
        });
    }
    let summary = serde_json::json!({
        "kind": "ComputedSummary",
        "total_mark_to_market_chf": total_value_chf,
        "total_mark_to_market_chf_pretty": format_chf(total_value_chf),
        "note": "Mark-to-market may differ from declared AUM due to cash sweeps and pending settlements (synthetic prices in this showcase).",
    });
    let block = render_records_block(&records, Some(&summary));
    Ok((
        citations,
        block,
        "Produce a portfolio overview for the cited client, listing each portfolio with its mark-to-market value in CHF and the combined total. Briefly note any discrepancy versus the declared AUM.",
    ))
}

async fn gather_risk_assessment(
    data: Principal,
    end_user: Principal,
    client_id: Option<u64>,
) -> AssistantResult<Gathered> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(data, end_user, cid).await?;
    let mut citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    let mut records = Vec::new();
    records.push(serde_json::json!({
        "marker": "[#0]",
        "kind": "Client",
        "id": client.id,
        "display_name": client.display_name,
        "risk_profile": format!("{:?}", client.risk_profile),
        "tax_residency": client.tax_residency,
    }));
    let mut by_class: std::collections::BTreeMap<String, u128> = Default::default();
    let mut total: u128 = 0;
    for pid in &client.portfolio_ids {
        let p = fetch_portfolio(data, end_user, *pid).await?;
        let mut portfolio_class_breakdown: std::collections::BTreeMap<String, u128> =
            Default::default();
        for pos in &p.positions {
            let v = (pos.quantity as u128).saturating_mul(pos.current_price_chf_cents as u128);
            total = total.saturating_add(v);
            let class = format!("{:?}", pos.asset_class);
            *by_class.entry(class.clone()).or_default() += v;
            *portfolio_class_breakdown.entry(class).or_default() += v;
        }
        let marker = format!("[#{}]", citations.len());
        records.push(serde_json::json!({
            "marker": marker,
            "kind": "Portfolio",
            "id": p.id,
            "name": p.name,
            "base_currency": p.base_currency,
            "position_count": p.positions.len(),
            "value_by_asset_class_chf_cents": portfolio_class_breakdown,
        }));
        citations.push(AssistantCitation {
            kind: CitationKind::Portfolio,
            id: p.id,
            label: p.name.clone(),
        });
    }
    // Pre-compute the asset-class breakdown so the model doesn't have to
    // do percentage math on cents.
    let mut breakdown = serde_json::Map::new();
    for (class, value) in &by_class {
        let pct = if total == 0 {
            0.0
        } else {
            (*value as f64 / total as f64) * 100.0
        };
        breakdown.insert(
            class.clone(),
            serde_json::json!({
                "pct_of_total": (pct * 10.0).round() / 10.0,
                "value_chf": (*value / 100) as i128,
                "value_chf_pretty": format_chf((*value / 100) as i128),
            }),
        );
    }
    // Surface the same compliance heuristic the stub had — the model can
    // pick it up or rephrase, but it's pre-decided canister-side.
    let compliance_note: Option<&'static str> = match (
        client.risk_profile,
        by_class.get("Equity").copied().unwrap_or(0),
        total,
    ) {
        (RiskProfile::Conservative, eq, t) if t > 0 && (eq as f64 / t as f64) > 0.40 => {
            Some("Equity allocation exceeds 40% but the declared risk profile is Conservative — flag for review.")
        }
        (RiskProfile::Speculative, eq, t) if t > 0 && (eq as f64 / t as f64) < 0.40 => {
            Some("Equity allocation under 40% on a Speculative mandate — likely room to deploy or reaffirm risk profile.")
        }
        _ => None,
    };
    let summary = serde_json::json!({
        "kind": "ComputedSummary",
        "asset_class_breakdown": breakdown,
        "total_market_value_chf": (total / 100) as i128,
        "total_market_value_chf_pretty": format_chf((total / 100) as i128),
        "compliance_note": compliance_note,
    });
    let block = render_records_block(&records, Some(&summary));
    Ok((
        citations,
        block,
        "Assess the risk of the cited client's holdings against their declared mandate. Show the asset-class breakdown and call out the compliance_note if present.",
    ))
}

async fn gather_kyc_status(
    data: Principal,
    end_user: Principal,
    client_id: Option<u64>,
) -> AssistantResult<Gathered> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(data, end_user, cid).await?;
    let citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    let now = ic_cdk::api::time();
    let days_to_expiry = (client.kyc_expires_ns as i128 - now as i128) / 86_400_000_000_000;
    let records = vec![serde_json::json!({
        "marker": "[#0]",
        "kind": "Client",
        "id": client.id,
        "display_name": client.display_name,
        "tax_residency": client.tax_residency,
        "kyc_status": format!("{:?}", client.kyc_status),
        "kyc_days_to_expiry": days_to_expiry,
    })];
    let block = render_records_block(&records, None);
    Ok((
        citations,
        block,
        "Summarise the cited client's KYC status. If Approved, mention days to expiry. If Pending, note that activity is restricted until approval. If Expired, state that an immediate refresh is required.",
    ))
}

async fn gather_meeting_digest(
    data: Principal,
    end_user: Principal,
    client_id: Option<u64>,
) -> AssistantResult<Gathered> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(data, end_user, cid).await?;
    let meetings = fetch_meetings(data, end_user, cid).await?;
    let mut citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    let mut records = vec![serde_json::json!({
        "marker": "[#0]",
        "kind": "Client",
        "id": client.id,
        "display_name": client.display_name,
        "meetings_on_file": meetings.len(),
    })];
    for m in meetings.iter().take(5) {
        let marker = format!("[#{}]", citations.len());
        citations.push(AssistantCitation {
            kind: CitationKind::Meeting,
            id: m.id,
            label: m.title.clone(),
        });
        records.push(serde_json::json!({
            "marker": marker,
            "kind": "Meeting",
            "id": m.id,
            "title": m.title,
            "occurred_at_ns": m.occurred_at_ns,
            "notes_md": m.notes_md,
            "decisions": m.decisions,
            "follow_ups": m.follow_ups,
        }));
    }
    let block = render_records_block(&records, None);
    Ok((
        citations,
        block,
        "Produce a digest of the cited client's most recent meetings, focusing on decisions and follow-ups. If no meetings are on file, say so plainly.",
    ))
}

async fn gather_open_trade_ideas(
    data: Principal,
    end_user: Principal,
    client_id: Option<u64>,
) -> AssistantResult<Gathered> {
    let cid = require_client_id(client_id)?;
    let client = fetch_client(data, end_user, cid).await?;
    let ideas = fetch_trade_ideas(data, end_user, cid).await?;
    let open: Vec<&TradeIdea> = ideas
        .iter()
        .filter(|i| matches!(i.status, TradeIdeaStatus::Draft | TradeIdeaStatus::Approved))
        .collect();
    let mut citations = vec![AssistantCitation {
        kind: CitationKind::Client,
        id: client.id,
        label: client.display_name.clone(),
    }];
    let mut records = vec![serde_json::json!({
        "marker": "[#0]",
        "kind": "Client",
        "id": client.id,
        "display_name": client.display_name,
        "open_trade_idea_count": open.len(),
    })];
    for idea in &open {
        let marker = format!("[#{}]", citations.len());
        citations.push(AssistantCitation {
            kind: CitationKind::TradeIdea,
            id: idea.id,
            label: idea.title.clone(),
        });
        records.push(serde_json::json!({
            "marker": marker,
            "kind": "TradeIdea",
            "id": idea.id,
            "title": idea.title,
            "rationale": idea.rationale,
            "status": format!("{:?}", idea.status),
        }));
    }
    let block = render_records_block(&records, None);
    Ok((
        citations,
        block,
        "List the open trade ideas (Draft or Approved) for the cited client with their status. If none, say so plainly.",
    ))
}

async fn gather_fx_exposure_book(
    data: Principal,
    end_user: Principal,
) -> AssistantResult<Gathered> {
    let clients = list_clients(data, end_user).await?;
    let mut citations = Vec::new();
    let mut records = Vec::new();
    let mut by_currency: std::collections::BTreeMap<String, u128> = Default::default();
    for c in &clients {
        for pid in &c.portfolio_ids {
            if let Ok(p) = fetch_portfolio(data, end_user, *pid).await {
                let value_chf_cents: u128 = p
                    .positions
                    .iter()
                    .map(|pos| {
                        (pos.quantity as u128).saturating_mul(pos.current_price_chf_cents as u128)
                    })
                    .sum();
                *by_currency.entry(p.base_currency.clone()).or_default() += value_chf_cents / 100;
                let marker = format!("[#{}]", citations.len());
                records.push(serde_json::json!({
                    "marker": marker,
                    "kind": "Portfolio",
                    "id": p.id,
                    "name": p.name,
                    "client_display_name": c.display_name,
                    "base_currency": p.base_currency,
                    "value_chf": (value_chf_cents / 100) as i128,
                }));
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
    let mut by_ccy = serde_json::Map::new();
    for (ccy, v) in &lines {
        by_ccy.insert(
            ccy.clone(),
            serde_json::json!({
                "value_chf": *v as i128,
                "value_chf_pretty": format_chf(*v as i128),
            }),
        );
    }
    let summary = serde_json::json!({
        "kind": "ComputedSummary",
        "by_base_currency": by_ccy,
        "note": "Synthetic prices in this showcase.",
    });
    let block = render_records_block(&records, Some(&summary));
    Ok((
        citations,
        block,
        "Summarise FX exposure across the user's visible book, broken down by base currency with CHF-equivalent values, sorted from largest to smallest.",
    ))
}

async fn gather_kyc_action_list(
    data: Principal,
    end_user: Principal,
) -> AssistantResult<Gathered> {
    let clients = list_clients(data, end_user).await?;
    let mut citations = Vec::new();
    let mut records = Vec::new();
    for c in &clients {
        let needs_action = matches!(
            c.kyc_status,
            KycStatusEnum::Expired | KycStatusEnum::Pending
        );
        if !needs_action {
            continue;
        }
        let marker = format!("[#{}]", citations.len());
        citations.push(AssistantCitation {
            kind: CitationKind::Client,
            id: c.id,
            label: c.display_name.clone(),
        });
        records.push(serde_json::json!({
            "marker": marker,
            "kind": "Client",
            "id": c.id,
            "display_name": c.display_name,
            "kyc_status": format!("{:?}", c.kyc_status),
            "tax_residency": c.tax_residency,
        }));
    }
    let block = render_records_block(&records, None);
    Ok((
        citations,
        block,
        "List the clients on the user's book that require KYC action and what is required for each. If the list is empty, state that plainly.",
    ))
}

/// Pretty-prints the records as a JSON array followed (optionally) by a
/// computed summary object. Kept human-readable so a developer reading
/// the audit trail or transcript can reason about what the model saw.
fn render_records_block(
    records: &[serde_json::Value],
    summary: Option<&serde_json::Value>,
) -> String {
    let arr = serde_json::Value::Array(records.to_vec());
    let mut s = serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".into());
    if let Some(sum) = summary {
        s.push_str("\n\nCOMPUTED SUMMARY:\n");
        s.push_str(&serde_json::to_string_pretty(sum).unwrap_or_default());
    }
    s
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
