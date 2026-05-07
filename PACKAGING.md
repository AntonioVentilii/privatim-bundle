# Packaging — turning this repo into an uploadable `.icp`

> Everyday development never needs this. The `.icp` archive is only
> built when you want to upload the app to a Cloud Engine. For local
> iteration, follow the _Development_ section in
> [`README.md`](./README.md).

This doc is a complete, self-contained walkthrough. By the end you'll
have a `privatim.icp` file in this repo's root that you can hand to a
Cloud Engines console.

---

## What gets produced and why

A `.icp` archive is the unit of distribution for a Cloud Engines app.
It contains:

- The compiled WASMs for every canister listed in `farm.manifest.json`
  — here: `identity.wasm.gz`, `audit.wasm.gz`, `data.wasm.gz`,
  `ai_assistant.wasm.gz`, plus the asset-canister WASM that serves the
  static frontend.
- The static frontend bundle (everything in `web_frontend/dist/`).
- The manifest itself, which is the **install contract** — it tells the
  engine's installer how to spin the canisters up, what `init_arg` to
  pass each, and what runtime config (II canister ID, dependency
  canister IDs) to inject into the asset canister via the `ic_env`
  cookie.
- App icons (`icons/app.svg` plus one per canister:
  `identity.svg`, `audit.svg`, `data.svg`, `ai_assistant.svg`,
  `web_frontend.svg`) and any optional screenshots.

The same `privatim.icp` works on any engine without rebuild — runtime
config is injected by the installer at install time, not baked into the
bundle. Move it from a generic testnet farm to a Swiss-only engine by
re-uploading the same artifact.

---

## One-time setup — install `demo-icp`

The `package` subcommand is a preview feature that the public
`icp 0.2.6` (the one on `npm`) does not ship. It lives on the
`nim-packaging-wip` branch of a fork. Install it as `demo-icp`,
**alongside** the public `icp`, so everyday dev keeps using the stable
release and only packaging hits the fork.

```bash
# 1. cargo install the fork directly from GitHub.
#    - The repo contains multiple binary crates, so we must pick `icp-cli`
#      explicitly.
#    - `--force` lets this re-run safely if you've already cargo-installed
#      a previous `icp` (which the rename in step 2 wipes anyway).
#    Result: a binary at ~/.cargo/bin/icp.
cargo install --git https://github.com/NikolaMilosa/icp-cli.git \
  --branch nim-packaging-wip \
  --locked \
  --force \
  icp-cli

# 2. Rename to demo-icp so it doesn't shadow your stable `icp`,
#    then remove the cargo-installed shadow.
cp ~/.cargo/bin/icp ~/.cargo/bin/demo-icp
rm ~/.cargo/bin/icp

# 3. Confirm both binaries resolve to different things.
which icp        # → ~/.npm-global/bin/icp        (public 0.2.6)
which demo-icp   # → ~/.cargo/bin/demo-icp         (the fork)
icp --version    # 0.2.6
demo-icp --help  # should list `package` as a subcommand
```

If `~/.cargo/bin` isn't on your `$PATH`, add this to `~/.zshrc` and
reload your shell:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

If `cargo install --locked` fails on a dependency mismatch, retry
without `--locked`. If it fails on a Rust toolchain version, the fork's
`rust-toolchain.toml` will tell you what's required — install via
`rustup install <version>` and retry.

---

## Build the bundle (three commands)

Run from the repo root:

```bash
# 1. Frontend bundle. Same script you use in everyday dev — it produces
#    web_frontend/dist/, which the manifest's `web_frontend.asset_dir`
#    points at.
pnpm --prefix web_frontend install     # only first time, or after deps changed
pnpm --prefix web_frontend build       # → web_frontend/dist/

# 2. Canister WASMs. Use the fork here so the artifacts go to the
#    layout `package create` expects to find them in.
demo-icp build                          # → .icp/cache/artifacts/{identity,audit,data,ai_assistant}/

# 3. Produce the .icp archive.
demo-icp package create \
  --manifest ./farm.manifest.json \
  --out ./privatim.icp \
  -e local
```

