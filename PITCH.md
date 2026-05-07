# Why Cloud Engines for an app like this

> A working argument, not marketing copy. If a sentence here can't be stated
> plainly, it shouldn't be in the pitch.

> **Two audiences, one document.**
> Sections 1–4 + _What this isn't_ + _The general shape_ + _TL;DR_ are the
> external pitch — the argument we'd put in front of a Swiss private bank's
> CIO, head of compliance, FINMA-facing legal counsel, or AI ethics
> committee. The _Engineering notes_ section near the end is for internal
> readers and technical evaluators: it names the things this bundle
> deliberately does not do yet, what they would cost to fix, and how to
> read the demo without flattering ourselves about what it proves.

---

## Why private banking is the right vertical

Private banking — and Swiss private banking specifically — sits at the
intersection of three pressures that make the Cloud Engines pitch
**existentially relevant**, not nice-to-have:

1. **Banking secrecy art. 47 BankG.** Disclosure of client information
   without authority is a criminal offence in Switzerland, with personal
   liability up to 250'000 CHF or 5 years' imprisonment for the
   responsible officer. Not "regulatory friction" — actual prison.
2. **FINMA Circular 2023/01 on operational risks** (and the EU AI Act,
   binding on EU-tier banks since 2026): both require a documentable
   lineage of AI-assisted decisions. _What did the model see, what did
   it answer, who acted on it, when._ A contract clause does not satisfy
   this. A cryptographic chain does.
3. **FADP / GDPR cross-border restrictions, post-Schrems II.** Client
   data must process inside CH/EU borders, on infrastructure where the
   customer can independently verify which physical machines handled
   which records. Microsoft Copilot in Azure-Switzerland is a contract;
   the customer cannot independently verify it.

This is why **Swiss private banks have not deployed AI on client data at
scale**. Not because the capability isn't available — every bank's data
science team has been fluent in LLMs for two years — but because no
amount of "we trust Microsoft" closes the gap between contractual data
residency and bank-secrecy criminal liability.

The Cloud Engines proposition closes that gap by making _"prove it
cryptographically"_ the cheapest answer. The four sections below walk
through what that looks like in practice, using the concrete shape of
the bundle in this repo as the worked example.

> **One framing note up front, applies everywhere below.** The AI
> assistant in this bundle calls a **real LLM**. No model weights ship
> inside the canister itself — inference runs on a GPU node attached
> to the same Cloud Engine, reached via a non-replicated HTTPS outcall
> to `POST /v1/agent/run`. The model URL is configured at install time
> via the `PUBLIC_LLM_BASE_URL` env var in the marketplace manifest, or
> rotated at runtime via `set_llm_base_url` (controllers only). The
> earlier version of this bundle shipped a deterministic stub with
> the claim that swapping in a real model would be _a one-canister
> swap, not a re-architecture_. That claim has now been exercised in
> code: the audit chain, the role-gated access, the citations contract,
> the inter-canister call shape — all unchanged. The five-canister
> design and what surrounds the model is still the substantive
> showcase; the model is just no longer a stub. Read everything below
> with that calibration.

---

## 1. Auditability that survives FINMA / the EU AI Act

### The problem

A FINMA examiner asks: _"Show me the chain of decisions for client X.
What did your AI assistant see when advisor Y asked about the portfolio
on date Z, what did it answer, what trade idea followed, who approved
it, and prove that the record I'm looking at hasn't been edited since."_

On Microsoft 365 + Azure-Switzerland, the honest answer is _"We have
application logs in Sentinel, an Outlook export, a SharePoint version
history, and we trust ourselves and Microsoft not to have rewritten
them."_ That is a contract clause and an internal control. It is not
a proof. The same answer to a CFTC, SEC, or Swiss prosecutor's discovery
request takes a week, costs a six-figure law-firm bill, and leaves the
bank exposed if any of those internal logs has gaps or has been edited.

