//! Privatim audit canister
//!
//! The trust anchor of the whole compliance story. Append-only,
//! hash-chained log of every state-changing action across the bank
//! workspace. Other canisters call `record(action, on_behalf_of)` from
//! their update paths; this canister maintains the chain and exposes
//! it for re-verification.
//!
//! Authz uses two patterns:
//!
//! - **Writes** (`record`) are gated by an explicit allowlist of writer
//!   principals (the data, ai_assistant, and identity canister principals,
//!   admitted by a controller after deploy). Writers can ONLY call
//!   `record`; nothing else.
//! - **Reads** (`audit_log_page`, `signed_audit_export`) consult the
//!   identity canister via composite query / inter-canister call to
//!   determine the caller's role and filter accordingly. Compliance and
//!   Admin see the full log; everyone else sees only entries for which
//!   they are the `on_behalf_of` principal.

use candid::{CandidType, Principal};
use ic_cdk::api::msg_caller as caller;
use ic_cdk::api::time;
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::BTreeSet;

const MAX_AUDIT_PAGE: u64 = 200;
const MAX_INTENT_LEN: usize = 200;

// ───────────────────── shared types ─────────────────────
//
// Mirrors the Role variant from `identity` so we can issue inter-canister
// calls to `identity.has_role` without depending on its crate. Keep in
// lockstep when identity's surface changes.

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    Advisor,
    Compliance,
    Admin,
}

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq)]
pub enum KycStatus {
    Pending,
    Approved,
    Expired,
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
pub enum AuditAction {
    ClientCreated { client_id: u64 },
    ClientKycUpdated { client_id: u64, status: KycStatus },
    ClientReassigned { client_id: u64, to: Principal },
    MeetingAdded { client_id: u64, meeting_id: u64 },
    TradeIdeaProposed { client_id: u64, trade_idea_id: u64 },
    TradeIdeaStatusChanged { trade_idea_id: u64, status: TradeIdeaStatus },
    RoleGranted { grantee: Principal, role: Role },
    RoleRevoked { grantee: Principal, role: Role },
    ClientAssigned { advisor: Principal, client_id: u64 },
    ClientUnassigned { advisor: Principal, client_id: u64 },
    AdminBootstrapped { admin: Principal },
    AiAssistantAdmitted { ai: Principal },
    ClientAccessed { client_id: u64, purpose: AccessPurpose },
    AssistantQueried { client_id: Option<u64>, intent: String },
    AssistantResponded {
        client_id: Option<u64>,
        intent: String,
        citations: Vec<u64>,
    },
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
pub enum AuditError {
    Unauthorized,
    InvalidArgument(String),
    IdentityCanisterNotConfigured,
}

pub type AuditResult<T> = Result<T, AuditError>;

#[derive(Default, CandidType, Serialize, Deserialize)]
struct State {
    /// Principals allowed to call `record`. Set by controllers after deploy.
    writers: BTreeSet<Principal>,
    /// Principal of the identity canister, used for role checks on reads.
    identity_canister: Option<Principal>,
    audit: Vec<AuditEntry>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

// ───────────────────── helpers ─────────────────────

fn action_repr(a: &AuditAction) -> String {
    use AuditAction::*;
    match a {
        ClientCreated { client_id } => format!("client_created:{client_id}"),
        ClientKycUpdated { client_id, status } => format!("client_kyc:{client_id}:{status:?}"),
        ClientReassigned { client_id, to } => format!("client_reassigned:{client_id}:{to}"),
        MeetingAdded { client_id, meeting_id } => format!("meeting:{client_id}:{meeting_id}"),
        TradeIdeaProposed { client_id, trade_idea_id } => {
            format!("trade_idea:{client_id}:{trade_idea_id}")
        }
        TradeIdeaStatusChanged { trade_idea_id, status } => {
            format!("trade_idea_status:{trade_idea_id}:{status:?}")
        }
        RoleGranted { grantee, role } => format!("role_granted:{grantee}:{role:?}"),
        RoleRevoked { grantee, role } => format!("role_revoked:{grantee}:{role:?}"),
        ClientAssigned { advisor, client_id } => format!("client_assigned:{advisor}:{client_id}"),
        ClientUnassigned { advisor, client_id } => {
            format!("client_unassigned:{advisor}:{client_id}")
        }
        AdminBootstrapped { admin } => format!("admin_bootstrapped:{admin}"),
        AiAssistantAdmitted { ai } => format!("ai_admitted:{ai}"),
        ClientAccessed { client_id, purpose } => format!("client_accessed:{client_id}:{purpose:?}"),
        AssistantQueried { client_id, intent } => format!("assistant_query:{client_id:?}:{intent}"),
        AssistantResponded {
            client_id,
            intent,
            citations,
        } => {
            let cites: Vec<String> = citations.iter().map(|c| c.to_string()).collect();
            format!(
                "assistant_response:{client_id:?}:{intent}:[{}]",
                cites.join(",")
            )
        }
        ComplianceExport { from_seq, to_seq } => format!("compliance_export:{from_seq}-{to_seq}"),
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

async fn check_role(role: Role) -> AuditResult<bool> {
    let p = caller();
    if ic_cdk::api::is_controller(&p) {
        return Ok(true);
    }
    let identity = STATE
        .with(|s| s.borrow().identity_canister)
        .ok_or(AuditError::IdentityCanisterNotConfigured)?;
    let res: (bool,) = ic_cdk::api::call::call(identity, "has_role", (p, role))
        .await
        .map_err(|e| AuditError::InvalidArgument(format!("{e:?}")))?;
    Ok(res.0)
}

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
fn admit_writer(p: Principal) -> AuditResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(AuditError::Unauthorized);
    }
    STATE.with(|s| {
        s.borrow_mut().writers.insert(p);
    });
    Ok(())
}

#[update]
fn revoke_writer(p: Principal) -> AuditResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(AuditError::Unauthorized);
    }
    STATE.with(|s| {
        s.borrow_mut().writers.remove(&p);
    });
    Ok(())
}

#[update]
fn set_identity_canister(p: Principal) -> AuditResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(AuditError::Unauthorized);
    }
    STATE.with(|s| s.borrow_mut().identity_canister = Some(p));
    Ok(())
}

