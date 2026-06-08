<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { AI_ENABLED } from '$lib/features';
	import { appErrorMessage } from '$lib/audit';
	import { getAuditId, getDataId, getIdentityId } from '$lib/ic-env';
	import { Principal } from '@dfinity/principal';

	type Probe = {
		label: string;
		status: 'ok' | 'fail' | 'unknown';
		detail?: string;
	};

	let probes = $state<Probe[]>([]);
	let probing = $state(false);
	let manualLog = $state<{ step: string; status: 'pending' | 'ok' | 'fail'; detail?: string }[]>(
		[]
	);
	let runningManual = $state(false);

	// — LLM endpoint setter —
	//
	// `icp deploy` injects `PUBLIC_CANISTER_ID:*` env vars but does NOT read
	// the marketplace manifests, so for local dev the LLM base URL has to
	// be set at runtime. We pre-fill the input with the same default the
	// manifests carry, and the manual-wiring sequence below includes a
	// `set_llm_base_url` call so a single click sets up everything.
	const DEFAULT_LLM_BASE_URL = 'https://[2602:fb2b:100:101:50bf:a8ff:fe92:6cdd]:11500';
	let llmUrl = $state(DEFAULT_LLM_BASE_URL);
	let savingLlm = $state(false);
	let llmSaveDetail = $state<string | null>(null);

	async function probe() {
		const b = auth.state.backends;
		if (!b) return;
		probing = true;
		probes = [];
		try {
			const auditIdentity = (await b.audit.identity_canister())[0];
			probes = [
				...probes,
				{
					label: 'audit → identity wired',
					status: auditIdentity ? 'ok' : 'fail',
					detail:
						auditIdentity?.toText() ??
						'env var not picked up; use Manual fallback below'
				}
			];

			const [dataIdentityOpt, dataAuditOpt] = await b.data.config();
			const dataIdentity = dataIdentityOpt[0];
			const dataAudit = dataAuditOpt[0];
			probes = [
				...probes,
				{
					label: 'data → identity wired',
					status: dataIdentity ? 'ok' : 'fail',
					detail: dataIdentity?.toText() ?? 'use Manual fallback'
				},
				{
					label: 'data → audit wired',
					status: dataAudit ? 'ok' : 'fail',
					detail: dataAudit?.toText() ?? 'use Manual fallback'
				}
			];

			// AI is disabled bundle-wide (see lib/features.ts); the ai_assistant
			// canister isn't shipped as a node, so skip its wiring probes.
			if (AI_ENABLED && b.ai) {
				const [aiDataOpt, aiAuditOpt] = await b.ai.config();
				const aiData = aiDataOpt[0];
				const aiAudit = aiAuditOpt[0];
				probes = [
					...probes,
					{
						label: 'ai_assistant → data wired',
						status: aiData ? 'ok' : 'fail',
						detail: aiData?.toText() ?? 'use Manual fallback'
					},
					{
						label: 'ai_assistant → audit wired',
						status: aiAudit ? 'ok' : 'fail',
						detail: aiAudit?.toText() ?? 'use Manual fallback'
					}
				];

				// LLM endpoint
				const llmOpt = await b.ai.llm_config();
				const llmCurrent = Array.isArray(llmOpt) && llmOpt.length > 0 ? llmOpt[0] : undefined;
				probes = [
					...probes,
					{
						label: 'ai_assistant → LLM endpoint set',
						status: llmCurrent ? 'ok' : 'fail',
						detail: llmCurrent ?? 'PUBLIC_LLM_BASE_URL env var not set; use the LLM section below'
					}
				];
				// If the canister already has a value, mirror it into the input
				// (overrides the DEFAULT_LLM_BASE_URL prefill) so the operator
				// sees what's actually live rather than a stale placeholder.
				if (llmCurrent) {
					llmUrl = llmCurrent;
				}
			}

			// Identity bootstrap
			const adminBootstrapped = await b.identity.admin_bootstrapped();
			probes = [
				...probes,
				{
					label: 'identity admin bootstrapped',
					status: adminBootstrapped ? 'ok' : 'fail',
					detail: adminBootstrapped ? 'first user has Admin' : 'sign in to bootstrap'
				}
			];
		} finally {
			probing = false;
		}
	}

	async function runManualFallback() {
		const b = auth.state.backends;
		if (!b) return;
		const identity = Principal.fromText(getIdentityId());
		const audit = Principal.fromText(getAuditId());
		const data = Principal.fromText(getDataId());

		const steps: { label: string; fn: () => Promise<unknown> }[] = [
			{ label: 'identity.bootstrap_admin', fn: () => b.identity.bootstrap_admin() },
			{
				label: 'audit.set_identity_canister(identity)',
				fn: () => b.audit.set_identity_canister(identity)
			},
			{
				label: 'data.set_identity_canister(identity)',
				fn: () => b.data.set_identity_canister(identity)
			},
			{ label: 'data.set_audit_canister(audit)', fn: () => b.data.set_audit_canister(audit) }
		];

		// AI disabled bundle-wide (see lib/features.ts): skip ai_assistant wiring.
		const ai = b.ai;
		if (AI_ENABLED && ai) {
			steps.push(
				{ label: 'ai_assistant.set_data_canister(data)', fn: () => ai.set_data_canister(data) },
				{
					label: 'ai_assistant.set_audit_canister(audit)',
					fn: () => ai.set_audit_canister(audit)
				},
				{
					label: `ai_assistant.set_llm_base_url(${llmUrl.trim() || DEFAULT_LLM_BASE_URL})`,
					fn: () => ai.set_llm_base_url(llmUrl.trim() || DEFAULT_LLM_BASE_URL)
				}
			);
		}

		manualLog = [];
		runningManual = true;
		try {
			for (const s of steps) {
				manualLog = [...manualLog, { step: s.label, status: 'pending' }];
				const idx = manualLog.length - 1;
				try {
					const r = await s.fn();
					if (r && typeof r === 'object' && 'Err' in (r as Record<string, unknown>)) {
						const errEntry = (r as { Err: unknown }).Err;
						manualLog = manualLog.map((x, i) =>
							i === idx
								? { ...x, status: 'fail' as const, detail: appErrorMessage(errEntry) }
								: x
						);
					} else {
						manualLog = manualLog.map((x, i) =>
							i === idx ? { ...x, status: 'ok' as const } : x
						);
					}
				} catch (err) {
					manualLog = manualLog.map((x, i) =>
						i === idx
							? { ...x, status: 'fail' as const, detail: appErrorMessage(err) }
							: x
					);
				}
			}
		} finally {
			runningManual = false;
		}
		await probe();
	}

	async function saveLlmUrl() {
		const ai = auth.state.backends?.ai;
		if (!ai) return;
		llmSaveDetail = null;
		savingLlm = true;
		try {
			const r = await ai.set_llm_base_url(llmUrl.trim());
			if ('Err' in r) {
				llmSaveDetail = appErrorMessage(r.Err);
			} else {
				llmSaveDetail = 'Saved.';
				await probe();
			}
		} catch (err) {
			llmSaveDetail = appErrorMessage(err);
		} finally {
			savingLlm = false;
		}
	}

	$effect(() => {
		void auth.state.principal;
		if (auth.state.backends && auth.state.authenticated) probe();
	});

	const allOk = $derived(probes.length > 0 && probes.every((p) => p.status === 'ok'));
