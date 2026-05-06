<script lang="ts">
	import { page } from '$app/state';
	import { auth } from '$lib/auth.svelte';
	import { appErrorMessage } from '$lib/audit';
	import { formatDateTime } from '$lib/format';
	import type {
		AssistantIntent,
		AssistantResponse
	} from '../../declarations/ai_assistant.types';
	import type { Client } from '../../declarations/app_backend.types';

	const intents: { value: keyof AssistantIntent | string; label: string; needsClient: boolean }[] = [
		{ value: 'PortfolioOverview', label: 'Portfolio overview', needsClient: true },
		{ value: 'RiskAssessment', label: 'Risk assessment', needsClient: true },
		{ value: 'KycStatus', label: 'KYC status', needsClient: true },
		{ value: 'MeetingDigest', label: 'Recent meetings digest', needsClient: true },
		{ value: 'OpenTradeIdeas', label: 'Open trade ideas', needsClient: true },
		{ value: 'FxExposureBook', label: 'FX exposure across my book', needsClient: false },
		{ value: 'KycActionList', label: 'KYC action list', needsClient: false }
	];

	let clients = $state<Client[]>([]);
	let intentValue = $state<string>('PortfolioOverview');
	let clientId = $state<string>(page.url.searchParams.get('client') ?? '');
	let prompt = $state('');
	let loading = $state(false);
	let response = $state<AssistantResponse | null>(null);
	let errMsg = $state<string | null>(null);
	let askedAt = $state<bigint | null>(null);

	$effect(() => {
		void auth.state.principal;
		if (!auth.state.app) return;
		auth.state.app.list_clients().then((cs) => (clients = cs));
	});

	const currentIntent = $derived(intents.find((i) => i.value === intentValue));

	async function ask() {
		const ai = auth.state.ai;
		if (!ai) return;
		errMsg = null;
		response = null;
		loading = true;
		askedAt = BigInt(Date.now()) * 1_000_000n;
		try {
			const intent = { [intentValue]: null } as AssistantIntent;
			const cid: [] | [bigint] =
				currentIntent?.needsClient && clientId ? [BigInt(clientId)] : [];
			const res = await ai.ask({
				intent,
				client_id: cid,
				raw_prompt: prompt
			});
			if ('Err' in res) {
				errMsg = appErrorMessage(res.Err);
				return;
			}
			response = res.Ok;
		} catch (err) {
			errMsg = appErrorMessage(err);
		} finally {
			loading = false;
		}
	}

	function renderAnswerHtml(answer: string): string {
		const escaped = answer
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;');
		return escaped
			.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
			.replace(/\[#(\d+)\]/g, '<sup class="cite">[$1]</sup>')
			.replace(/\n/g, '<br>');
	}
</script>

<section class="mx-auto max-w-4xl space-y-6">
	<header class="space-y-2">
		<div
			class="ink-muted inline-flex items-center gap-2 text-[11px] tracking-[0.18em] uppercase"
		>
			<span class="size-1.5 rounded-full" style="background: var(--color-copper);"></span>
			Stub LLM · production runs on this engine's GPU node
		</div>
		<h1 class="font-serif text-3xl font-black tracking-tight">Assistant</h1>
		<p class="ink-muted text-sm">
			Ask structured questions over the data you're already authorised to see. Every
			question and answer is appended to the audit chain.
		</p>
	</header>

	<div class="surface space-y-4 rounded p-6">
		<label class="block">
			<span class="ink-muted text-xs tracking-[0.18em] uppercase">Intent</span>
			<select
				bind:value={intentValue}
				class="rule-line surface mt-1 w-full rounded border px-3 py-2 text-sm"
			>
				{#each intents as i}
					<option value={i.value}>{i.label}</option>
				{/each}
			</select>
		</label>

		{#if currentIntent?.needsClient}
			<label class="block">
				<span class="ink-muted text-xs tracking-[0.18em] uppercase">Client</span>
				<select
					bind:value={clientId}
					class="rule-line surface mt-1 w-full rounded border px-3 py-2 text-sm"
					required
				>
					<option value="">Select a client…</option>
					{#each clients as c}
						<option value={c.id.toString()}>{c.display_name}</option>
					{/each}
				</select>
			</label>
		{/if}

		<label class="block">
			<span class="ink-muted text-xs tracking-[0.18em] uppercase">Free-text prompt (optional)</span>
			<textarea
				bind:value={prompt}
				rows="2"
				placeholder="Surfaced for the audit log only — interpretation lives in the intent."
				class="rule-line surface mt-1 w-full rounded border px-3 py-2 text-sm"
			></textarea>
		</label>

		<button
			type="button"
			onclick={ask}
			disabled={loading || (currentIntent?.needsClient && !clientId)}
			class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)] disabled:opacity-50"
			style="background: var(--color-burgundy);"
		>
			{loading ? 'Thinking…' : 'Ask'}
		</button>
	</div>

	{#if errMsg}
		<div
			class="surface rounded border-l-4 p-4 text-sm"
			style="border-color: var(--color-bad); color: var(--color-bad);"
		>
			{errMsg}
		</div>
	{/if}

	{#if response}
		<article class="surface space-y-4 rounded p-6">
			<header class="flex items-baseline justify-between">
				<div>
					<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">Response</div>
					<div class="ink-muted mt-1 text-xs">
						Model: <span class="font-mono">{response.model}</span> · audit seq:
						<span class="font-mono">{response.audit_seq.toString()}</span>
						{#if askedAt}
							· {formatDateTime(askedAt)}
						{/if}
					</div>
				</div>
			</header>
			<div class="prose-sm whitespace-pre-line ink leading-relaxed">
				{@html renderAnswerHtml(response.answer)}
			</div>
			{#if response.citations.length > 0}
				<footer class="rule-line space-y-1 border-t pt-3">
					<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">Citations</div>
					<ol class="ml-4 list-decimal space-y-0.5 text-xs">
						{#each response.citations as c, i}
							<li>
								<span class="ink-muted">[{i}]</span>
								<span class="ink-muted text-xs">
									{Object.keys(c.kind)[0]} #{c.id.toString()}
								</span>
								— {c.label}
							</li>
						{/each}
					</ol>
				</footer>
			{/if}
		</article>
	{/if}
</section>

<style>
	:global(.cite) {
		color: var(--color-burgundy);
		font-weight: 700;
	}
</style>
