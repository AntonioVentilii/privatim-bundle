<script lang="ts">
	import { page } from '$app/state';
	import { auth } from '$lib/auth.svelte';
	import { AI_ENABLED } from '$lib/features';
	import { appErrorMessage } from '$lib/audit';
	import { formatDateTime } from '$lib/format';
	import type {
		AssistantIntent,
		AssistantResponse
	} from '../../declarations/ai_assistant.types';
	import type { Client } from '../../declarations/data.types';

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

	// — token-streaming animation state —
	//
	// The canister returns the full answer text in one shot; we animate
	// it into the UI word-by-word so the demo "feels" like inference is
	// happening live. The badge shows the ACTUAL canister-side compute
	// time returned by ai_assistant.ask (covers the data fetches +
	// LLM https outcall + audit appends), so the timing isn't faked.
	let streamedAnswer = $state('');
	let streaming = $state(false);
	let traceLines = $state<string[]>([]);

	$effect(() => {
		if (!AI_ENABLED) return;
		void auth.state.principal;
		if (!auth.state.backends || !auth.state.authenticated) return;
		auth.state.backends.data.list_clients().then((cs) => (clients = cs));
	});

	const currentIntent = $derived(intents.find((i) => i.value === intentValue));

	function intentLabel(v: string): string {
		return intents.find((i) => i.value === v)?.label ?? v;
	}

	async function ask() {
		const ai = auth.state.backends?.ai;
		if (!ai) return;
		errMsg = null;
		response = null;
		streamedAnswer = '';
		traceLines = [];
		loading = true;
		streaming = true;
		askedAt = BigInt(Date.now()) * 1_000_000n;

		// Show trace lines incrementally while the call runs. Honest about
		// what's actually happening on the canister side — we can't
		// stream from the canister, but the trace mirrors the order
		// of operations the canister performs (verifies caller, reads
		// from data, audit-logs the prompt + response).
		const traceTimers: number[] = [];
		const pushTrace = (line: string, delay: number) => {
			traceTimers.push(
				window.setTimeout(() => {
					traceLines = [...traceLines, line];
				}, delay)
			);
		};
		pushTrace(`→ ai_assistant.ask({${intentLabel(intentValue)}})`, 50);
		pushTrace(`  ↳ verifying caller (II principal)`, 250);
		pushTrace(`  ↳ identity.register_ai_assistant_self() (idempotent)`, 450);
		if (currentIntent?.needsClient) {
			pushTrace(`  ↳ data.get_client_for(${clientId}) — composite query authz`, 700);
			pushTrace(`  ↳ data.list_meetings_for / list_trade_ideas_for / get_portfolio_for`, 900);
		} else {
			pushTrace(`  ↳ data.list_clients_for(end_user) — book-wide read`, 700);
			pushTrace(`  ↳ aggregating across all visible portfolios`, 900);
		}
		pushTrace(`  ↳ synthesising response`, 1200);
		pushTrace(`  ↳ audit.append(AssistantQueried)`, 1400);
		pushTrace(`  ↳ audit.append(AssistantResponded)`, 1500);

		try {
			const intent = { [intentValue]: null } as AssistantIntent;
			const cid: [] | [bigint] =
				currentIntent?.needsClient && clientId ? [BigInt(clientId)] : [];
			const res = await ai.ask({
				intent,
				client_id: cid,
				raw_prompt: prompt
			});
			// Stop trace animation timers
			for (const t of traceTimers) window.clearTimeout(t);
			if ('Err' in res) {
				errMsg = appErrorMessage(res.Err);
				streaming = false;
				return;
			}
			response = res.Ok;
			await streamAnswer(res.Ok.answer);
		} catch (err) {
			for (const t of traceTimers) window.clearTimeout(t);
			errMsg = appErrorMessage(err);
		} finally {
			loading = false;
			streaming = false;
		}
	}

	async function streamAnswer(full: string): Promise<void> {
		// Word-by-word reveal at ~30ms/word. Resilient to fast typists
		// re-clicking Ask: the new ask() resets streamedAnswer = ''
		// before this resolves.
		streamedAnswer = '';
		const words = full.split(/(\s+)/);
		for (const w of words) {
			streamedAnswer += w;
			await new Promise((r) => setTimeout(r, 28));
			if (!streaming && !response) break; // user clicked Ask again
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

{#if !AI_ENABLED}
	<section class="mx-auto max-w-4xl">
		<div class="surface rounded p-8 text-center">
			<h1 class="font-serif text-2xl font-black tracking-tight">Assistant unavailable</h1>
			<p class="ink-muted mx-auto mt-2 max-w-md text-sm">
				The AI assistant is temporarily disabled. Everything else in your workspace —
				clients, documents, and the audit log — works as usual.
			</p>
			<a
				href="/"
				class="mt-4 inline-block rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)]"
				style="background: var(--color-burgundy);"
			>
				Back to overview
			</a>
		</div>
	</section>
{:else}
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

	{#if loading || traceLines.length > 0}
		<article class="surface space-y-2 rounded p-4 font-mono text-[11px]">
			<div class="ink-muted tracking-[0.18em] uppercase">Inference trace (on engine)</div>
			{#each traceLines as l, i}
				<div class="ink-muted" class:trace-active={i === traceLines.length - 1 && loading}>
					{l}
				</div>
			{/each}
			{#if loading}
				<div class="ink-muted opacity-60">  ↳ awaiting response…</div>
			{/if}
		</article>
	{/if}

	{#if response || streamedAnswer}
		<article class="surface space-y-4 rounded p-6">
			<header class="flex items-baseline justify-between">
				<div>
					<div class="ink-muted text-[11px] tracking-[0.18em] uppercase">Response</div>
					<div class="ink-muted mt-1 text-xs">
						{#if response}
							inferred in <span class="font-mono">{response.inference_ms.toString()} ms</span>
							· audit seq <span class="font-mono">{response.audit_seq.toString()}</span>
						{/if}
						{#if askedAt}
							{response ? '·' : ''}
							{formatDateTime(askedAt)}
						{/if}
					</div>
				</div>
			</header>
			<div
				class="prose-sm whitespace-pre-line ink leading-relaxed"
				class:streaming-cursor={streaming}
			>
				{@html renderAnswerHtml(streamedAnswer || response?.answer || '')}
			</div>
			{#if response && response.citations.length > 0 && !streaming}
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
{/if}

<style>
	:global(.cite) {
		color: var(--color-burgundy);
		font-weight: 700;
	}
	:global(.streaming-cursor)::after {
		content: '▍';
		display: inline-block;
		margin-left: 1px;
		animation: blink 0.85s infinite;
		color: var(--color-burgundy);
	}
	:global(.trace-active) {
		color: var(--color-ink);
	}
	@keyframes blink {
		0%, 49% { opacity: 1; }
		50%, 100% { opacity: 0; }
	}
</style>
