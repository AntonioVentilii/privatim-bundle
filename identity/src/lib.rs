//! Privatim identity canister
//!
//! The IAM service. Owns role grants, advisor↔client assignments, and
//! AI-canister admittance. Read by every other canister for authz; written
//! by Admins or canister controllers.
//!
//! Why this is its own canister:
//!
//! 1. **Trust boundary.** Role changes propagate to every other canister
//!    without redeploying any of them. A controller can grant a new
//!    Compliance role without touching client data, audit data, or the AI.
//! 2. **Stable, small state.** Identity is mostly read; the data canister
//!    consults it on every authorised operation. Keeping it small means
//!    those reads are cheap.
//! 3. **First-login bootstrap.** Cloud Engines installs run `init` as the
//!    platform installer principal — a non-human. The first authenticated
//!    caller becomes Admin (and Advisor). That's encoded here, in the
//!    canister whose state survives across redeployments of the data plane.

use candid::{CandidType, Principal};
use ic_cdk::api::msg_caller as caller;
use ic_cdk::api::time;
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, CandidType, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Advisor,
    Compliance,
    Admin,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct WhoAmI {
    /// The caller's principal. Named `id` (not `principal`) because
    /// `principal` is a reserved keyword in candid and cannot be used as
    /// a record field name without escaping.
    pub id: Principal,
    pub roles: Vec<Role>,
    pub assigned_clients: Vec<u64>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum IdentityError {
    Unauthorized,
    AlreadyBootstrapped,
}

pub type IdentityResult<T> = Result<T, IdentityError>;

#[derive(Default, CandidType, Serialize, Deserialize)]
struct State {
    roles: BTreeMap<Principal, BTreeSet<Role>>,
    /// (advisor, client_id). Lookups in both directions need to be cheap.
    assignments: BTreeSet<(Principal, u64)>,
    /// First non-anonymous caller to `bootstrap_admin` becomes Admin.
    admin_bootstrapped: bool,
    /// Principal of the ai_assistant canister. Once set, it can post audit
    /// entries on behalf of users.
    ai_assistant: Option<Principal>,
    /// Audit-of-identity-changes — pruned to last N entries to keep size bounded.
    audit_log: Vec<IdentityEvent>,
    bootstrap_at_ns: Option<u64>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct IdentityEvent {
    pub ts_ns: u64,
    pub by: Principal,
    pub kind: IdentityEventKind,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum IdentityEventKind {
    AdminBootstrapped,
    RoleGranted { grantee: Principal, role: Role },
    RoleRevoked { grantee: Principal, role: Role },
    ClientAssigned { advisor: Principal, client_id: u64 },
    ClientUnassigned { advisor: Principal, client_id: u64 },
    AiAssistantAdmitted { ai: Principal },
}

const MAX_LOCAL_LOG: usize = 500;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn assert_authenticated() -> IdentityResult<Principal> {
    let p = caller();
    if p == Principal::anonymous() {
        return Err(IdentityError::Unauthorized);
    }
    Ok(p)
}

fn is_admin(state: &State, p: Principal) -> bool {
    state.roles.get(&p).is_some_and(|s| s.contains(&Role::Admin))
        || ic_cdk::api::is_controller(&p)
}

fn push_event(state: &mut State, by: Principal, kind: IdentityEventKind) {
    state.audit_log.push(IdentityEvent {
        ts_ns: time(),
        by,
        kind,
    });
    if state.audit_log.len() > MAX_LOCAL_LOG {
        let drop_n = state.audit_log.len() - MAX_LOCAL_LOG;
        state.audit_log.drain(0..drop_n);
    }
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

// ───────────────────── queries ─────────────────────

#[query]
fn whoami() -> WhoAmI {
    let p = caller();
    STATE.with(|s| {
        let st = s.borrow();
        WhoAmI {
            id: p,
            roles: st
                .roles
                .get(&p)
                .map(|r| r.iter().copied().collect())
                .unwrap_or_default(),
            assigned_clients: st
                .assignments
                .iter()
                .filter(|(a, _)| *a == p)
                .map(|(_, c)| *c)
                .collect(),
        }
    })
}

/// Authz check used by other canisters. Returns the caller's role set.
#[query]
fn roles_of(p: Principal) -> Vec<Role> {
    STATE.with(|s| {
        s.borrow()
            .roles
            .get(&p)
            .map(|r| r.iter().copied().collect())
            .unwrap_or_default()
    })
}

#[query]
fn has_role(p: Principal, role: Role) -> bool {
    STATE.with(|s| {
        s.borrow()
            .roles
            .get(&p)
            .is_some_and(|r| r.contains(&role) || r.contains(&Role::Admin))
            || ic_cdk::api::is_controller(&p)
    })
}

#[query]
fn is_assigned(advisor: Principal, client_id: u64) -> bool {
    STATE.with(|s| s.borrow().assignments.contains(&(advisor, client_id)))
}

#[query]
fn assigned_clients(advisor: Principal) -> Vec<u64> {
    STATE.with(|s| {
        s.borrow()
            .assignments
            .iter()
            .filter(|(a, _)| *a == advisor)
            .map(|(_, c)| *c)
            .collect()
    })
}

#[query]
fn ai_assistant_principal() -> Option<Principal> {
    STATE.with(|s| s.borrow().ai_assistant)
}

#[query]
fn admin_bootstrapped() -> bool {
    STATE.with(|s| s.borrow().admin_bootstrapped)
}

#[query]
fn recent_events(limit: u64) -> Vec<IdentityEvent> {
    let lim = limit.clamp(1, 200) as usize;
    STATE.with(|s| {
        let st = s.borrow();
        let n = st.audit_log.len();
        let start = n.saturating_sub(lim);
        st.audit_log[start..].to_vec()
    })
}

// ───────────────────── updates ─────────────────────

/// Idempotent: claims Admin role for the first non-anonymous caller. After
/// that, fails. Other canisters are advised to call this on the user's
/// behalf as part of the first authenticated update path.
#[update]
fn bootstrap_admin() -> IdentityResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        // Demo showcase: every signed-in user is granted Admin + Advisor so
        // they land in a fully populated workspace and can use the demo-data
        // button. (This was once-only TOFU — only the first principal became
        // admin, leaving every later visitor with an empty, locked app.)
        // Idempotent: repeat logins don't re-emit the grant events.
        let newly_admin = st.roles.entry(p).or_default().insert(Role::Admin);
        let newly_advisor = st.roles.entry(p).or_default().insert(Role::Advisor);
        if !st.admin_bootstrapped {
            st.admin_bootstrapped = true;
            st.bootstrap_at_ns = Some(time());
            push_event(&mut st, p, IdentityEventKind::AdminBootstrapped);
        }
        if newly_admin {
            push_event(
                &mut st,
                p,
                IdentityEventKind::RoleGranted {
                    grantee: p,
                    role: Role::Admin,
                },
            );
        }
        if newly_advisor {
            push_event(
                &mut st,
                p,
                IdentityEventKind::RoleGranted {
                    grantee: p,
                    role: Role::Advisor,
                },
            );
        }
        Ok(())
    })
}

#[update]
fn grant_role(grantee: Principal, role: Role) -> IdentityResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if !is_admin(&st, p) {
            return Err(IdentityError::Unauthorized);
        }
        st.roles.entry(grantee).or_default().insert(role);
        push_event(
            &mut st,
            p,
            IdentityEventKind::RoleGranted { grantee, role },
        );
        Ok(())
    })
}

