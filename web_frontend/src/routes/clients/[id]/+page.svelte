<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { auth } from '$lib/auth.svelte';
	import { appErrorMessage } from '$lib/audit';
	import {
		formatChf,
		formatChfFromCents,
		formatDate,
		formatDateTime,
		variantKey
	} from '$lib/format';
	import type {
		Client,
		Meeting,
		Portfolio,
		TradeIdea
	} from '../../../declarations/data.types';

	let client = $state<Client | null>(null);
	let portfolios = $state<Portfolio[]>([]);
	let meetings = $state<Meeting[]>([]);
	let tradeIdeas = $state<TradeIdea[]>([]);
	let loading = $state(true);
	let errMsg = $state<string | null>(null);

	const clientId = $derived(BigInt(page.params.id ?? '0'));

	async function load() {
		const b = auth.state.backends;
		if (!b) return;
		loading = true;
		errMsg = null;
		try {
			const c = await b.data.get_client(clientId);
			if ('Err' in c) {
				errMsg = appErrorMessage(c.Err);
				return;
			}
			client = c.Ok;
			const ps: Portfolio[] = [];
			for (const pid of client.portfolio_ids) {
				const p = await b.data.get_portfolio(pid);
				if ('Ok' in p) ps.push(p.Ok);
			}
			portfolios = ps;
			const [ms, ts] = await Promise.all([
				b.data.list_meetings(clientId),
				b.data.list_trade_ideas(clientId)
			]);
			if ('Ok' in ms) meetings = ms.Ok;
			if ('Ok' in ts) tradeIdeas = ts.Ok;
			b.data.record_client_access(clientId, { ManualReview: null }).catch(() => {});
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		void clientId;
		void auth.state.principal;
		if (auth.state.backends && auth.state.authenticated) load();
	});

	function portfolioMtmChf(p: Portfolio): bigint {
		const positions = p.positions.reduce(
			(s, pos) => s + pos.quantity * pos.current_price_chf_cents,
			0n
		);
		return (positions + BigInt(p.cash_chf_cents)) / 100n;
	}

	function statusColor(s: TradeIdea['status']): string {
		if ('Approved' in s) return 'color: var(--color-good); border-color: var(--color-good);';
		if ('Executed' in s) return 'color: var(--color-burgundy); border-color: var(--color-burgundy);';
		if ('Rejected' in s) return 'color: var(--color-bad); border-color: var(--color-bad);';
		return 'color: var(--color-ink-muted); border-color: var(--color-rule-strong);';
	}
</script>

<a class="ink-muted text-xs hover:underline" href="/clients">← back to clients</a>

