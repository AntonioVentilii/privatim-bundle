//! Privatim documents canister
//!
//! Stores **already-encrypted** document blobs. The canister never sees
//! plaintext. Client-side AES-256-GCM encryption happens in the user's
//! browser before upload; the encryption key lives only in the user's
//! `localStorage` keyed by document ID.
//!
//! The sovereignty story this canister enables:
//!
//! - The canister stores ciphertext (`bytes`), an IV, and a SHA-256 hash
//!   of the **plaintext** (for integrity verification on download).
//! - The canister never holds an encryption key. An engine creator with
//!   full canister-controller access cannot decrypt the stored blobs —
//!   there's nothing to decrypt with.
//! - The audit chain captures `DocumentUploaded` / `DocumentAccessed` /
//!   `DocumentDeleted` events, so reads + writes are still part of the
//!   FINMA-shaped lineage even though the canister can't read content.
//!
//! Tradeoffs for v1 (showcase):
//!
//! - Per-document random key, no per-user wrapping. The uploader's
//!   browser holds the key in localStorage. Lose the browser, lose the
//!   document. No cross-device access, no "share with another advisor".
//!   Production answer: vetkeys when stable, or threshold-signed key
//!   wrapping among advisors.

use candid::{CandidType, Principal};
use ic_cdk::api::msg_caller as caller;
use ic_cdk::api::time;
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;

const MAX_LABEL_LEN: usize = 200;
const MAX_DOC_BYTES: usize = 4 * 1024 * 1024; // 4 MiB per document — showcase cap
const SHA256_HEX_LEN: usize = 64;

// ───────────────────── domain types ─────────────────────

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct Document {
    pub id: u64,
    pub client_id: u64,
    pub label: String,
    /// 12-byte AES-GCM IV (nonce). Generated client-side.
    pub iv: Vec<u8>,
    /// AES-256-GCM ciphertext including the 16-byte auth tag at the end.
    pub ciphertext: Vec<u8>,
    /// Hex-encoded SHA-256 of the *plaintext*. Lets the user verify integrity
    /// after decryption. Never used by the canister to derive anything.
    pub plaintext_sha256: String,
    /// Original filename or descriptive label. Plaintext (for searchability).
    pub uploaded_by: Principal,
    pub uploaded_at_ns: u64,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: u64,
    pub client_id: u64,
    pub label: String,
    pub plaintext_sha256: String,
    pub uploaded_by: Principal,
    pub uploaded_at_ns: u64,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct UploadArgs {
    pub client_id: u64,
    pub label: String,
    pub iv: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub plaintext_sha256: String,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum DocumentsError {
    Unauthorized,
    NotFound,
    InvalidArgument(String),
    AuditCanisterNotConfigured,
    UpstreamFailed(String),
}

pub type DocumentsResult<T> = Result<T, DocumentsError>;

/// Mirrors the audit canister's AuditAction variants we emit. Kept in
/// lockstep with audit's declared surface.
#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum AuditAction {
    DocumentUploaded {
        client_id: u64,
        doc_id: u64,
        plaintext_sha256: String,
    },
    DocumentAccessed {
        client_id: u64,
        doc_id: u64,
    },
    DocumentDeleted {
        client_id: u64,
        doc_id: u64,
    },
}

// ───────────────────── state ─────────────────────

#[derive(Default, CandidType, Serialize, Deserialize)]
struct State {
    next_doc_id: u64,
    docs: BTreeMap<u64, Document>,
    audit_canister: Option<Principal>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn read_principal_env(name: &str) -> Option<Principal> {
    if !ic_cdk::api::env_var_name_exists(name) {
        return None;
    }
    Principal::from_text(ic_cdk::api::env_var_value(name)).ok()
}

#[init]
fn init() {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if let Some(p) = read_principal_env("PUBLIC_CANISTER_ID:audit") {
            st.audit_canister = Some(p);
        }
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

// ───────────────────── helpers ─────────────────────

fn assert_authenticated() -> DocumentsResult<Principal> {
    let p = caller();
    if p == Principal::anonymous() {
        return Err(DocumentsError::Unauthorized);
    }
    Ok(p)
}

fn validate(args: &UploadArgs) -> DocumentsResult<()> {
    if args.label.trim().is_empty() || args.label.len() > MAX_LABEL_LEN {
        return Err(DocumentsError::InvalidArgument("label".into()));
    }
    if args.iv.len() != 12 {
        return Err(DocumentsError::InvalidArgument("iv must be 12 bytes".into()));
    }
    if args.ciphertext.is_empty() || args.ciphertext.len() > MAX_DOC_BYTES {
        return Err(DocumentsError::InvalidArgument(format!(
            "ciphertext: 1..={MAX_DOC_BYTES} bytes"
        )));
    }
    if args.plaintext_sha256.len() != SHA256_HEX_LEN
        || !args.plaintext_sha256.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(DocumentsError::InvalidArgument(
            "plaintext_sha256 must be 64 hex chars".into(),
        ));
    }
    Ok(())
}

fn meta_of(d: &Document) -> DocumentMeta {
    DocumentMeta {
        id: d.id,
        client_id: d.client_id,
        label: d.label.clone(),
        plaintext_sha256: d.plaintext_sha256.clone(),
        uploaded_by: d.uploaded_by,
        uploaded_at_ns: d.uploaded_at_ns,
        size_bytes: d.size_bytes,
    }
}

async fn record_audit(action: AuditAction, on_behalf_of: Principal) -> DocumentsResult<()> {
    let audit = STATE
        .with(|s| s.borrow().audit_canister)
        .ok_or(DocumentsError::AuditCanisterNotConfigured)?;
    let _: (Result<u64, candid::Reserved>,) =
        ic_cdk::api::call::call(audit, "append", (action, on_behalf_of))
            .await
            .map_err(|e| DocumentsError::UpstreamFailed(format!("audit.append: {e:?}")))?;
    Ok(())
}

// ───────────────────── config ─────────────────────

#[update]
fn set_audit_canister(p: Principal) -> DocumentsResult<()> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(DocumentsError::Unauthorized);
    }
    STATE.with(|s| s.borrow_mut().audit_canister = Some(p));
    Ok(())
}

