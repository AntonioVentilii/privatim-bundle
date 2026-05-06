# Privatim — sovereign AI for private banking

A simplified, single-bundle wealth-management showcase for the Internet
Computer's Cloud Engines marketplace. Designed for Swiss private banks where
client data **must not leave the bank's compute**: banking secrecy art. 47
BankG, FINMA Circular 2023/01 (Operational Risks), and the FADP / GDPR
cross-border restrictions all converge on the same constraint.

The pitch is in [`PITCH.md`](./PITCH.md). The packaging walkthrough is in
[`PACKAGING.md`](./PACKAGING.md). This file is the dev workflow.

## What it does

- **Client records, portfolios, meeting notes, trade ideas.** A minimal
  CRM-shaped surface that wealth advisors actually recognise.
- **Role-gated access.** Three roles — `Advisor`, `Compliance`, `Admin` —
  granted by a controller from `/admin/roles`. Advisors see only the
  clients they're assigned to. Compliance sees the audit log without
  client PII unredacted.
- **AI assistant scoped to your book.** A separate canister
  (`ai_assistant`) that answers structured queries over the bank's data
  via inter-canister calls under the caller's identity. **v1 is a
  transparent stub** — UI labels it "Stub LLM, real inference deploys
  on this engine's GPU node". Real prompts, real source citations,
  honest about what it is.
- **Hash-chained audit log of every read, every write, every AI prompt
  and response.** That's the FINMA-shaped lineage. The frontend's
  `/audit` page re-verifies the chain in the browser; `/admin/compliance`
  exports a signed slice for handing to the regulator.
- **Internet Identity** at the edge. No PII honeypot, no email-and-password
  database to subpoena.

## Layout

```
.
├── README.md                  # this file (dev workflow + overview)
├── PACKAGING.md               # how to ship the .icp bundle to a Cloud Engine
├── PITCH.md                   # external pitch + internal engineering notes
├── icp.yaml                   # icp-cli project file
├── farm.manifest.json         # for testnet farm + Cloud Engines (II = s24we-…)
├── local.manifest.json        # for a local icp-cli network
├── icons/                     # one SVG per canister + app.svg
├── app_backend/               # Rust canister (clients, portfolios, audit, roles)
├── ai_assistant/              # Rust canister (stub LLM today, real LLM v2)
└── web_frontend/              # SvelteKit + Tailwind, adapter-static
```

## Build & package

For the everyday dev flow, see _Development_ below — you do **not** need
to build a `.icp` archive to iterate.

When you actually want to ship the app to a Cloud Engine, the full
walkthrough is in [`PACKAGING.md`](./PACKAGING.md). Short version:

```bash
# one-time: install the fork that ships the `package` subcommand
cargo install --git https://github.com/NikolaMilosa/icp-cli.git \
  --branch nim-packaging-wip --locked --force icp-cli
cp ~/.cargo/bin/icp ~/.cargo/bin/demo-icp && rm ~/.cargo/bin/icp

# every release:
pnpm --prefix web_frontend build
demo-icp build
demo-icp package create -m ./farm.manifest.json -o ./privatim.icp -e local
# → upload privatim.icp via the Cloud Engines console.
```

## Development

There are **three independent iteration loops**. Boot the replica + deploy
once, then leave the dev server running and iterate on whichever side
you're touching.

### Loop 0 — boot the local replica (do this first)

```bash
icp network start
icp network status                 # confirm Gateway Url (default 127.0.0.1:8000)
icp deploy                         # builds + deploys all three canisters

# Wire the frontend dev server to the freshly-deployed canisters:
{
  echo "VITE_CANISTER_ID_APP_BACKEND=$(icp canister status app_backend | awk '/^Canister Id:/ {print $3}')"
  echo "VITE_CANISTER_ID_AI_ASSISTANT=$(icp canister status ai_assistant | awk '/^Canister Id:/ {print $3}')"
} > web_frontend/.env.local
```

### Loop 1 — frontend iteration

```bash
cd web_frontend
pnpm install                       # once
pnpm dev                           # http://localhost:5173, hot reload
pnpm check                         # svelte-check + tsc, must be 0 errors
pnpm build                         # → web_frontend/dist (production bundle)
```

`vite.config.ts` proxies `/api/*` to `http://127.0.0.1:8000` (the
`icp 0.2.6` pocket-ic gateway port; override with `IC_REPLICA_URL`).

### Loop 2 — backend iteration

For Rust changes:

```bash
icp deploy app_backend             # rebuild + reinstall just the data canister
icp deploy ai_assistant            # rebuild + reinstall just the AI canister
```

The dev server picks up the new candid surface on the next page reload.
If you changed function signatures, regenerate the IDL bindings in
`web_frontend/src/declarations/<canister>.{idl,types}.ts` to match —
right now those are hand-written, so updates are manual.

### Demo data

The canister seeds **synthetic Swiss private-banking data** on every
fresh `init` (12 advisors, 30 clients with KYC + portfolios, ~200 meetings
and trade ideas). All synthetic — no real PII. Re-seed without
redeploying via `icp canister call app_backend reset_demo` (controller-only).

## Why this is "simplified"

A real private-banking system has KYC document workflows, custodian
integrations, market-data feeds, trade execution, settlement, regulatory
reporting, multi-tenancy, MIFID-II suitability checks, and a security
team larger than this entire codebase. **None of that is here.** What's
here is the smallest amount of code that demonstrates the _sovereignty +
on-engine-AI + auditable lineage_ story Cloud Engines is built around,
shaped against a recognisable wealth-management surface.