{#if loading && !client}
	<div class="ink-muted py-12 text-center">Loading client…</div>
{:else if errMsg}
	<div class="surface mt-4 rounded p-8 text-center">
		<p class="text-sm" style="color: var(--color-bad);">{errMsg}</p>
	</div>
{:else if client}
	<header class="mt-4 space-y-3">
		<div
			class="ink-muted inline-flex items-center gap-2 text-[11px] tracking-[0.18em] uppercase"
		>
			<span>Client #{client.id.toString()}</span>
			<span>·</span>
			<span>{variantKey(client.client_type as unknown as Record<string, unknown>)}</span>
			<span>·</span>
			<span class="font-mono">{client.tax_residency}</span>
		</div>
		<h1 class="font-serif text-3xl font-black tracking-tight sm:text-4xl">
			{client.display_name}
		</h1>
		<p class="ink-muted text-sm">{client.legal_name}</p>
	</header>

	<section class="mt-6 grid gap-4 md:grid-cols-3">
		<div class="surface rounded p-5">
			<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">Risk profile</div>
			<div class="mt-1 font-serif text-xl font-black">
				{variantKey(client.risk_profile as unknown as Record<string, unknown>)}
			</div>
		</div>
		<div class="surface rounded p-5">
			<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">KYC</div>
			<div class="mt-1 font-serif text-xl font-black">
				{variantKey(client.kyc_status as unknown as Record<string, unknown>)}
			</div>
			<div class="ink-muted mt-1 text-xs">
				{'Expired' in client.kyc_status ? 'Expired' : 'Expires'}: {formatDate(
					client.kyc_expires_ns
				)}
			</div>
		</div>
		<div class="surface rounded p-5">
			<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">AUM</div>
			<div class="numeral mt-1 font-serif text-xl font-black">
				CHF {formatChf(client.aum_chf)}
			</div>
		</div>
	</section>

	<section class="mt-8 space-y-3">
		<h2 class="font-serif text-xl font-black">Portfolios</h2>
		<div class="grid gap-3 lg:grid-cols-2">
			{#each portfolios as p (p.id)}
				<div class="surface rounded p-5">
					<div class="flex items-baseline justify-between">
						<h3 class="font-serif text-lg font-black">{p.name}</h3>
						<span class="ink-muted font-mono text-xs">{p.base_currency}</span>
					</div>
					<div class="numeral mt-2 font-mono text-2xl font-black">
						CHF {formatChf(portfolioMtmChf(p))}
					</div>
					<div class="ink-muted mt-1 text-xs">
						Cash CHF {formatChfFromCents(p.cash_chf_cents)} · {p.positions.length} positions
					</div>
					<table class="mt-3 w-full text-xs">
						<thead class="ink-muted">
							<tr class="text-left">
								<th class="py-1 font-bold">Ticker</th>
								<th class="py-1 font-bold">Class</th>
								<th class="py-1 text-right font-bold">Qty</th>
								<th class="py-1 text-right font-bold">Mark CHF</th>
							</tr>
						</thead>
						<tbody>
							{#each p.positions as pos}
								{@const value = (pos.quantity * pos.current_price_chf_cents) / 100n}
								<tr class="rule-line border-t">
									<td class="py-1 font-mono">{pos.ticker}</td>
									<td class="py-1">
										{variantKey(pos.asset_class as unknown as Record<string, unknown>)}
									</td>
									<td class="numeral py-1 text-right font-mono">
										{formatChf(pos.quantity)}
									</td>
									<td class="numeral py-1 text-right font-mono">
										{formatChf(value)}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/each}
		</div>
	</section>

	<section class="mt-8 grid gap-6 md:grid-cols-2">
		<div>
			<h2 class="font-serif text-xl font-black">Meetings</h2>
			<div class="mt-3 space-y-3">
				{#each meetings as m (m.id)}
					<div class="surface rounded p-4">
						<div class="flex items-baseline justify-between">
							<h3 class="font-bold">{m.title}</h3>
							<span class="ink-muted font-mono text-xs">{formatDateTime(m.occurred_at_ns)}</span>
						</div>
						<p class="ink-muted mt-2 text-sm whitespace-pre-line">{m.notes_md}</p>
						{#if m.decisions.length > 0}
							<div class="mt-2 text-xs">
								<span class="ink-muted">Decisions:</span>
								<ul class="ml-4 list-disc">
									{#each m.decisions as d}
										<li>{d}</li>
									{/each}
								</ul>
							</div>
						{/if}
						{#if m.follow_ups.length > 0}
							<div class="mt-2 text-xs">
								<span class="ink-muted">Follow-ups:</span>
								<ul class="ml-4 list-disc">
									{#each m.follow_ups as f}
										<li>{f}</li>
									{/each}
								</ul>
							</div>
						{/if}
					</div>
				{:else}
					<div class="ink-muted text-sm">No meetings on file.</div>
				{/each}
			</div>
		</div>
		<div>
			<h2 class="font-serif text-xl font-black">Trade ideas</h2>
			<div class="mt-3 space-y-3">
				{#each tradeIdeas as t (t.id)}
					<div class="surface rounded p-4">
						<div class="flex items-baseline justify-between">
							<h3 class="font-bold">{t.title}</h3>
							<span
								class="rounded border px-2 py-0.5 text-[10px] font-bold tracking-wider uppercase"
								style={statusColor(t.status)}
							>
								{variantKey(t.status as unknown as Record<string, unknown>)}
							</span>
						</div>
						<p class="ink-muted mt-2 text-sm whitespace-pre-line">{t.rationale}</p>
					</div>
				{:else}
					<div class="ink-muted text-sm">No trade ideas yet.</div>
				{/each}
			</div>
		</div>
	</section>

	<div class="mt-8 flex flex-wrap justify-end gap-3">
		<a
			href={`/clients/${client.id}/documents`}
			class="surface ink-muted hover:ink rounded px-4 py-2 text-sm font-bold"
		>
			Documents (encrypted) →
		</a>
		<a
			href={`/assistant?client=${client.id}`}
			class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)]"
			style="background: var(--color-burgundy);"
		>
			Ask the assistant about this client →
		</a>
	</div>
{/if}
