<script lang="ts">
	import { auth } from '$lib/auth.svelte';
	import { Principal } from '@dfinity/principal';
	import { appErrorMessage } from '$lib/audit';
	import type { Role } from '../../../declarations/identity.types';

	let granteeText = $state('');
	let role = $state<'Advisor' | 'Compliance' | 'Admin'>('Advisor');
	let assignClient = $state('');
	let msg = $state<string | null>(null);

	function asRole(r: 'Advisor' | 'Compliance' | 'Admin'): Role {
		return { [r]: null } as Role;
	}

	async function grantOrRevoke(action: 'grant' | 'revoke') {
		const id = auth.state.backends?.identity;
		if (!id) return;
		msg = null;
		try {
			const p = Principal.fromText(granteeText.trim());
			const res =
				action === 'grant'
					? await id.grant_role(p, asRole(role))
					: await id.revoke_role(p, asRole(role));
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

	async function assignClientToMe() {
		const id = auth.state.backends?.identity;
		const p = auth.state.principal;
		if (!id || !p) return;
		msg = null;
		try {
			const cid = BigInt(assignClient.trim());
			const res = await id.assign_client(p, cid);
			if ('Err' in res) {
				msg = appErrorMessage(res.Err);
				return;
			}
			msg = `Assigned client #${cid.toString()} to you`;
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
			<code>Compliance</code> (sees full audit log + runs exports),
			<code>Admin</code> (grants roles + assigns clients).
		</p>
		<p class="ink-muted text-xs">
			One-shot wiring of inter-canister principals lives at
			<a href="/admin/bootstrap" class="underline">/admin/bootstrap</a>.
		</p>
	</header>

	{#if !auth.hasRole('Admin')}
		<div class="surface rounded p-8 text-center">
			<p class="ink-muted text-sm">
				Admin role required. The first authenticated principal becomes Admin automatically.
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
					onclick={() => grantOrRevoke('grant')}
					class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)]"
					style="background: var(--color-burgundy);"
				>
					Grant
				</button>
				<button
					type="button"
					onclick={() => grantOrRevoke('revoke')}
					class="surface ink-muted hover:ink rounded px-4 py-2 text-sm"
				>
					Revoke
				</button>
			</div>
		</div>

		<div class="surface space-y-3 rounded p-6">
			<h2 class="font-serif text-lg font-black">Assign a client to yourself</h2>
			<p class="ink-muted text-sm">
				Demo helper. Seeded clients are owned by synthetic advisor principals; this gives
				you visibility on a specific client by client_id.
			</p>
			<label class="block">
				<span class="ink-muted text-xs tracking-[0.18em] uppercase">Client ID</span>
				<input
					type="number"
					bind:value={assignClient}
					placeholder="0"
					class="rule-line surface mt-1 w-full rounded border px-3 py-2 text-sm"
				/>
			</label>
			<button
				type="button"
				onclick={assignClientToMe}
				class="surface ink-muted hover:ink rounded px-4 py-2 text-sm"
			>
				Assign to me
			</button>
		</div>

		{#if msg}
			<div class="surface rounded p-4 text-sm">{msg}</div>
		{/if}
	{/if}
</section>
