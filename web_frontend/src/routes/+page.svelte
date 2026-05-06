<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { formatChf, formatDateTime } from '$lib/format';
	import type { Client } from '../declarations/data.types';
	import type { AuditHead } from '../declarations/audit.types';

	let clients = $state<Client[]>([]);
	let head = $state<AuditHead | null>(null);
	let loading = $state(true);

	async function load() {
		const b = auth.state.backends;
		if (!b) return;
		loading = true;
		try {
			[clients, head] = await Promise.all([b.data.list_clients(), b.audit.audit_head()]);
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		void auth.state.principal;
		void auth.state.ready;
		if (auth.state.backends && auth.state.authenticated) load();
	});

	const totalAum = $derived(clients.reduce((s, c) => s + c.aum_chf, 0n));
	const expiredKyc = $derived(clients.filter((c) => 'Expired' in c.kyc_status).length);
	const pendingKyc = $derived(clients.filter((c) => 'Pending' in c.kyc_status).length);
</script>

<section class="space-y-10">
	<div class="space-y-3">
		<div
			class="ink-muted inline-flex items-center gap-2 text-[11px] tracking-[0.18em] uppercase"
		>
			<span class="size-1.5 rounded-full" style="background: var(--color-burgundy);"></span>
			Swiss-locked engine · 5-canister architecture · audit-chained · stub LLM
		</div>
		<h1 class="font-serif text-4xl leading-tight font-black tracking-tight sm:text-5xl">
			A workspace your <span class="burgundy">compliance officer</span> can sign off without
			reading the contract.
		</h1>
		<p class="ink-muted max-w-3xl text-base sm:text-lg">
			Every client record, meeting note, trade idea, and AI interaction is appended to an on-chain
			hash-chained audit log running on a jurisdiction-locked Cloud Engine. The data — and the AI
			that reads it — never leave the bank's compute envelope.
		</p>
	</div>

	{#if !auth.state.authenticated}
		<div class="surface space-y-3 rounded p-8 text-center">
			<h2 class="font-serif text-2xl font-black">Sign in to continue</h2>
			<p class="ink-muted">
				Internet Identity is required. Your principal becomes your role-bound advisor identity.
			</p>
			<button
				type="button"
				onclick={() => auth.login()}
				class="mt-2 rounded px-5 py-2 text-sm font-bold text-[var(--color-paper)]"
				style="background: var(--color-burgundy);"
			>
				Sign in with Internet Identity
			</button>
		</div>
	{:else if loading && clients.length === 0}
		<div class="ink-muted py-12 text-center">Loading workspace…</div>
	{:else}
		<div class="grid gap-4 md:grid-cols-3">
			<div class="surface rounded p-6">
				<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">Clients on book</div>
				<div class="numeral mt-1 font-serif text-3xl font-black">{clients.length}</div>
			</div>
			<div class="surface rounded p-6">
				<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">Total AUM</div>
				<div class="numeral mt-1 font-serif text-3xl font-black">
					CHF {formatChf(totalAum)}
				</div>
			</div>
			<div class="surface rounded p-6">
				<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">KYC requires action</div>
				<div class="numeral mt-1 font-serif text-3xl font-black">
					{expiredKyc + pendingKyc}
					{#if expiredKyc > 0 || pendingKyc > 0}
						<span class="ink-muted text-sm font-normal">
							({expiredKyc} expired, {pendingKyc} pending)
						</span>
					{/if}
				</div>
			</div>
		</div>

		<div class="grid gap-6 md:grid-cols-[2fr_1fr]">
			<a
				href="/clients"
				class="surface hover:surface-deep group flex flex-col justify-between gap-2 rounded p-6 transition"
			>
				<div>
					<h2 class="font-serif text-xl font-black">Open the client book →</h2>
					<p class="ink-muted mt-1 text-sm">
						Filter and inspect every client you have visibility on. Opening a client detail
						records a `ClientAccessed` audit entry.
					</p>
				</div>
				<div class="ink-muted text-xs">
					{clients.length} clients visible to you
				</div>
			</a>
			<a
				href="/assistant"
				class="surface hover:surface-deep group flex flex-col gap-2 rounded p-6 transition"
				style="border-left: 3px solid var(--color-burgundy);"
			>
				<h2 class="font-serif text-xl font-black">Ask the assistant →</h2>
				<p class="ink-muted text-sm">
					Stub LLM. Real intent routing over your visible book. Every question + answer is
					hash-chained.
				</p>
			</a>
		</div>

		<div class="surface space-y-2 rounded p-6">
			<div class="flex items-baseline justify-between">
				<h2 class="font-serif text-xl font-black">Audit chain</h2>
				<a class="ink-muted hover:ink text-xs underline" href="/audit">View →</a>
			</div>
			<div class="ink-muted text-sm">
				Head at <span class="numeral ink font-mono">seq={head?.seq.toString() ?? '—'}</span> ·
				latest hash
				<code class="numeral text-xs">
					{head && head.hash ? `${head.hash.slice(0, 16)}…` : '—'}
				</code>
			</div>
			<div class="ink-muted text-xs">
				Last refreshed {head ? formatDateTime(BigInt(Date.now()) * 1_000_000n) : '—'}
			</div>
		</div>
	{/if}
</section>
