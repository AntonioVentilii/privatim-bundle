<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { appErrorMessage } from '$lib/audit';
	import { formatDateTime } from '$lib/format';
	import type {
		AuditHead,
		ComplianceExport
	} from '../../../declarations/audit.types';

	let head = $state<AuditHead | null>(null);
	let fromSeq = $state('0');
	let toSeq = $state('');
	let loading = $state(false);
	let errMsg = $state<string | null>(null);
	let lastExport = $state<ComplianceExport | null>(null);

	$effect(() => {
		void auth.state.principal;
		if (!auth.state.backends || !auth.state.authenticated) return;
		auth.state.backends.audit.audit_head().then((h) => {
			head = h;
			if (!toSeq) toSeq = h.seq.toString();
		});
	});

	async function doExport() {
		const audit = auth.state.backends?.audit;
		if (!audit) return;
		errMsg = null;
		loading = true;
		try {
			const res = await audit.signed_audit_export(BigInt(fromSeq || '0'), BigInt(toSeq || '0'));
			if ('Err' in res) {
				errMsg = appErrorMessage(res.Err);
				return;
			}
			lastExport = res.Ok;
			const blob = new Blob([JSON.stringify(serialise(res.Ok), null, 2)], {
				type: 'application/json'
			});
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `privatim-audit-${fromSeq}-to-${toSeq}.json`;
			a.click();
			URL.revokeObjectURL(url);
		} catch (err) {
			errMsg = appErrorMessage(err);
		} finally {
			loading = false;
		}
	}

	function serialise(v: unknown): unknown {
		if (typeof v === 'bigint') return v.toString();
		if (Array.isArray(v)) return v.map(serialise);
		if (v && typeof v === 'object') {
			const o: Record<string, unknown> = {};
			for (const [k, val] of Object.entries(v)) {
				if (k === 'caller' || k === 'exporter' || k === 'to' || k === 'grantee') {
					o[k] =
						val && typeof val === 'object' && 'toText' in val
							? (val as { toText(): string }).toText()
							: serialise(val);
				} else {
					o[k] = serialise(val);
				}
			}
			return o;
		}
		return v;
	}
</script>

<section class="mx-auto max-w-3xl space-y-6">
	<header class="space-y-2">
		<div
			class="ink-muted inline-flex items-center gap-2 text-[11px] tracking-[0.18em] uppercase"
		>
			Compliance / regulator handoff
		</div>
		<h1 class="font-serif text-3xl font-black tracking-tight">Audit export</h1>
		<p class="ink-muted text-sm">
			Exports a slice of the audit chain as a JSON file. The export records itself in the chain
			(`ComplianceExport`) so it's part of the same lineage. The file's `head_hash` is the
			canister chain head at the time of export — the regulator can verify the engine threshold
			signature against it independently.
		</p>
	</header>

	{#if !auth.hasRole('Compliance') && !auth.hasRole('Admin')}
		<div class="surface rounded p-8 text-center">
			<p class="ink-muted text-sm">
				Compliance or Admin role required. The first authenticated principal is auto-granted Admin.
			</p>
		</div>
	{:else}
		<div class="surface space-y-4 rounded p-6">
			<div class="ink-muted text-sm">
				Chain head:
				<span class="numeral ink font-mono">seq={head?.seq.toString() ?? '—'}</span> ·
				<code class="numeral text-xs">
					{head && head.hash ? `${head.hash.slice(0, 16)}…` : '—'}
				</code>
			</div>

			<div class="grid grid-cols-2 gap-3">
				<label class="block">
					<span class="ink-muted text-xs tracking-[0.18em] uppercase">From seq</span>
					<input
						type="number"
						min="0"
						bind:value={fromSeq}
						class="rule-line surface mt-1 w-full rounded border px-3 py-2 text-sm"
					/>
				</label>
				<label class="block">
					<span class="ink-muted text-xs tracking-[0.18em] uppercase">To seq</span>
					<input
						type="number"
						min="0"
						bind:value={toSeq}
						class="rule-line surface mt-1 w-full rounded border px-3 py-2 text-sm"
					/>
				</label>
			</div>

			<button
				type="button"
				onclick={doExport}
				disabled={loading}
				class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)] disabled:opacity-50"
				style="background: var(--color-burgundy);"
			>
				{loading ? 'Exporting…' : 'Export & download JSON'}
			</button>

			{#if errMsg}
				<div class="text-sm" style="color: var(--color-bad);">{errMsg}</div>
			{/if}
		</div>

		{#if lastExport}
			<div class="surface space-y-2 rounded p-6 text-sm">
				<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">Last export</div>
				<div>Range: <span class="font-mono">{lastExport.from_seq.toString()}–{lastExport.to_seq.toString()}</span></div>
				<div>Entries: <span class="font-mono">{lastExport.entries.length}</span></div>
				<div>
					Head hash: <code class="numeral text-xs">{lastExport.head_hash.slice(0, 24)}…</code>
				</div>
				<div>Exported at: {formatDateTime(lastExport.exported_at_ns)}</div>
			</div>
		{/if}
	{/if}
</section>