</script>

<section class="mx-auto max-w-3xl space-y-6">
	<header class="space-y-2">
		<h1 class="font-serif text-3xl font-black tracking-tight">Inter-canister wiring</h1>
		<p class="ink-muted max-w-2xl text-sm">
			On Cloud Engines installs (and on local <code>icp deploy</code>), the canisters
			auto-discover each other from <code>PUBLIC_CANISTER_ID:&lt;dep&gt;</code> env vars
			that the installer injects. This page reports the wiring status and offers a
			one-click manual fallback for the rare case env vars aren't picked up
			(e.g. after <code>--mode reinstall</code> on a non-installer-driven flow).
		</p>
	</header>

	{#if !auth.state.authenticated}
		<div class="surface rounded p-8 text-center">
			<p class="ink-muted text-sm">Sign in first.</p>
		</div>
	{:else}
		<div class="surface space-y-3 rounded p-6">
			<div class="flex items-center justify-between">
				<h2 class="font-serif text-lg font-black">Wiring status</h2>
				<button
					type="button"
					onclick={probe}
					class="surface ink-muted hover:ink rounded px-3 py-1.5 text-xs"
					disabled={probing}
				>
					{probing ? 'Probing…' : 'Re-probe'}
				</button>
			</div>

			{#if probes.length === 0}
				<div class="ink-muted text-sm">Probing…</div>
			{:else}
				<ol class="space-y-1 text-sm">
					{#each probes as p}
						<li class="rule-line flex items-baseline gap-3 border-b py-2 last:border-b-0">
							{#if p.status === 'ok'}
								<span style="color: var(--color-good);">✓</span>
							{:else if p.status === 'fail'}
								<span style="color: var(--color-bad);">✗</span>
							{:else}
								<span class="ink-muted">?</span>
							{/if}
							<span class="flex-1">{p.label}</span>
							{#if p.detail}
								<span class="ink-muted font-mono text-[11px]">{p.detail}</span>
							{/if}
						</li>
					{/each}
				</ol>

				{#if allOk}
					<div
						class="rounded px-3 py-2 text-sm"
						style="background: oklch(0.92 0.06 145); color: var(--color-good);"
					>
						All wiring auto-discovered from env vars. No action required — the demo is ready.
						Go to <a href="/clients" class="underline">/clients</a>.
					</div>
				{/if}
			{/if}
		</div>

		{#if AI_ENABLED}
			<div class="surface space-y-3 rounded p-6">
				<h2 class="font-serif text-lg font-black">LLM endpoint</h2>
				<p class="ink-muted text-sm">
					Base URL of the on-engine GPU node serving <code>POST /v1/agent/run</code>.
					The canister appends the path itself. Picked up automatically from the
					<code>PUBLIC_LLM_BASE_URL</code> env var at install time; rotate at runtime here
					(controllers only).
				</p>
				<input
					type="text"
					bind:value={llmUrl}
					placeholder="https://[2602:fb2b:100:101:50bf:a8ff:fe92:6cdd]:11500"
					class="rule-line surface w-full rounded border px-3 py-2 font-mono text-xs"
				/>
				<div class="flex items-center gap-3">
					<button
						type="button"
						onclick={saveLlmUrl}
						disabled={savingLlm || !llmUrl.trim()}
						class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)] disabled:opacity-50"
						style="background: var(--color-burgundy);"
					>
						{savingLlm ? 'Saving…' : 'Save LLM endpoint'}
					</button>
					{#if llmSaveDetail}
						<span class="ink-muted text-xs">{llmSaveDetail}</span>
					{/if}
				</div>
			</div>
		{/if}

		{#if !allOk}
			<div class="surface space-y-3 rounded p-6">
				<h2 class="font-serif text-lg font-black">Manual fallback</h2>
				<p class="ink-muted text-sm">
					Issues the inter-canister wiring setter calls in sequence{AI_ENABLED
						? ' (plus the ai_assistant LLM endpoint)'
						: ''}. Use when the wiring status above shows red — typical after a
					local <code>icp deploy</code>, which doesn't read the marketplace manifest.
				</p>
				<button
					type="button"
					onclick={runManualFallback}
					disabled={runningManual}
					class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)] disabled:opacity-50"
					style="background: var(--color-burgundy);"
				>
					{runningManual ? 'Running…' : 'Run manual wiring'}
				</button>
				{#if manualLog.length > 0}
					<ol class="space-y-1 text-sm">
						{#each manualLog as l}
							<li class="rule-line flex items-baseline gap-3 border-b py-2 last:border-b-0">
								{#if l.status === 'pending'}
									<span class="ink-muted">…</span>
								{:else if l.status === 'ok'}
									<span style="color: var(--color-good);">✓</span>
								{:else}
									<span style="color: var(--color-bad);">✗</span>
								{/if}
								<span class="flex-1 font-mono text-xs">{l.step}</span>
								{#if l.detail}
									<span class="ink-muted text-[11px]">{l.detail}</span>
								{/if}
							</li>
						{/each}
					</ol>
				{/if}
			</div>
		{/if}
	{/if}
</section>