That's it. After step 3 you have:

```bash
ls -lh privatim.icp
# -rw-r--r-- 1 you you ~5M May  6 15:00 privatim.icp
```

Slightly larger than a 2-canister bundle (Vici was ~3M) — five WASMs
plus the SvelteKit bundle. Still well under the typical Cloud Engines
package-size limit.

> **`-e local` is the packaging environment, not the install target.** It
> tells the packager which built artifacts to pick up from
> `.icp/cache/artifacts/`. The same `.icp` then installs against any
> engine — local, farm, mainnet, custom domain — without rebuild.

---

## Sanity-check before uploading

The packager validates the manifest and asset paths during step 3, so
most problems show up there. A few extra checks worth running:

```bash
# Confirm the archive is well-formed
file privatim.icp                       # should report a known archive type

# Sanity-check the manifest one more time — IDs and env-var prefixes are
# the most common drift between dev and ship time
cat farm.manifest.json | python3 -m json.tool | head -50
```

Specifically:

- **`PUBLIC_II_CANISTER_ID`** must be the engine's II canister ID. For
  the standard testnet farm that's `s24we-diaaa-aaaaa-aaaka-cai` —
  already set in this repo. For a Swiss-only engine the engine creator
  may run their own II instance, in which case override before step 3.
- **`canisters[*].dependencies`** must reflect the actual dependency
  graph. In this bundle: `identity` has no deps, `audit` depends on
  `identity`, `data` depends on `identity` and `audit`, `ai_assistant`
  depends on all three, `web_frontend` depends on all four. The
  installer uses this for both deploy ordering and `ic_env` cookie
  population.
- **`main_canister`** should be `web_frontend`. That tells the engine
  which canister gets the public URL and the `__META_MAIN_CANISTER`
  flag.

---

## Upload to a Cloud Engine

Open the engine's console (the marketplace UI on the engine creator's
domain) and use the **Upload `.icp` package** flow. Hand the file
`privatim.icp` over.

The installer then runs, driven entirely by `farm.manifest.json` inside
the archive. Roughly:

1. **Validate** the manifest and signatures.
2. **Topologically sort canisters by `dependencies`:**
   `identity` → `audit` → `data` → `ai_assistant` → `web_frontend`.
3. **Create + install + init each canister in order.** Each `init_arg`
   from the manifest fires the canister's `#[init]` function. For
   Privatim:
   - `identity` init: empty state, ready for first-login bootstrap.
   - `audit` init: empty state.
   - `data` init: **seeds 12 synthetic Swiss clients with portfolios,
     meetings, and trade ideas.** This is the demo data baked into the
     wasm.
   - `ai_assistant` init: empty state, ready for `set_data_canister` /
     `set_audit_canister` wiring.
4. **Inject env vars** on each canister, including the auto-populated
   `PUBLIC_CANISTER_ID:<dep>` for every declared dependency. The asset
   canister gets `PUBLIC_II_CANISTER_ID` from the manifest plus
   `PUBLIC_CANISTER_ID:identity` / `:audit` / `:data` / `:ai_assistant`
   from the dependency graph.
5. **Mark `web_frontend` as the main canister** so the engine routes
   the public URL to it.

When the install finishes the engine console shows all five canisters
as `Running` and gives you a public URL.

---

## Post-install — what to expect

Open the URL the engine gave you. The first load should:

- Plant the `ic_env` cookie on the response to `/`.
  DevTools → Application → Cookies should show
  `ic_env=PUBLIC_CANISTER_ID:identity=…&PUBLIC_CANISTER_ID:audit=…&PUBLIC_CANISTER_ID:data=…&PUBLIC_CANISTER_ID:ai_assistant=…&PUBLIC_II_CANISTER_ID=s24we-…`
  — five entries, all readable client-side by `ic-env.ts`.