### What Cloud Engines does

Two things compose:

1. **Tamperproof execution at the protocol layer.** Every state transition
   in the bank's canisters is signed by the engine's threshold key — a
   key no single node controls and the operator cannot extract. Any
   response is verifiable against the NNS root key by anyone, anywhere,
   with no relationship to the operator. _The infrastructure is the
   auditor._
2. **Append-only state by construction.** State is replicated across
   every node in the engine. To "edit history" you would have to
   simultaneously corrupt a threshold of independently operated machines
   — nodes registered to different providers, governed by the NNS,
   attested by chain-key signatures. Doable in a thought experiment, not
   in the real world.

### What this app adds on top

Five canisters, **one audit chain**:

```
seq_n.hash = sha256(
    seq_n || ts_ns || on_behalf_of || action_repr || seq_{n-1}.hash
)
```

Every state transition lands here, regardless of which canister did the
work. Concretely, the chain captures:

- Client created, KYC updated, advisor reassigned (`data` writes)
- Meeting added, trade idea proposed, trade idea approved/rejected
  (`data` writes)
- Role granted, role revoked, client assigned to advisor
  (`identity` writes)
- Client record accessed for review / KYC refresh / trade-idea
  preparation (`data` writes — _every read of sensitive data is logged_)
- AI assistant queried, AI assistant responded (with citation IDs)
  (`ai_assistant` writes directly)
- Compliance export run with `[from_seq, to_seq]` (`audit` writes its
  own export event)

The frontend's `/audit` page re-derives the entire chain client-side and
shows green or red depending on whether what the canister returned matches
what it _claims_ to have stored. Three audiences, one artifact:

- **The advisor** can see their own actions tracked.
- **The compliance officer** opens `/admin/compliance`, picks a sequence
  range, downloads a JSON file containing the slice + the engine's
  threshold-signed chain head — and hands it to a regulator.
- **The regulator** receives an answer to "prove nothing has been edited"
  that is not a SOC 2 report, a contract, or a legal opinion. It is a
  hash — verifiable against the NNS root key by their own technical
  staff with no relationship to the bank or DFINITY.

This is the property FINMA, the EU AI Act, and any future AI-in-finance
regulator will increasingly demand and traditional cloud cannot
provide regardless of how the contract is worded.

---

## 2. Banking-secrecy-grade sovereignty

### The problem

A French national lives in Geneva and banks with a Swiss private bank.
Three jurisdictions converge on the same record:

- **art. 47 BankG**: criminal liability if data leaks outside CH borders
  without authority.
- **GDPR + the EU Data Act**: French regulators care about which physical
  machines processed her records, with Schrems II making the legal
  status of contractual EU-only-region claims contestable.
- **The CLOUD Act**: any US parent company can be compelled by US warrant
  to disclose data wherever it lives.

Microsoft 365 in Azure-Switzerland satisfies _none_ of these on its own
— Microsoft is a US parent company subject to US warrants regardless of
the data centre's GPS coordinates. UBS, Pictet, Julius Baer, and every
other Swiss private bank has spent five years arguing this with their
compliance officers, and the answer has consistently been: _"contractual
residency is not bank-secrecy compliant; we cannot deploy AI on client
data."_

### What Cloud Engines does

The engine creator picks specific nodes from a public registry — by
jurisdiction, operator, certifications (ISO 27001, SOC 2, TEE), hardware
spec — and that selection is recorded on chain. Chain-key cryptography
proves the specific subset of nodes the creator picked produced any
given response. The **Swiss Cloud Engines** launched at Davos 2026 are
the canonical reference: exclusively nodes operated by independent
providers in Switzerland and Liechtenstein, satisfying GDPR _and_ Swiss
data-protection law _and_ delivering a verifiable proof, not a contract
clause.

For a Swiss private bank this means:

- Deploy the same `.icp` bundle to a Swiss-only engine. _The bank's
  client data — and the AI that reads it — never touches a machine
  outside Switzerland._
