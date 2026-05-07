# Pitch — bullet points

> Scannable companion to [`PITCH.md`](./PITCH.md). Each bullet links to
> the full argument and receipts in the main document. Use this as the
> speaker notes; use `PITCH.md` for the writeup.

---

## Framing

- **Swiss private banks have not deployed AI on client data at scale —
  not because the capability isn't available, but because banking
  secrecy art. 47 BankG is criminal liability and Microsoft Copilot in
  Azure-Switzerland is a contract, not a proof.** Cloud Engines closes
  that gap. → [Why private banking is the right vertical](./PITCH.md#why-private-banking-is-the-right-vertical)
- **The AI in this bundle is a transparent stub** (`model: stub-v1`).
  Real intent routing over real client data with real source-line
  citations. The pitch is the architecture surrounding the AI box, not
  the box itself. v2 swap-out is one canister.
  → opening callout in [`PITCH.md`](./PITCH.md#why-private-banking-is-the-right-vertical)

---

## The four problems Cloud Engines actually solves for an app like this

### 1. Auditability that survives FINMA / the EU AI Act

- A FINMA examiner asks for the chain of decisions for client X. On
  Microsoft 365 + Azure-Switzerland, the answer is "we have logs and we
  trust ourselves" — a contract, not a proof.
- Cloud Engines makes the substrate the auditor: every state transition
  is signed by the engine threshold key, verifiable against the NNS
  root key by anyone, with no relationship to the operator. State is
  replicated and append-only by construction.
- This bundle adds an application-level cryptographic timeline:
  `hash_n = sha256(seq || ts || on_behalf_of || action || hash_{n-1})`
  written by all five canisters into a single chain. Captures client
  reads, writes, role changes, AI prompts, AI responses, and compliance
  exports.
- `/audit` re-verifies client-side; `/admin/compliance` exports a
  signed JSON slice for handing to the regulator.
  → [§1 — Auditability that survives FINMA / the EU AI Act](./PITCH.md#1-auditability-that-survives-finma--the-eu-ai-act)

### 2. Banking-secrecy-grade sovereignty

- "EU (Frankfurt)" or "Azure-Switzerland" is a contract clause. Schrems
  II + the CLOUD Act make it contestable. The customer cannot
  independently verify which physical machines processed a request.
- Engine creators pick specific nodes by jurisdiction / operator /
  certifications, recorded on chain. Chain-key cryptography proves
  exactly those nodes produced any given response. _Verifiable
  sovereignty, not contractual sovereignty._
- The Swiss Cloud Engines (Davos 2026, GDPR + Swiss data-protection
  law) are the canonical reference.
- Same `.icp` bundle ports across engines — Swiss-only, EU-only,
  UK-only, US-only — without code changes. Inter-canister calls stay
  inside the same threshold-signed boundary. Data and AI never leave
  the bank's compute envelope.
  → [§2 — Banking-secrecy-grade sovereignty](./PITCH.md#2-banking-secrecy-grade-sovereignty)

### 3. Governance you can name (and an AI you can defend)

- Under the EU AI Act and FINMA AI guidance, the bank must answer
  structurally: who can change the AI, who can change the data, who
  can change the audit, who can change the access rules. _"Our IT
  department, with logging"_ does not satisfy this.
- The whitepaper's _commercial_ model has three actors: node providers,
  engine creators, app deployers — with an 80/20 revenue split (80% to
  node providers as ICP/USD stablecoin, 20% burned as cycles, engine
  creator's margin in between). The _governance_ slicing of the same
  architecture splits power across three parties whose interests are
  not jointly aligned: node providers (paid for uptime, no canister
  access), engine creators (live ops + node selection, no state
  read/write), the NNS (governs the protocol, controlled by no single
  entity including DFINITY).
- This bundle adds a fourth structural layer **inside the application
  itself** by splitting into 5 canisters. Each regulator question
  ("who can change what the AI saw?", "who can rewrite the audit log?")
  maps to a different canister with a different controller set, and
  any change leaves an entry in the audit log of one of the others.
- EU AI Act Article 13 explainability: AI responses include inline
  citations to record IDs. Click any citation, see the underlying
  record. The audit chain captures every citation. _What the AI saw is
  provable per query._
  → [§3 — Governance you can name (and an AI you can defend)](./PITCH.md#3-governance-you-can-name-and-an-ai-you-can-defend)

### 4. The boring operational stuff that kills enterprise AI projects

- Pre-launch checklist for a regulated AI deployment in a private bank
  (24/7 uptime data + inference, SOC 2, ISO 27001, FINMA audits, KMS,
  HSM, IAM, DR, pen tests on data + inference, MLOps team) is 7-figures
  ongoing before user #1. _This is the actual reason private banks
  haven't shipped AI on client data._
- Protocol handles replication, consensus, key management, software
  upgrades (per-canister!), persistence, OS / patch surface. _Most
  security is a property of the substrate, not a checklist for the
  bank._
- The whitepaper's claim (§1.3) — _"an app deployer does not need a
  dedicated security team or system administrator, because the protocol
  enforces integrity and availability by design"_ — is engineering
  consequence, not slogan.
- Multi-canister design lets the bank swap LLM models — eventually
  swap inference onto a TEE/GPU node — without touching client data,
  without re-certifying the data plane, without going back through
  procurement.
- This bundle: 5 canisters, 1 manifest, ~2 800 LoC of Rust + a
  SvelteKit app. Add a real LLM, real KYC document storage, real
  trade execution and there's still no ops team.
  → [§4 — The boring operational stuff that kills enterprise AI projects](./PITCH.md#4-the-boring-operational-stuff-that-kills-enterprise-ai-projects)

---

## What this isn't (external honesty)

- Cloud Engines does not make the legal question go away — substantive
  law is still on the bank's lawyers.
- The audit chain only covers what the canisters see. Hallucinations
  faithfully record what records the model saw — the answer can still
  be wrong. Provable provenance ≠ correct recommendation.
- Engine replication is lower than mainnet (3–4 nodes is _strictly
  more resilient than a single-operator cloud_ but weaker than full
  NNS-vetted subnets). Pick spec class with eyes open.
- Engines are isolated from mainnet XNet — mainnet canister signatures
  (II, ckBTC, ckETH) work; inter-engine canister calls don't exist
  today.
- The EU AI Act conformity-assessment / bias-audit / documentation-
  management obligations are NOT solved by this architecture. The
  bundle is necessary, not sufficient, on the AI Act front.
- No trading or settlement is connected. Trade _ideas_ only —
  connecting real custody is an explicit product decision and an
  explicit attack surface; not in scope here.
  → [What this isn't](./PITCH.md#what-this-isnt)

---

## Engineering notes — internal honesty

> Six things this bundle deliberately does not do yet, what each would
> cost to fix, and how to read the demo without flattering ourselves.

- **The AI is a transparent stub** (`model: stub-v1`). Real intent
  routing + real citations + deterministic Rust synthesis. Substantive
  AI engineering is concentrated in `ai_assistant/src/lib.rs`'s seven
  intent handlers. v2 swap to a Llama-3.2-1B class on-canister model
  is ~3–5 days; TEE/GPU node is weeks-to-months when those land. _The
  audit chain, role gating, inter-canister calls, citations contract
  — all unchanged in v2._
  → [Engineering notes §1](./PITCH.md#1-the-ai-is-a-transparent-stub)

- **Trade-idea workflow is creator-trusted, no four-eyes.** Status
  changes gated by "primary advisor or Admin". No proposer/approver
  separation. ~1–2 days to add a `pending_approval` state requiring a
  Compliance principal _different from_ the proposer.
  → [Engineering notes §2](./PITCH.md#2-resolution-of-trade-ideas-is-creator-trusted)

- **State persistence is `ic_cdk::storage::stable_save`** (whole-State
  blob), not `ic-stable-structures`. Survives `icp deploy` upgrade
  and schema-compatible changes; does NOT survive `--mode reinstall`,
  canister loss, or state > heap. ~1 day to refactor `audit` and
  `data` to `StableBTreeMap` / `StableVec`.
  → [Engineering notes §3](./PITCH.md#3-state-persistence-is-ic_cdkstoragestable_save-not-ic-stable-structures)

- **Inter-canister composite-query latency is not cached.** Every
  `data.list_clients` makes 2 inter-canister calls to `identity`;
  every `get_client` makes 2–3. ~50–200ms per hop locally. For a
  showcase fine; for production add a read-through cache in `data`
  keyed by `(principal, role)` with 5–10s TTL. ~2h of work.
  → [Engineering notes §4](./PITCH.md#4-inter-canister-composite-query-latency-is-not-cached)

- **Local-dev bootstrap is manual.** Inter-canister wiring lives in
  per-canister `set_*_canister` setters. `/admin/bootstrap` page runs
  all 10 wiring calls with one click. On Cloud Engines we should make
  `init` read `PUBLIC_CANISTER_ID:<dep>` env vars (auto-injected by
  the installer) so wiring is automatic on prod and the page is a
  fallback. ~half a day.
  → [Engineering notes §5](./PITCH.md#5-local-dev-bootstrap-is-manual)

- **Synthetic advisor principals** in seed data; first-login bootstrap
  grants Admin so the user sees all clients out of the box. Real
  product seeds empty and lets bank advisors create their own clients
  (auto-assigned to creator). Demo-quality-of-life shortcut, not a
  production pattern.
  → [Engineering notes §6](./PITCH.md#6-synthetic-advisor-principals--admin-sees-all-bootstrap)

- **Smaller things worth knowing.** No KYC document workflow yet
  (round 2 adds the documents canister + client-side AES-GCM
  encryption). Audit writes are gated by an allowlist, not by
  per-canister signed attestation. Frontend is fully CSR by design —
  don't "fix" it. Inter-engine XNet doesn't exist; cross-border
  reporting needs application-layer coordination.
  → [Engineering notes — Smaller things worth knowing](./PITCH.md#smaller-things-worth-knowing)

---

## The general shape (why this isn't just about private banking)

- Replace "trade idea" with: diagnosis recommendation (healthcare),
  claim adjudication (insurance), credit decision (consumer lending),
  legal-document review, intelligence assessment, patent-strategy
  advice, board-meeting summary. Same shape: an AI advises a regulated
  professional; a regulator wants to verify the chain of events; the
  substrate must not be the operator's to control.
- Cloud Engines makes _"prove it cryptographically"_ the cheapest
  answer instead of the most expensive one.
- Private banking is the right first vertical because the liability is
  existential and the tolerance for vagueness is lowest. The patterns
  generalise downward.
  → [The general shape](./PITCH.md#the-general-shape)

---

## TL;DR for the pitch deck

> **Privatim** runs entirely on a Swiss-only Cloud Engine. Five
> canisters strictly separate roles, hash-chained audit, client
> records, AI orchestration, and the SPA. Every read of a client
> record, every AI prompt, every AI response, every role grant, every
> trade-idea state change is appended to an on-chain hash-chained
> audit log that any user, auditor, or regulator can re-verify in
> their own browser, against a key the operator does not control, on
> nodes located in a jurisdiction the operator picked and the
> network's governance system can prove.
>
> There is no PII database to leak, no admin team to compromise, no
> KMS to rotate, no Microsoft to subpoena, no parent company a court
> can compel into rewriting history.
>
> _The infrastructure is the auditor. The infrastructure is the
> AI's compliance officer._

→ [TL;DR for the pitch deck](./PITCH.md#tldr-for-the-pitch-deck)
