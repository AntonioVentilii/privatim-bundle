<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { auth } from '$lib/auth.svelte';
	import { shortPrincipal } from '$lib/format';
	import { AI_ENABLED } from '$lib/features';

	let { children } = $props();

	const navItems = [
		{
			href: '/',
			label: 'Overview',
			match: (p: string) => p === '/'
		},
		{
			href: '/clients',
			label: 'Clients',
			match: (p: string) => p.startsWith('/clients')
		},
		{
			href: '/assistant',
			label: 'Assistant',
			match: (p: string) => p.startsWith('/assistant')
		},
		{
			href: '/audit',
			label: 'Audit log',
			match: (p: string) => p.startsWith('/audit')
		},
		{
			href: '/admin/compliance',
			label: 'Compliance export',
			match: (p: string) => p.startsWith('/admin/compliance'),
			requires: 'Compliance' as const
		},
		{
			href: '/admin/roles',
			label: 'Roles',
			match: (p: string) => p.startsWith('/admin/roles'),
			requires: 'Admin' as const
		},
		{
			href: '/admin/bootstrap',
			label: 'Bootstrap',
			match: (p: string) => p.startsWith('/admin/bootstrap'),
			requires: 'Admin' as const
		}
	];

	onMount(async () => {
		await auth.init();
	});

	function visibleItems() {
		return navItems.filter((i) => {
			if (!AI_ENABLED && i.href === '/assistant') return false;
			return !i.requires || auth.hasRole(i.requires) || auth.hasRole('Admin');
		});
	}
</script>

<div class="paper-grain flex min-h-screen flex-col">
	<header class="rule-line border-b">
		<div class="mx-auto flex w-full max-w-7xl items-center justify-between gap-4 px-8 py-5">
			<a href="/" class="flex items-center gap-3">
				<div
					class="flex size-10 items-center justify-center rounded font-serif text-xl font-black text-[var(--color-paper)]"
					style="background: var(--color-burgundy);"
				>
					P
				</div>
				<div class="leading-tight">
					<div class="font-serif text-xl font-black tracking-tight">Privatim</div>
					<div class="ink-muted -mt-0.5 text-[11px] tracking-[0.18em] uppercase">
						Sovereign banking workspace
					</div>
				</div>
			</a>

			<div class="flex items-center gap-3">
				{#if auth.state.authenticated && auth.state.principal}
					<div class="ink-muted hidden flex-col items-end leading-tight md:flex">
						<span class="text-[10px] tracking-[0.18em] uppercase">Signed in</span>
						<span class="ink font-mono text-xs" title={auth.state.principal.toText()}>
							{shortPrincipal(auth.state.principal)}
						</span>
						{#if auth.state.roles.length > 0}
							<span class="text-[10px] uppercase tracking-[0.18em]">
								{auth.state.roles.map((r) => Object.keys(r)[0]).join(' · ')}
							</span>
						{/if}
					</div>
					<button
						type="button"
						onclick={() => auth.logout()}
						class="surface ink-muted hover:ink rounded px-3 py-1.5 text-xs"
					>
						Sign out
					</button>
				{:else if auth.state.ready}
					<button
						type="button"
						onclick={() => auth.login()}
						class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)]"
						style="background: var(--color-burgundy);"
					>
						Sign in with Internet Identity
					</button>
				{:else}
					<div class="ink-muted text-xs">Loading…</div>
				{/if}
			</div>
		</div>

		<nav class="rule-line mx-auto max-w-7xl border-t px-8">
			<div class="flex flex-wrap gap-x-6 gap-y-1 py-2">
				{#each visibleItems() as item}
					{@const active = item.match(page.url.pathname)}
					<a
						href={item.href}
						class="ink-muted hover:ink relative py-1 text-sm transition {active
							? 'ink font-bold'
							: ''}"
						style={active
							? 'border-bottom: 2px solid var(--color-burgundy); padding-bottom: 2px;'
							: ''}
					>
						{item.label}
					</a>
				{/each}
			</div>
		</nav>
	</header>

	<main class="mx-auto w-full max-w-7xl flex-1 px-8 py-8">
		{@render children()}
	</main>

	<footer class="rule-line ink-muted border-t px-8 py-6 text-center text-xs">
		Privatim · Cloud Engines bundle · jurisdiction-locked, audit-chained, on-engine AI
	</footer>
</div>
