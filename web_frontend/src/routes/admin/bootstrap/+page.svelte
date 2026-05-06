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

	type Step = {
		label: string;
		fn: () => Promise<unknown>;
	};

	let log = $state<{ step: string; status: 'pending' | 'ok' | 'fail'; detail?: string }[]>([]);
	let running = $state(false);

	function steps(): Step[] {
		const b = auth.state.backends;
		if (!b) return [];
		const identityId = getIdentityId();
		const auditId = getAuditId();
		const dataId = getDataId();
		const aiId = getAiAssistantId();
		const identity = Principal.fromText(identityId);
		const audit = Principal.fromText(auditId);
		const data = Principal.fromText(dataId);
		const ai = Principal.fromText(aiId);
		return [
			{
				label: 'identity.bootstrap_admin (you become Admin)',
				fn: () => b.identity.bootstrap_admin()
			},
			{
				label: 'audit.set_identity_canister(identity)',
				fn: () => b.audit.set_identity_canister(identity)
			},
			{
				label: 'audit.admit_writer(data)',
				fn: () => b.audit.admit_writer(data)
			},
			{
				label: 'audit.admit_writer(ai_assistant)',
				fn: () => b.audit.admit_writer(ai)
			},
			{
				label: 'audit.admit_writer(identity) — for role/assignment events',
				fn: () => b.audit.admit_writer(identity)
			},
			{
				label: 'data.set_identity_canister(identity)',
				fn: () => b.data.set_identity_canister(identity)
			},
			{
				label: 'data.set_audit_canister(audit)',
				fn: () => b.data.set_audit_canister(audit)
			},
			{
				label: 'ai_assistant.set_data_canister(data)',
				fn: () => b.ai.set_data_canister(data)
			},
			{
				label: 'ai_assistant.set_audit_canister(audit)',
				fn: () => b.ai.set_audit_canister(audit)
			},
			{
				label: `identity.admit_ai_assistant(${aiId})`,
				fn: () => b.identity.admit_ai_assistant(ai)
			}
		];
	}

	async function run() {
		log = [];
		running = true;
		try {
			for (const s of steps()) {
				log = [...log, { step: s.label, status: 'pending' }];
				const idx = log.length - 1;
				try {
					const r = await s.fn();
					if (r && typeof r === 'object' && 'Err' in (r as Record<string, unknown>)) {
						const errEntry = (r as { Err: unknown }).Err;
						log = log.map((x, i) =>
							i === idx
								? { ...x, status: 'fail' as const, detail: appErrorMessage(errEntry) }
								: x
						);
					} else {
						log = log.map((x, i) => (i === idx ? { ...x, status: 'ok' as const } : x));
					}
				} catch (err) {
					log = log.map((x, i) =>
						i === idx
							? { ...x, status: 'fail' as const, detail: appErrorMessage(err) }
							: x
					);
				}
			}
		} finally {
			running = false;
		}
	}
</script>

<section class="mx-auto max-w-3xl space-y-6">
	<header class="space-y-2">
		<h1 class="font-serif text-3xl font-black tracking-tight">Inter-canister bootstrap</h1>
		<p class="ink-muted max-w-2xl text-sm">
			Run once after a fresh <code>icp deploy</code>. Wires the five canisters together: tells
			audit who its writers are, points data + ai_assistant at audit + identity, and admits the
			ai_assistant principal in identity. On a Cloud Engines install most of this is automatic
			via injected env vars; this page is for local dev and for the rare case you need to
			re-wire.
		</p>
	</header>

	{#if !auth.state.authenticated}
		<div class="surface rounded p-8 text-center">
			<p class="ink-muted text-sm">Sign in first. Bootstrap requires a controller identity.</p>
		</div>
	{:else}
		<div class="surface space-y-3 rounded p-6">
			<button
				type="button"
				onclick={run}
				disabled={running}
				class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)] disabled:opacity-50"
				style="background: var(--color-burgundy);"
			>
				{running ? 'Running…' : 'Run bootstrap'}
			</button>
			{#if log.length > 0}
				<ol class="space-y-2 text-sm">
					{#each log as l}
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
</section>