- "Where did the AI run?" gets a verifiable answer, not a clause buried
  on page 47 of the data-processing agreement. The threshold key
  identifies _exactly which nodes_ produced the response, and the
  on-chain registry says where those nodes live and who operates them.
- The CLOUD Act argument disappears. There is no US parent company
  sitting on top of the substrate. The NNS — not a corporation —
  governs the protocol.
- Regulatory portability becomes architectural: the same bundle deploys
  to a UK-only engine for a London branch, an EU-only engine for a
  Luxembourg subsidiary, a US-only engine when the SEC sandbox lands.
  Same code, same audit chain, different jurisdictions.

### What this app adds on top

Nothing — and that's the point. The bundle is **portable across engines**
without code changes. Move it from a low-trust shared engine (for
staging) to a Swiss-only dedicated engine (for production) by
re-uploading the same `.icp`. The audit chain travels with it. The
identity, audit, data, and AI canisters all run on the same engine, so
inter-canister calls stay inside the same threshold-signed boundary —
no data leaves the bank's compute envelope at any layer.

---

## 3. Governance you can name (and an AI you can defend)

### The problem

Under the EU AI Act and FINMA's emerging AI guidance, the bank must be
able to answer:

- _Who can change the AI's behaviour?_
- _Who can change the data the AI sees?_
- _Who can change the audit record of what the AI did?_
- _Who can change the rules about who has access to which clients?_

If the answer is "our IT department, with logging" or "Microsoft, under
contractual terms", that's not enough — the regulator wants a structural
answer, not a procedural one. _The properties of the system itself_,
not "we promise our staff are honest."

### What Cloud Engines + this five-canister design does

> The whitepaper's commercial model has three actors: **node providers**
> (supply machines), **engine creators** (assemble engines, sell packages),
> and **app deployers** (the bank buying capacity with a credit card).
> Revenue is split transparently: 80% to node providers as ICP/USD
> stablecoin, 20% burned as cycles into ICP tokenomics, with the engine
> creator retaining a margin in between. The slicing below is the
> _governance_ view of the same architecture — it adds the NNS as the
> protocol-level governance layer that sits beneath all three commercial
> actors and that none of them controls.

Three layers of governance, none of which can collude silently:

1. **Node providers** are paid for keeping their machines running. They
   are not the bank. They cannot read or modify canister state. If one
   goes rogue or offline, the threshold mechanism keeps execution honest.
2. **The engine creator** picks nodes, sets pricing, runs the engine
   side. They _can_ remove nodes and reshape the engine — that is part
   of the live-ops model — but they cannot read canister state or
   rewrite past state without breaking the chain-key signatures everyone
   else can verify.
3. **The NNS** — the network's governance system — controls which nodes
   are valid, approves protocol upgrades, and sets network parameters.
   No single party, including DFINITY, controls the NNS. _DFINITY
   contributes to ICP. DFINITY does not control it._

This bundle adds a **fourth layer of structural governance inside the
application itself**, by splitting the workload across five canisters
with strict separation of concerns:

- `identity` owns role grants and advisor↔client assignments. It has no
  knowledge of clients, portfolios, or audit logs. Compromising it
  changes who can do what — but not what was done.
- `audit` owns the hash-chained log. It has no knowledge of clients,
  portfolios, or roles. Compromising it requires touching a separate
  canister with separate controllers — and any tampering invalidates
  every subsequent hash. Compromising it doesn't let you change the
  past; it only lets you stop logging the future, which the chain head
  exposes immediately.
- `data` owns clients, portfolios, meetings, trade ideas. It has no
  knowledge of roles or hash chains. It calls `identity` for authz and
  `audit` for logging. Compromising it without touching the others
  means changing data without leaving an audit trail — and the chain
  exposes the gap.
