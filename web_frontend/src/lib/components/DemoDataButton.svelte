<script lang="ts">
	// Admin/controller-only control that re-seeds the data canister with
	// synthetic clients, portfolios, meetings, and trade ideas via
	// `reset_demo()`. Confirms first (it wipes and re-seeds), then dispatches
	// `seeded` so the calling page can reload. Lets showcase users populate the
	// workspace on demand rather than relying on install-time seeding.
	import { auth } from '$lib/auth.svelte';
	import { appErrorMessage } from '$lib/audit';

	let { onseeded }: { onseeded?: (count: number) => void } = $props();

	let busy = $state(false);
	let msg = $state<string | null>(null);
	let error = $state(false);

	const canSeed = $derived(auth.hasRole('Admin'));

	async function seed() {
		const data = auth.state.backends?.data;
		if (!data || busy) return;
		if (
			!window.confirm(
				'Re-seed the workspace with synthetic clients? This replaces the current demo data.'
			)
		) {
			return;
		}
		busy = true;
		msg = null;
		error = false;
		try {
			const res = await data.reset_demo();
			if ('Err' in res) {
				error = true;
				msg = appErrorMessage(res.Err);
				return;
			}
			const count = Number(res.Ok);
			msg = `Seeded ${count} clients.`;
			onseeded?.(count);
		} catch (err) {
			error = true;
			msg = err instanceof Error ? err.message : String(err);
		} finally {
			busy = false;
		}
	}
</script>

{#if canSeed}
	<div class="flex items-center gap-3">
		<button
			type="button"
			onclick={seed}
			disabled={busy}
			class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)] disabled:opacity-50"
			style="background: var(--color-burgundy);"
		>
			{busy ? 'Seeding…' : '✨ Generate demo data'}
		</button>
		{#if msg}
			<span class="ink-muted text-xs" style={error ? 'color: var(--color-bad);' : ''}>{msg}</span>
		{/if}
	</div>
{/if}