#[query]
fn writers() -> Vec<Principal> {
    STATE.with(|s| s.borrow().writers.iter().copied().collect())
}

#[query]
fn identity_canister() -> Option<Principal> {
    STATE.with(|s| s.borrow().identity_canister)
}

// ───────────────────── public queries ─────────────────────

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
fn total_entries() -> u64 {
    STATE.with(|s| s.borrow().audit.len() as u64)
}

/// Composite query: filters by caller's role. Compliance and Admin see the
/// full log; everyone else sees only entries where `caller == on_behalf_of`
/// (which is the principal recorded in `entry.caller` when the action was
/// taken, since writers always pass the user identity through).
#[query(composite = true)]
async fn audit_log_page(cursor: Option<u64>, limit: u64) -> AuditPage {
    let limit = limit.clamp(1, MAX_AUDIT_PAGE) as usize;
    let p = caller();
    let see_all = if ic_cdk::api::is_controller(&p) {
        true
    } else {
        let identity_opt = STATE.with(|s| s.borrow().identity_canister);
        match identity_opt {
            Some(identity) => {
                let res: Result<(bool,), _> =
                    ic_cdk::api::call::call(identity, "has_role", (p, Role::Compliance)).await;
                res.map(|r| r.0).unwrap_or(false)
            }
            None => false,
        }
    };
    STATE.with(|s| {
        let st = s.borrow();
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

// ───────────────────── writes ─────────────────────

#[update]
fn append(action: AuditAction, on_behalf_of: Principal) -> AuditResult<u64> {
    let who = caller();
    let allowed = STATE.with(|s| s.borrow().writers.contains(&who))
        || ic_cdk::api::is_controller(&who);
    if !allowed {
        return Err(AuditError::Unauthorized);
    }
    if let AuditAction::AssistantQueried { intent, .. }
    | AuditAction::AssistantResponded { intent, .. } = &action
    {
        if intent.len() > MAX_INTENT_LEN {
            return Err(AuditError::InvalidArgument("intent".into()));
        }
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        push_audit(&mut st, on_behalf_of, action);
        Ok(st.audit.len() as u64)
    })
}

#[update]
async fn signed_audit_export(from_seq: u64, to_seq: u64) -> AuditResult<ComplianceExport> {
    let p = caller();
    let is_compliance = check_role(Role::Compliance).await?;
    if !is_compliance {
        return Err(AuditError::Unauthorized);
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        let total = st.audit.len() as u64;
        let from = from_seq.min(total);
        let to = to_seq.min(total);
        if to < from {
            return Err(AuditError::InvalidArgument("range".into()));
        }
        let entries: Vec<AuditEntry> = st.audit[from as usize..to as usize].to_vec();
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

#[update]
fn reset_demo() -> AuditResult<u64> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(AuditError::Unauthorized);
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.audit.clear();
        Ok(0)
    })
}

ic_cdk::export_candid!();
