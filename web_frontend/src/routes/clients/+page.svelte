<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { formatChf, variantKey } from '$lib/format';
	import type { Client } from '../../declarations/data.types';

	let clients = $state<Client[]>([]);
	let loading = $state(true);
	let q = $state('');

	async function load() {
		const b = auth.state.backends;
		if (!b) return;
		loading = true;
		try {
			clients = await b.data.list_clients();
			clients.sort((a, b) => Number(b.aum_chf - a.aum_chf));
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		void auth.state.principal;
		if (auth.state.backends && auth.state.authenticated) load();
	});

	const filtered = $derived(
		clients.filter((c) => {
			if (!q.trim()) return true;
			const needle = q.trim().toLowerCase();
			return (
				c.display_name.toLowerCase().includes(needle) ||
				c.legal_name.toLowerCase().includes(needle) ||
				c.tax_residency.toLowerCase().includes(needle)
			);
		})
	);

	function kycColor(c: Client): string {
		if ('Approved' in c.kyc_status)
			return 'background: oklch(0.92 0.06 145); color: var(--color-good);';
		if ('Pending' in c.kyc_status)
			return 'background: oklch(0.95 0.06 65); color: var(--color-warn);';
		return 'background: oklch(0.94 0.05 25); color: var(--color-bad);';
	}
</script>

<section class="space-y-6">
	<header class="flex flex-wrap items-end justify-between gap-4">
		<div>
			<h1 class="font-serif text-3xl font-black tracking-tight">Clients</h1>
			<p class="ink-muted mt-1 text-sm">
				{clients.length} on your book. Click any row to open the client detail.
			</p>
		</div>
		<input
			type="search"
			bind:value={q}
			placeholder="Filter by name or residency"
			class="rule-line surface min-w-[20rem] rounded border px-3 py-2 text-sm"
		/>
	</header>

	{#if loading && clients.length === 0}
		<div class="ink-muted py-12 text-center">Loading clients…</div>
	{:else if filtered.length === 0}
		<div class="surface rounded p-8 text-center">
			<p class="ink-muted text-sm">No clients match.</p>
		</div>
	{:else}
		<div class="surface overflow-hidden rounded">
			<table class="w-full text-sm">
				<thead class="rule-line border-b text-left">
					<tr class="ink-muted text-[11px] tracking-[0.18em] uppercase">
						<th class="px-4 py-3 font-bold">Client</th>
						<th class="px-4 py-3 font-bold">Type</th>
						<th class="px-4 py-3 font-bold">Residency</th>
						<th class="px-4 py-3 font-bold">Risk profile</th>
						<th class="px-4 py-3 font-bold">KYC</th>
						<th class="px-4 py-3 text-right font-bold">AUM (CHF)</th>
					</tr>
				</thead>
				<tbody>
					{#each filtered as c (c.id)}
						<tr
							class="rule-line cursor-pointer border-b last:border-b-0 transition hover:bg-[var(--color-paper-deep)]"
							onclick={() => (window.location.href = `/clients/${c.id}`)}
						>
							<td class="px-4 py-3">
								<div class="font-bold">{c.display_name}</div>
								<div class="ink-muted text-xs">{c.legal_name}</div>
							</td>
							<td class="px-4 py-3 text-xs">
								{variantKey(c.client_type as unknown as Record<string, unknown>)}
							</td>
							<td class="px-4 py-3 font-mono text-xs">{c.tax_residency}</td>
							<td class="px-4 py-3 text-xs">
								{variantKey(c.risk_profile as unknown as Record<string, unknown>)}
							</td>
							<td class="px-4 py-3">
								<span
									class="rounded px-2 py-0.5 text-[10px] font-bold tracking-wider uppercase"
									style={kycColor(c)}
								>
									{variantKey(c.kyc_status as unknown as Record<string, unknown>)}
								</span>
							</td>
							<td class="numeral px-4 py-3 text-right font-mono">{formatChf(c.aum_chf)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</section>