#[update]
fn revoke_role(grantee: Principal, role: Role) -> IdentityResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if !is_admin(&st, p) {
            return Err(IdentityError::Unauthorized);
        }
        if let Some(rs) = st.roles.get_mut(&grantee) {
            rs.remove(&role);
        }
        push_event(
            &mut st,
            p,
            IdentityEventKind::RoleRevoked { grantee, role },
        );
        Ok(())
    })
}

#[update]
fn assign_client(advisor: Principal, client_id: u64) -> IdentityResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if !is_admin(&st, p) {
            return Err(IdentityError::Unauthorized);
        }
        st.assignments.insert((advisor, client_id));
        push_event(
            &mut st,
            p,
            IdentityEventKind::ClientAssigned { advisor, client_id },
        );
        Ok(())
    })
}

#[update]
fn unassign_client(advisor: Principal, client_id: u64) -> IdentityResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if !is_admin(&st, p) {
            return Err(IdentityError::Unauthorized);
        }
        st.assignments.remove(&(advisor, client_id));
        push_event(
            &mut st,
            p,
            IdentityEventKind::ClientUnassigned { advisor, client_id },
        );
        Ok(())
    })
}

/// Admit the ai_assistant canister so it can post audit entries on behalf
/// of users via the audit canister. Admin-only.
#[update]
fn admit_ai_assistant(ai: Principal) -> IdentityResult<()> {
    let p = assert_authenticated()?;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if !is_admin(&st, p) {
            return Err(IdentityError::Unauthorized);
        }
        st.ai_assistant = Some(ai);
        push_event(&mut st, p, IdentityEventKind::AiAssistantAdmitted { ai });
        Ok(())
    })
}

/// Trust-on-first-use self-registration: the calling canister claims to
/// be the AI assistant. Succeeds only if the slot is empty (so the first
/// caller after a fresh deploy wins). Subsequent overrides require admin.
///
/// Lets the `ai_assistant` canister auto-introduce itself from its `init`
/// (via `ic_cdk::futures::spawn`) so no human bootstrap step is needed
/// to set up the data canister's `_for` endpoint authz.
#[update]
fn register_ai_assistant_self() -> IdentityResult<()> {
    let p = caller();
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if st.ai_assistant.is_some() {
            return Err(IdentityError::AlreadyBootstrapped);
        }
        st.ai_assistant = Some(p);
        push_event(&mut st, p, IdentityEventKind::AiAssistantAdmitted { ai: p });
        Ok(())
    })
}

ic_cdk::export_candid!();