#[query]
fn config() -> Option<Principal> {
    STATE.with(|s| s.borrow().audit_canister)
}

// ───────────────────── reads ─────────────────────

#[query]
fn list_documents(client_id: u64) -> DocumentsResult<Vec<DocumentMeta>> {
    assert_authenticated()?;
    let mut out: Vec<DocumentMeta> = STATE.with(|s| {
        s.borrow()
            .docs
            .values()
            .filter(|d| d.client_id == client_id)
            .map(meta_of)
            .collect()
    });
    out.sort_by(|a, b| b.uploaded_at_ns.cmp(&a.uploaded_at_ns));
    Ok(out)
}

#[query]
fn get_document_meta(id: u64) -> DocumentsResult<DocumentMeta> {
    assert_authenticated()?;
    STATE.with(|s| {
        s.borrow()
            .docs
            .get(&id)
            .map(meta_of)
            .ok_or(DocumentsError::NotFound)
    })
}

/// Returns the full document (including ciphertext + IV). The caller's
/// browser then decrypts using its locally-held AES key.
#[update]
async fn fetch_document(id: u64) -> DocumentsResult<Document> {
    let p = assert_authenticated()?;
    let doc = STATE
        .with(|s| s.borrow().docs.get(&id).cloned())
        .ok_or(DocumentsError::NotFound)?;
    let _ = record_audit(
        AuditAction::DocumentAccessed {
            client_id: doc.client_id,
            doc_id: doc.id,
        },
        p,
    )
    .await;
    Ok(doc)
}

// ───────────────────── writes ─────────────────────

#[update]
async fn upload_document(args: UploadArgs) -> DocumentsResult<u64> {
    let p = assert_authenticated()?;
    validate(&args)?;

    let (id, plaintext_sha256, client_id) = STATE.with(|s| {
        let mut st = s.borrow_mut();
        let id = st.next_doc_id;
        st.next_doc_id += 1;
        let size_bytes = args.ciphertext.len() as u64;
        let plaintext_sha256 = args.plaintext_sha256.clone();
        let client_id = args.client_id;
        st.docs.insert(
            id,
            Document {
                id,
                client_id,
                label: args.label.trim().to_string(),
                iv: args.iv,
                ciphertext: args.ciphertext,
                plaintext_sha256: plaintext_sha256.clone(),
                uploaded_by: p,
                uploaded_at_ns: time(),
                size_bytes,
            },
        );
        (id, plaintext_sha256, client_id)
    });

    record_audit(
        AuditAction::DocumentUploaded {
            client_id,
            doc_id: id,
            plaintext_sha256,
        },
        p,
    )
    .await?;
    Ok(id)
}

#[update]
async fn delete_document(id: u64) -> DocumentsResult<()> {
    let p = assert_authenticated()?;
    let removed = STATE.with(|s| s.borrow_mut().docs.remove(&id));
    let doc = removed.ok_or(DocumentsError::NotFound)?;
    record_audit(
        AuditAction::DocumentDeleted {
            client_id: doc.client_id,
            doc_id: doc.id,
        },
        p,
    )
    .await?;
    Ok(())
}

#[update]
fn reset_demo() -> DocumentsResult<u64> {
    let who = caller();
    if !ic_cdk::api::is_controller(&who) {
        return Err(DocumentsError::Unauthorized);
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        let audit = st.audit_canister;
        *st = State::default();
        st.audit_canister = audit;
        Ok(0)
    })
}

ic_cdk::export_candid!();