- `ai_assistant` owns the AI orchestrator. It has no direct read access
  to any data; it issues inter-canister calls _under the user's
  identity_ and goes through the same authz that a human user would.
  Compromising it doesn't grant new privileges.
- `web_frontend` owns nothing but the SPA. It cannot read or write
  state — it just renders.

For a regulator's question _"who can change the AI's behaviour?"_ the
structural answer is now: a canister controller of `ai_assistant`, and
nobody else. _"Who can change what the AI saw?"_: a canister controller
of `data`. _"Who can change the audit record?"_: a canister controller
of `audit`, and even they cannot change the past — only the future.
_"Who can change the role grants?"_: a canister controller of
`identity`. **Each question maps to a different canister and a different
controller set, and any change leaves an entry in the audit log of one
of the others.**

This is structurally stronger than what a Microsoft 365 + Azure deployment
can offer. It is also the part that most clearly justifies the move from
2 canisters to 5 — the separation isn't architectural fashion, it's a
product property the regulator will eventually ask for.

### Specific to AI: the EU AI Act explainability angle

EU AI Act Article 13 requires _high-risk AI systems_ (which includes
financial advisory) to provide users with a description of the system's
characteristics, capabilities, limitations, and the data on which it
was trained or operates.

This bundle's AI assistant returns answers with **inline citations to
record IDs** — `[#0]`, `[#1]`, `[#2]` mark the records the answer was
derived from. Click any citation, see the underlying client / portfolio
/ meeting / trade idea. The audit chain captures every citation. _What
the AI saw is provable per query, and the proof is verifiable by the
client whose data it was._ For a regulator that's structural
explainability, not a model card.

---

## 4. The boring operational stuff that kills enterprise AI projects

### The problem

A regulated AI deployment in a private bank needs:

- 24/7 uptime on the data plane and the inference plane
- A security team to handle the SOC 2, ISO 27001, FINMA-aligned audits,
  patch CVEs, rotate credentials, configure firewalls, configure WAF,
  configure DDoS mitigation
- A devops team for backups, replicas, region failover, disaster
  recovery drills
- Periodic penetration tests — both on the data plane and the AI
  inference plane
- A KMS, an HSM, an IAM policy that auditors believe in
- For the AI plane specifically: model-versioning, prompt-version
  control, drift monitoring, red-team testing pipelines
- An MLOps team to run all of the above

For a Swiss private bank with 200–500 advisors, this is a 7-figure
ongoing cost before the first AI-assisted client interaction. It is the
single biggest reason _capable_ private banks have not deployed AI on
client data: the project economics don't pencil out under the
compliance overhead.

### What Cloud Engines does

The whitepaper's substantive claim (§1.3): _"an app deployer does not
need a dedicated security team or system administrator, because the
protocol enforces integrity and availability by design."_ Concretely,
the protocol handles:

- **Replication** across all engine nodes.
- **Consensus.** No deadlocks, no split-brain, no Paxos to misconfigure.
- **Key management.** The threshold key is generated by the protocol,
  never exposed to any node. There is no HSM to rotate.
- **Software upgrades.** Roll a canister upgrade through `icp deploy`;
  the protocol applies it atomically across the replica set with no
  downtime. Each of the five canisters can upgrade independently — AI
  weights without touching client data, role schema without touching
  audit.
- **Persistence.** Orthogonal: state survives node replacement at the
  protocol layer. No backup script to run, no restore drill to fail.
- **OS / patch surface.** There isn't one to patch. There are no SSH
  ports, no IAM users, no Bastion hosts.

What's left for the bank is: write the application, configure the
engine selection (Swiss nodes, ISO 27001, TEE), and answer support
tickets.

For the AI plane specifically, the multi-canister design lets the bank
swap LLM models — eventually swap the inference layer onto a TEE/GPU
node when those land — without touching client data, without
re-certifying the data plane, and without going back through procurement.

### What this app adds on top