- Hydrate the SPA. The home page renders **the marketing copy** ("A
  workspace your compliance officer can sign off…"), with a _Sign in
  with Internet Identity_ button. No client data shown yet because no
  user is authenticated.

Click _Sign in with Internet Identity_. The frontend's `ii.ts` derives
the engine's II URL by swapping the canister-ID subdomain in the current
URL for the `PUBLIC_II_CANISTER_ID` value, redirects there, and brings
you back authenticated.

After login, the frontend automatically calls `identity.bootstrap_admin`
on your principal. **You become Admin and Advisor.** Roles render in
the header pill.

### One-time bootstrap

The first time the bundle is installed on a fresh engine, you need to
wire the inter-canister principals once:

1. Navigate to **`/admin/bootstrap`** (visible only when you have the
   Admin role).
2. Click **_Run bootstrap_**. The page issues 10 calls in sequence:
   - `identity.bootstrap_admin` (idempotent — fails silently if you
     beat it to it during login)
   - `audit.set_identity_canister(identity)`
   - `audit.admit_writer(data)`
   - `audit.admit_writer(ai_assistant)`
   - `audit.admit_writer(identity)` — for role/assignment events
   - `data.set_identity_canister(identity)`
   - `data.set_audit_canister(audit)`
   - `ai_assistant.set_data_canister(data)`
   - `ai_assistant.set_audit_canister(audit)`
   - `identity.admit_ai_assistant(ai)`

Each step shows a green ✓ or red ✗ inline. After all green, the bundle
is fully wired.

> **Note for production engines:** in a future revision the canister
> `init` functions will read the `PUBLIC_CANISTER_ID:<dep>` env vars
> auto-injected by the installer, eliminating the manual bootstrap
> entirely. The bootstrap page will stay as a fallback. Tracked in
> [`PITCH.md` — Engineering notes §5](./PITCH.md#5-local-dev-bootstrap-is-manual).

### Demo flow

Then:

- Go to `/clients` — you see all 12 demo clients (Admin sees all,
  regardless of advisor assignment).
- Click any client (e.g. _Müller Holdings AG_). Detail page renders;
  in the background, `data.record_client_access` posts a
  `ClientAccessed` entry to the audit chain.
- Open `/assistant`, pick the same client, ask _"Portfolio overview"_.
  The canister fetches the client's records under your identity, posts
  them as context to the on-engine LLM (`PUBLIC_LLM_BASE_URL`), and
  renders the model's answer with canister-built citations like
  `[#0]`, `[#1]`, `[#2]`. The badge shows the actual end-to-end
  inference time (data fetches + LLM outcall + audit appends). Two new
  audit entries are written: `AssistantQueried` and `AssistantResponded`.
- Open `/audit`. The hash-chain re-verifier runs in your browser. The
  badge should go **green** ("Chain intact — N of M entries
  verified").
- Open `/admin/compliance`. Pick a sequence range, click
  _Export & download JSON_. A `privatim-audit-FROM-to-TO.json` file
  downloads, containing the slice + the engine's chain head hash.
  This is the "hand it to FINMA" artifact.

---

## Updating an already-installed app

The bundle is **immutable** — once uploaded, an `.icp` archive is the
artifact for that version of the app. To ship a fix:

1. Make the change locally.
2. Rebuild from scratch:
   `pnpm --prefix web_frontend build && demo-icp build && demo-icp package create -m ./farm.manifest.json -o ./privatim.icp -e local`.
3. Bump `application_version` in `farm.manifest.json` (e.g. `0.2.0` →
   `0.2.1`) so the engine console shows a meaningful version diff.
4. Upload the new `privatim.icp`. The engine console offers
   **upgrade in place** (preserves canister state — `pre_upgrade` /
   `post_upgrade` fire, clients/portfolios/meetings stay where they
   are, the audit chain continues from where it left off) versus
   **reinstall** (wipes state, runs `init` again — the demo clients
   re-seed, the audit chain restarts from `seq=0`).

For a **showcase demo refresh** you usually want **reinstall** + re-run
`/admin/bootstrap`. For a **real product upgrade** with users on the
platform, you want **in-place upgrade**, which preserves all state
including the audit chain (its primary value proposition).

> **Gotcha:** if you upgrade only some of the five canisters and not
> others, mismatched candid surfaces will produce inter-canister call
> failures. Either upgrade all five (the default) or co-ordinate carefully
> when partial upgrades are needed.

---

## Troubleshooting

In rough order of likelihood. Most of these come from the
`develop-canisters-for-engines` skill's playbook.

### `unknown subcommand 'package'`

You ran `icp package create`, not `demo-icp package create`. The public
`icp` doesn't have it. Use `demo-icp`.

### `asset directory '././dist' does not exist`

You skipped `pnpm --prefix web_frontend build`. The packager looks for
the directory `farm.manifest.json` declares as `web_frontend.asset_dir`
(in this repo: `./web_frontend/dist`). Run the build, then retry.

### Canister WASM missing from `.icp/cache/artifacts/`

You skipped `demo-icp build` or it errored. Run it; if it errors, look
for missing Rust toolchain components (`wasm32-unknown-unknown` target,
`ic-wasm`).

### After upload: home page is blank, DevTools shows `Invalid combined threshold signature`

The agent is verifying responses against the **mainnet** root key, but
this canister is on a non-mainnet engine. Confirm `actor.ts` is calling
`HttpAgent.create({ shouldFetchRootKey: true })` and explicitly
`await agent.fetchRootKey()`. (It is, in this repo — but if you forked
and tweaked, check.)

### After upload: agent calls go to `https://icp-api.io/...`

Someone hardcoded `host: 'https://icp-api.io'` (or kept `VITE_IC_HOST` in
the agent setup). Replace with `host: window.location.origin`. Skill
rule 2.

### After upload: II login fails with `Invalid delegation`

Wrong II canister ID baked in. Confirm `PUBLIC_II_CANISTER_ID` in
`farm.manifest.json` matches the engine's II — for the standard farm,
`s24we-diaaa-aaaaa-aaaka-cai`. Confirm `ii.ts` reads it from the
`ic_env` cookie at runtime.

### `/clients` is empty after sign-in

Either:

- You haven't run `/admin/bootstrap` yet — the data canister can't reach
  `identity` for authz, so authz fails closed and returns no clients.
- You're not Admin. Compliance and Admin see all clients; Advisors only
  see assigned clients. If you're a fresh sign-in but missed the
  auto-bootstrap window (e.g. you logged in before the wiring
  completed), go to `/admin/bootstrap` and run it.

### `/assistant` returns `NotConfigured("data")` or `NotConfigured("audit")`

You skipped `/admin/bootstrap` (specifically the
`ai_assistant.set_data_canister` / `set_audit_canister` steps). Run it.

### Audit log shows entries but `/audit` shows red

Hash chain doesn't match what the canister returned — could indicate
the canister was reinstalled mid-session (chain restarted but the
frontend has cached entries from the old chain). Hard-refresh the
browser. If still red, something genuinely tampered with the chain or
the frontend's hash recomputation has a bug — file an issue.

### `cargo install --git ... --locked` fails

Either drop `--locked`, or check the fork's `rust-toolchain.toml` for
the required version. If it's still failing, the fork may have moved —
the skill points at `nim-packaging-wip` as the canonical branch, but
that may be renamed in the future.

---

## Why this is its own doc

Three reasons:

1. **Different audience.** Local development is the everyday flow; you
   want it short and clear. Packaging is a once-per-release concern with
   a different set of tools (`demo-icp`), error modes, and verification
   steps. Mixing them in `README.md` makes both worse.
2. **Different cadence.** The dev flow stabilises early; the packaging
   flow churns until `icp` ships `package` in stable. Isolating it here
   means changes here don't pollute the dev docs.
3. **Linkability.** Internal stakeholders evaluating the bundle can be
   pointed at a single doc that explains how the artifact is produced
   and what's in it, without wading through Tailwind tips or hot-reload
   tricks.
