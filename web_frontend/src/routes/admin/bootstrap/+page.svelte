<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { appErrorMessage } from '$lib/audit';
	import {
		getAiAssistantId,
		getAuditId,
		getDataId,
		getIdentityId
	} from '$lib/ic-env';
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
			{ label: 'data.set_audit_canister(audit)', fn: () => b.data.set_audit_canister(audit) },
			{ label: 'ai_assistant.set_data_canister(data)', fn: () => b.ai.set_data_canister(data) },
			{ label: 'ai_assistant.set_audit_canister(audit)', fn: () => b.ai.set_audit_canister(audit) }
		];

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

		{#if !allOk}
			<div class="surface space-y-3 rounded p-6">
				<h2 class="font-serif text-lg font-black">Manual fallback</h2>
				<p class="ink-muted text-sm">
					Issues 6 setter calls in sequence. Use only if the wiring status above shows red.
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