The bundle is **five canisters, one manifest, ~2 800 LoC of Rust + a
SvelteKit app**. Add a real LLM, real KYC document storage, and real
trade execution and you still don't have an ops team, because there's
still nothing to ops.

---

## What this isn't

Honesty matters for the pitch to land:

- **Cloud Engines does not make the legal question go away.** A FINMA
  examiner can still decide a particular use of AI on client data is
  out of scope for this bank's authorisation. What changes is that you
  have a defensible answer to "where did the data live?", "who could
  have edited it?", and "what did the AI see?". The substantive law is
  still up to the bank's lawyers.
- **The audit chain only covers what the canisters see.** If a model is
  hallucinating, the citations will faithfully record what records the
  model saw — but the answer can still be wrong. Provable provenance
  doesn't make a wrong recommendation right. It only makes it
  forensically explicable, which is what the regulator wants.
- **The LLM call is non-replicated.** The HTTPS outcall to the GPU
  node is made by a single engine replica, not threshold-signed by the
  whole engine. This is necessary because LLM completions are
  non-deterministic, and a replicated outcall would have every replica
  in the engine run the same prompt, get different completions, and
  see them all rejected by consensus. The consequence is that the
  _model's free-text response_ carries one-of-N trust, not
  threshold-of-N. Everything _around_ the call — the records the model
  was given (replicated query results from `data` under the user's
  identity), the citations the canister builds from those records,
  the `AssistantQueried` and `AssistantResponded` audit-log entries
  that capture both the prompt context and the response — remains
  threshold-signed and tamperproof. For the regulator's question
  _"what records did the AI see and what did it produce"_, both halves
  are still provable; the tamperproof guarantee on the second half is
  one-of-N rather than threshold-of-N. A future TEE/GPU subnet
  (the whitepaper hints at SEV-enabled subnets — §2.2 _vs.
  decentralised compute platforms_) would close this gap by giving
  the inference itself a hardware attestation; the canister-side
  contract wouldn't have to change.
- **Engine-level replication is lower than mainnet.** This is a
  deliberate trade-off in the whitepaper: 3–4 nodes is _strictly more
  resilient than a single-operator cloud deployment_, but weaker than a
  full NNS-vetted subnet. For private banking this is almost always
  still the right choice — but the operator should pick the spec class
  with eyes open.
- **Engines are isolated from mainnet XNet.** Mainnet canister signatures
  (II, ckBTC, ckETH) work — but inter-engine canister calls don't exist
  today. Multi-engine deployments need to coordinate at the application
  layer.
- **The EU AI Act compliance work is not finished by this architecture.**
  The bundle's structure makes _explainability_ and _data-lineage_
  arguments easier; it does not by itself satisfy bias-audit
  requirements, the conformity-assessment process for high-risk
  systems, or the documentation-management obligations. The bundle is
  necessary, not sufficient.
- **No trading or settlement is connected.** This bundle records trade
  _ideas_ and their approval status — it does not execute trades, does
  not move cash, does not settle. Connecting custody is a real product
  decision and a real attack surface; not in scope here.

---

## Engineering notes — what this build is and isn't

> This section is written for internal readers and technical evaluators.
> If you are reading this as an external pitch you can skip it; the rest
> of the document is the argument. This part is the honest receipts.

The bundle in this repo is a showcase, not a product. There are five
substantive engineering shortcuts that anyone looking to "ship this for
real" needs to know about (down from six — §1 used to be _"the AI is a
transparent stub"_; that one has been retired by PR #1, and §1 now
documents what landed instead so this section keeps the original
numbering for anyone cross-referencing the bullet doc). Two of the
remaining shortcuts mirror the Vici-bundle shortcuts (same demo-grade
work); three are specific to Privatim.

### 1. The AI is a real on-engine LLM call (formerly a transparent stub).

