<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { describeAction, verifyChain, type ChainVerification } from '$lib/audit';
	import { formatDateTime, shortPrincipal } from '$lib/format';
	import type { AuditEntry } from '../../declarations/audit.types';

	let entries = $state<AuditEntry[]>([]);
	let loading = $state(true);
	let total = $state<bigint>(0n);
	let nextCursor = $state<bigint | null>(null);
	let verification = $state<ChainVerification | null>(null);
	let verifying = $state(false);

	async function loadInitial() {
		const audit = auth.state.backends?.audit;
		if (!audit) return;
		loading = true;
		try {
			const page = await audit.audit_log_page([], 50n);
			entries = page.entries;
			total = page.total;
			nextCursor = page.next_cursor[0] ?? null;
			await verify();
		} finally {
			loading = false;
		}
	}

	async function loadMore() {
		const audit = auth.state.backends?.audit;
		if (!audit || nextCursor === null) return;
		const page = await audit.audit_log_page([nextCursor], 50n);
		entries = [...entries, ...page.entries];
		total = page.total;
		nextCursor = page.next_cursor[0] ?? null;
		await verify();
	}

	async function verify() {
		verifying = true;
		try {
			verification = await verifyChain(entries);
		} finally {
			verifying = false;
		}
	}

	$effect(() => {
		void auth.state.principal;
		void auth.state.ready;
		if (auth.state.backends && auth.state.authenticated && entries.length === 0) loadInitial();
	});
</script>

<section class="space-y-6">
	<header class="space-y-2">
		<div
			class="ink-muted inline-flex items-center gap-2 text-[11px] tracking-[0.18em] uppercase"
		>
			SHA-256 hash chain · re-verified in your browser
		</div>
		<h1 class="font-serif text-3xl font-black tracking-tight">Audit log</h1>
		<p class="ink-muted max-w-3xl text-sm">
			Every read, write, AI prompt and AI response is appended to a hash-chained log. Each
			entry's hash includes the previous entry's hash; tampering with any past entry invalidates
			every subsequent hash. This page fetches the log and re-derives every hash client-side.
		</p>
	</header>

	<div class="surface flex flex-wrap items-center justify-between gap-3 rounded p-4">
		<div class="flex items-center gap-3">
			{#if verifying}
				<div
					class="size-2.5 animate-pulse rounded-full"
					style="background: var(--color-warn);"
				></div>
				<div class="text-sm font-bold">Verifying chain…</div>
			{:else if verification?.ok}
				<div
					class="size-2.5 rounded-full"
					style="background: var(--color-good); box-shadow: 0 0 8px var(--color-good);"
				></div>
				<div class="text-sm font-bold" style="color: var(--color-good);">
					Chain intact — {entries.length} of {total.toString()} entries verified
				</div>
			{:else if verification && !verification.ok}
				<div class="size-2.5 rounded-full" style="background: var(--color-bad);"></div>
				<div class="text-sm font-bold" style="color: var(--color-bad);">
					Chain INVALID at seq {verification.firstBadSeq?.toString()} ({verification.reason})
				</div>
			{:else}
				<div class="ink-muted text-sm">—</div>
			{/if}
		</div>
		<button
			type="button"
			onclick={verify}
			class="surface ink-muted hover:ink rounded px-3 py-1.5 text-xs"
		>
			Re-verify
		</button>
	</div>

	{#if loading && entries.length === 0}
		<div class="ink-muted py-12 text-center">Loading log…</div>
	{:else if entries.length === 0}
		<div class="surface rounded p-8 text-center text-sm">No entries visible to you yet.</div>
	{:else}
		<ol class="rule-line relative space-y-2 border-l pl-5">
			{#each entries as e (e.seq)}
				<li class="relative">
					<span
						class="absolute top-3 size-3 rounded-full"
						style="left: -27px; background: var(--color-paper); border: 2px solid var(--color-burgundy);"
					></span>
					<article class="surface space-y-2 rounded p-4">
						<header class="flex flex-wrap items-baseline justify-between gap-2">
							<div class="flex items-center gap-2">
								<span class="font-mono text-xs" style="color: var(--color-burgundy);">
									#{e.seq.toString()}
								</span>
								<span class="text-sm font-bold">{describeAction(e.action)}</span>
							</div>
							<span class="ink-muted font-mono text-[11px]">{formatDateTime(e.ts_ns)}</span>
						</header>
						<div class="ink-muted flex flex-wrap gap-x-4 gap-y-0.5 text-[11px]">
							<span>by <span class="font-mono">{shortPrincipal(e.caller)}</span></span>
						</div>
						<details class="text-[11px]">
							<summary class="ink-muted hover:ink cursor-pointer">Hashes</summary>
							<div class="mt-1 space-y-0.5 font-mono break-all">
								<div><span class="ink-muted">prev </span>{e.prev_hash || '—'}</div>
								<div><span class="ink-muted">this </span>{e.hash}</div>
							</div>
						</details>
					</article>
				</li>
			{/each}
		</ol>

		{#if nextCursor !== null}
			<div class="flex justify-center">
				<button
					type="button"
					onclick={loadMore}
					class="surface ink-muted hover:ink rounded px-5 py-2 text-sm"
				>
					Load more
				</button>
			</div>
		{/if}
	{/if}
</section>
