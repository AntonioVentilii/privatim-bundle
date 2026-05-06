<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { Principal } from '@dfinity/principal';
	import { appErrorMessage } from '$lib/audit';
	import type { Role } from '../../../declarations/app_backend.types';

	let granteeText = $state('');
	let role = $state<'Advisor' | 'Compliance' | 'Admin'>('Advisor');
	let aiPrincipalText = $state('');
	let msg = $state<string | null>(null);
	let aiPrincipalCurrent = $state<string | null>(null);

	$effect(() => {
		void auth.state.principal;
		if (!auth.state.ai) return;
		auth.state.ai.whoami().then((p) => (aiPrincipalCurrent = p.toText()));
	});

	function asRole(r: 'Advisor' | 'Compliance' | 'Admin'): Role {
		return { [r]: null } as Role;
	}

	async function grant(action: 'grant' | 'revoke') {
		const app = auth.state.app;
		if (!app) return;
		msg = null;
		try {
			const p = Principal.fromText(granteeText.trim());
			const res =
				action === 'grant'
					? await app.grant_role(p, asRole(role))
					: await app.revoke_role(p, asRole(role));
			if ('Err' in res) {
				msg = appErrorMessage(res.Err);
				return;
			}
			msg = `${action === 'grant' ? 'Granted' : 'Revoked'} ${role} ${
				action === 'grant' ? 'to' : 'from'
			} ${granteeText.trim()}`;
		} catch (err) {
			msg = appErrorMessage(err);
		}
	}

	async function admitAi() {
		const app = auth.state.app;
		if (!app) return;
		msg = null;
		try {
			const p = Principal.fromText(aiPrincipalText.trim() || aiPrincipalCurrent || '');
			const res = await app.admit_ai_assistant(p);
			if ('Err' in res) {
				msg = appErrorMessage(res.Err);
				return;
			}
			msg = `Admitted ai_assistant principal ${p.toText()}`;
		} catch (err) {
			msg = appErrorMessage(err);
		}
	}
</script>

<section class="mx-auto max-w-2xl space-y-6">
	<header class="space-y-2">
		<h1 class="font-serif text-3xl font-black tracking-tight">Roles</h1>
		<p class="ink-muted text-sm">
			Admin only. Three roles: <code>Advisor</code> (sees own clients),
			<code>Compliance</code> (sees full audit log, runs exports),
			<code>Admin</code> (can grant roles + reassign clients).
		</p>
	</header>

	{#if !auth.hasRole('Admin')}
		<div class="surface rounded p-8 text-center">
			<p class="ink-muted text-sm">
				Admin role required. The first authenticated principal becomes Admin automatically; ask
				them to grant you the role here if you signed in second.
			</p>
		</div>
	{:else}
		<div class="surface space-y-3 rounded p-6">
			<h2 class="font-serif text-lg font-black">Grant or revoke a role</h2>
			<label class="block">
				<span class="ink-muted text-xs tracking-[0.18em] uppercase">Grantee principal</span>
				<input
					type="text"
					bind:value={granteeText}
					placeholder="abc12-…"
					class="rule-line surface mt-1 w-full rounded border px-3 py-2 font-mono text-xs"
				/>
			</label>
			<label class="block">
				<span class="ink-muted text-xs tracking-[0.18em] uppercase">Role</span>
				<select
					bind:value={role}
					class="rule-line surface mt-1 w-full rounded border px-3 py-2 text-sm"
				>
					<option>Advisor</option>
					<option>Compliance</option>
					<option>Admin</option>
				</select>
			</label>
			<div class="flex gap-2">
				<button
					type="button"
					onclick={() => grant('grant')}
					class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)]"
					style="background: var(--color-burgundy);"
				>
					Grant
				</button>
				<button
					type="button"
					onclick={() => grant('revoke')}
					class="surface ink-muted hover:ink rounded px-4 py-2 text-sm"
				>
					Revoke
				</button>
			</div>
		</div>

		<div class="surface space-y-3 rounded p-6">
			<h2 class="font-serif text-lg font-black">Admit ai_assistant</h2>
			<p class="ink-muted text-sm">
				Grants the AI canister the Admin role on app_backend so it can write
				`AssistantQueried` / `AssistantResponded` audit entries on behalf of users. Run this
				once after deploying both canisters.
			</p>
			<div class="ink-muted text-xs">
				ai_assistant principal:
				<span class="font-mono">{aiPrincipalCurrent ?? '—'}</span>
			</div>
			<input
				type="text"
				bind:value={aiPrincipalText}
				placeholder={aiPrincipalCurrent ?? 'principal of ai_assistant canister'}
				class="rule-line surface w-full rounded border px-3 py-2 font-mono text-xs"
			/>
			<button
				type="button"
				onclick={admitAi}
				class="surface ink-muted hover:ink rounded px-4 py-2 text-sm"
			>
				Admit ai_assistant
			</button>
		</div>

		{#if msg}
			<div class="surface rounded p-4 text-sm">{msg}</div>
		{/if}
	{/if}
</section>