The earlier version of this section made the claim that swapping a
stub for a real model would be a one-canister change. That claim has
now been _exercised_ in code: PR #1 (`feat(assistant): adding AI
integration`) replaced the deterministic Rust synthesiser with a real
HTTPS-outcall LLM client. The audit chain, role gating, inter-canister
calls, and citations contract all stayed the same. The diff was almost
entirely confined to `ai_assistant/src/lib.rs`.

What now happens on every `ask`:

1. **Phase 1 — gather records.** The `ai_assistant` canister calls
   `data._for(end_user)` under the user's identity (the
   update-mode endpoint family added for exactly this lane — see §4
   below) to fetch only the records the caller is already authorised
   to see. This is the same authz boundary the stub had.
2. **Phase 2 — call the model.** It builds a preamble bundling the
   role prompt, the per-intent task description, a citation index, and
   the records as JSON. It POSTs
   `{ prompt, preamble, context, max_turns: 30 }` to
   `{PUBLIC_LLM_BASE_URL}/v1/agent/run` via
   `ic_cdk::management_canister::http_request` with
   `is_replicated: false` (see "What this isn't" in the external
   pitch for the trust-model consequence).
3. **Phase 3 — audit both sides.** It records `AssistantQueried`
   (with the citation IDs in the prompt context) and
   `AssistantResponded` on the `audit` canister so both halves of the
   exchange land on the same hash chain as the rest of the bank's
   data. Inference timing on the response covers data fetches + LLM
   outcall + audit appends, so the badge in the UI isn't faked.

**Citations remain canister-built.** The model is told to refer to
records by `[#N]` markers, but the `AssistantCitation[]` array
returned to the frontend is constructed from the data the canister
fetched, never from anything the model said. A hallucinating model
can't invent citations — at worst it can mis-cite an existing one,
which the frontend renders as a broken link.

Substantive AI engineering is now concentrated in `ai_assistant/src/lib.rs`'s
seven `gather_*` functions (each shaping the records the model is
allowed to see for a given intent) plus a single shared `call_llm`.
The model URL is set at install time via `PUBLIC_LLM_BASE_URL` in the
marketplace manifest, or rotated at runtime via `set_llm_base_url`
(controllers only). For local dev — where `icp deploy` doesn't read
the marketplace manifest — `/admin/bootstrap` has an LLM-endpoint
section that saves directly to the canister.

If a stakeholder evaluates the model itself and concludes "this isn't
as smart as GPT-5" they have read the demo wrong. The model is now a
swappable component; the pitch is the architecture _surrounding_ that
component, which has been demonstrated to be model-agnostic.

### 2. Resolution of trade ideas is creator-trusted.

Trade ideas have a status (`Draft`, `Approved`, `Rejected`, `Executed`),
but state transitions are gated only by "is the caller the client's
primary advisor or an Admin?". There is no four-eyes approval, no
compliance-must-sign-off-before-Approved, no real audit trail of who
proposed vs. approved. The audit chain captures the status change but
not the workflow.

For a real product, this is the next ~1–2 days of work: introduce a
`pending_approval` state, require `set_trade_idea_status(Approved)` to
be called by a compliance principal _different from_ the proposer,
record both events. The audit log already has the data shape for it.

### 3. State persistence is `ic_cdk::storage::stable_save`, not `ic-stable-structures`.

Each canister keeps state in a `RefCell<State>` in heap. `pre_upgrade`
serialises to candid + writes a single blob to stable memory.
`post_upgrade` reads it back. Survives `icp deploy` upgrade and
schema-compatible state changes. Does **not** survive
`icp deploy --mode reinstall`, canister loss, or state larger than the
wasm heap. Acceptable for a showcase, unacceptable for production. First
refactor when shipping for real: move `audit.audit` (the hash-chained
log) and `data.{clients, portfolios, meetings, trade_ideas}` to
`StableBTreeMap` / `StableVec`. ~1 day of mechanical work.

### 4. Authz at the data canister boundary — two read paths.

The data canister exposes its reads through **two parallel
endpoint families**:

- **Composite queries** (`list_clients`, `get_client`, etc.) — used
  by the frontend. They issue inter-canister queries to the identity
  canister to scope visibility before returning (Compliance/Admin
  see all, Advisors see only assigned clients).
- **Update-mode `_for(end_user)` endpoints** — used by the AI
  assistant from inside its `ask` update. Same authz logic but the
  end user's principal is passed explicitly as an argument, and
  data verifies `caller == ai_assistant_canister` before trusting
  the argument.

Why two paths: composite queries can be called as queries (frontend
case) but **not from an update context** (IC0527 — "Composite query
cannot be called in replicated mode"). The AI assistant's `ask` is
an update because it writes audit entries, so it can't invoke
composite queries on data. The `_for` endpoints exist for that
exact lane.

The AI principal is auto-discovered: `ai_assistant.init` calls
`identity.register_ai_assistant_self()` (TOFU — first canister to
claim the slot wins) via `ic_cdk::futures::spawn`. The data canister
then fetches that principal lazily from identity on first `_for`
call and caches it. No human bootstrap step.

This closes the showcase-grade authz hole that existed when
`data.list_clients` was a single regular query: a malicious
authenticated user calling `data.list_clients` directly via `dfx`
now hits the composite query, which routes through identity and
filters their result set the same way the frontend would. The AI
path is gated on canister identity, not role, so an attacker
can't impersonate the AI to forge `end_user`.

### 5. Inter-canister composite-query latency is not cached.

Every read on `data.list_clients` issues two inter-canister query calls
to `identity` (`has_role(p, Compliance)` and `has_role(p, Admin)`).
Every `data.get_client` does the same plus an `is_assigned`. On a local
replica this is ~50–200ms per hop, so a single page load on the client
detail page hits ~500ms of inter-canister latency. On production engines
it's faster but still measurable.

For a showcase this is fine. For production, the right pattern is a
read-through cache in `data` keyed by `(principal, role)` with a 5–10s
TTL. Cache invalidation on role grant/revoke is "wait for TTL" —
acceptable because role grants are rare and the staleness window is
small. ~2 hours of work if/when it becomes the demo's bottleneck.

### 6. Local-dev bootstrap is manual.

Inter-canister wiring (telling `audit` who its writers are, telling
`data` and `ai_assistant` where to find `identity` and `audit`) is real
configuration. On Cloud Engines installs, the manifest's
`dependencies` block makes the installer auto-inject
`PUBLIC_CANISTER_ID:<dep>` env vars, which the canister `init`
functions can read. **This is not yet wired up in our `init`s** — they
read no env vars. Locally there's no installer, so we get the same
result either way: a one-shot wiring step.

The `/admin/bootstrap` page runs all 10 wiring calls in one click,
which is acceptable for a demo. For a real install the right move is to
add `ic_cdk::env_var_value("PUBLIC_CANISTER_ID:<dep>")` reads into
each canister's `init` so the wiring is automatic on Cloud Engines _and_
the page becomes a fallback only. ~half a day of work.

### 7. Synthetic advisor principals + Admin-sees-all bootstrap.

Seeded clients are owned by 6 deterministic synthetic advisor principals
that no human can sign in as. The first authenticated principal becomes
Admin via `identity.bootstrap_admin`, and Admins see all clients
regardless of assignment, so the demo populates immediately.

For a real bank we'd seed an empty workspace and let the bank's actual
advisors create clients via `data.create_client`, which auto-assigns
the creating principal. The synthetic-principals approach is purely a
demo-quality-of-life shortcut and should not survive contact with
production.

### Smaller things worth knowing

Not blocking, but worth surfacing so internal readers don't trip on
them:

- **KYC document storage IS shipped (round 2).** A separate
  `documents` canister stores client-side AES-256-GCM encrypted
  blobs (KYC PDFs, contracts, signed mandates). Each document gets a
  fresh random 256-bit key generated in the user's browser; the
  canister sees only ciphertext + IV. **The engine creator with full
  canister-controller access cannot decrypt** — there's nothing on
  the canister to decrypt with. The audit chain captures
  `DocumentUploaded` / `DocumentAccessed` / `DocumentDeleted` events
  with the plaintext SHA-256 (so compliance can verify a document
  later if the user provides a copy), but never the key or the
  plaintext itself. Tradeoffs (also showcase-grade): no key escrow,
  no cross-device sync, no "share with another advisor" flow — the
  uploader's browser localStorage is the only place the key lives.
  Production answer is vetkeys when the IC ships them stable, or
  threshold-signed key wrapping among advisors.
- **The audit canister is callable by writers via plain `update`.**
  A writer canister (data, ai_assistant, identity) calls `audit.record`
  and the entry lands. There's no per-call attestation, just an
  allowlist of writer principals. Adequate for a showcase; a real
  product would benefit from each writer signing its action with a
  per-canister key so a compromised writer's identity is provable in
  the chain.
- **The frontend is fully CSR** (`ssr=false; prerender=false`). The
  asset canister serves one `index.html`; the SPA hydrates client-side
  and reads runtime config from the `ic_env` cookie planted by the
  Cloud Engines installer. This is the only correct mode for a
  packageable engine app today; calling it out so nobody "fixes" it
  back to SSR.
- **Two canisters, multi-engine — not yet supported by the IC.**
  Inter-engine XNet doesn't exist. So if a global bank wanted
  jurisdiction-per-engine deployments _with shared cross-border
  reporting_, they'd need to coordinate that at the application layer.
  Out of scope for this bundle.

---

## The general shape

If you strip out "private banking" and replace it with anything else
where a regulator / auditor / counterparty needs to verify what an AI
saw, what data it processed, and where the inference happened, the
argument is structurally the same. Replace "trade idea" with:

- _diagnosis recommendation_ (regulated healthcare)
- _claim adjudication_ (insurance, with EU AI Act implications)
- _credit decision_ (consumer lending, ECOA / GDPR Article 22)
- _legal-document review_ (data leaks via OpenAI fine-tunes are an
  active firm-malpractice concern)
- _intelligence assessment_ (defence / national security)
- _patent-strategy advice_ (industrial IP)
- _board-meeting summary_ (governance, especially in regulated
  industries)

In each case the operator is being asked to prove a chain of events —
including AI-assisted ones — to a sceptical third party, on a substrate
the third party doesn't control. **Cloud Engines is the substrate that
makes _"prove it cryptographically"_ the cheapest answer instead of the
most expensive one.**

Private banking is the right first vertical because the existential
liability is highest and the tolerance for vagueness is lowest. The
patterns that work here generalise downward.

---

## TL;DR for the pitch deck

> **Privatim** runs entirely on a Swiss-only Cloud Engine. Five canisters
> — identity, audit, data, ai_assistant, web_frontend — that strictly
> separate roles, hash-chained audit, client records, AI orchestration,
> and the SPA. Every read of a client record, every AI prompt, every
> AI response, every role grant, every trade-idea state change is
> appended to an on-chain hash-chained audit log that any user, auditor,
> or regulator can re-verify in their own browser, against a key the
> operator does not control, on nodes located in a jurisdiction the
> operator picked and the network's governance system can prove.
>
> There is no PII database to leak, no admin team to compromise, no
> KMS to rotate, no Microsoft to subpoena, no parent company a court
> can compel into rewriting history.
>
> The same `.icp` bundle ports across engines. The AI canister can be
> swapped without touching client data. The audit chain is the single
> source of truth a FINMA examiner can verify with a `sha256` and the
> NNS root key.
>
> _The infrastructure is the auditor. The infrastructure is the
> AI's compliance officer._
