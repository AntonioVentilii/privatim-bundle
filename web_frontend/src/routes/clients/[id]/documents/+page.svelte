<script lang="ts">
	import { page } from '$app/state';
	import { auth } from '$lib/auth.svelte';
	import { appErrorMessage } from '$lib/audit';
	import { formatDateTime, shortPrincipal } from '$lib/format';
	import {
		decryptDocument,
		downloadBlob,
		dropKey,
		encryptFile,
		exportKey,
		formatBytes,
		generateKey,
		importKey,
		loadKey,
		storeKey,
		verifyPlaintextSha
	} from '$lib/document-crypto';
	import type {
		DocumentMeta
	} from '../../../../declarations/documents.types';

	let docs = $state<DocumentMeta[]>([]);
	let loading = $state(true);
	let errMsg = $state<string | null>(null);
	let okMsg = $state<string | null>(null);

	let file = $state<File | null>(null);
	let label = $state('');
	let uploading = $state(false);
	let uploadProgress = $state<string | null>(null);

	const clientId = $derived(BigInt(page.params.id ?? '0'));

	async function load() {
		const b = auth.state.backends;
		if (!b || !auth.state.authenticated) return;
		loading = true;
		try {
			const res = await b.documents.list_documents(clientId);
			if ('Err' in res) {
				errMsg = appErrorMessage(res.Err);
				docs = [];
			} else {
				docs = res.Ok;
			}
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		void clientId;
		void auth.state.principal;
		if (auth.state.backends && auth.state.authenticated) load();
	});

	function onFileChange(e: Event) {
		const input = e.target as HTMLInputElement;
		const f = input.files?.[0];
		if (f) {
			file = f;
			if (!label) label = f.name;
		}
	}

	async function upload() {
		const b = auth.state.backends;
		if (!b || !file) return;
		errMsg = null;
		okMsg = null;
		uploading = true;
		try {
			uploadProgress = 'Generating AES-256 key…';
			const key = await generateKey();
			const exported = await exportKey(key);

			uploadProgress = 'Encrypting in your browser…';
			const { iv, ciphertext, plaintextSha256 } = await encryptFile(file, key);

			uploadProgress = `Uploading ${formatBytes(ciphertext.length)} of ciphertext…`;
			const res = await b.documents.upload_document({
				client_id: clientId,
				label: label.trim() || file.name,
				iv,
				ciphertext,
				plaintext_sha256: plaintextSha256
			});
			if ('Err' in res) {
				errMsg = appErrorMessage(res.Err);
				return;
			}
			storeKey(res.Ok, exported);
			okMsg = `Uploaded as document #${res.Ok}. Key cached locally.`;
			file = null;
			label = '';
			(document.getElementById('doc-file') as HTMLInputElement | null)?.value &&
				((document.getElementById('doc-file') as HTMLInputElement).value = '');
			await load();
		} catch (err) {
			errMsg = appErrorMessage(err);
		} finally {
			uploading = false;
			uploadProgress = null;
		}
	}

	async function downloadDoc(meta: DocumentMeta) {
		const b = auth.state.backends;
		if (!b) return;
		errMsg = null;
		okMsg = null;
		const keyB64 = loadKey(meta.id);
		if (!keyB64) {
			errMsg = `No decryption key found in this browser for doc #${meta.id}. Documents are encrypted with browser-only keys; if you uploaded from a different browser or cleared localStorage, the document is unrecoverable.`;
			return;
		}
		try {
			const res = await b.documents.fetch_document(meta.id);
			if ('Err' in res) {
				errMsg = appErrorMessage(res.Err);
				return;
			}
			const key = await importKey(keyB64);
			const plain = await decryptDocument(res.Ok.ciphertext, res.Ok.iv, key);
			const ok = await verifyPlaintextSha(plain, res.Ok.plaintext_sha256);
			if (!ok) {
				errMsg =
					'Plaintext SHA-256 mismatch — file integrity check failed. Aborting download.';
				return;
			}
			downloadBlob(plain, meta.label);
			okMsg = `Decrypted ${formatBytes(plain.length)} and triggered download.`;
		} catch (err) {
			errMsg = appErrorMessage(err);
		}
	}

	async function deleteDoc(meta: DocumentMeta) {
		const b = auth.state.backends;
		if (!b) return;
		if (!confirm(`Delete "${meta.label}"? This cannot be undone.`)) return;
		errMsg = null;
		okMsg = null;
		try {
			const res = await b.documents.delete_document(meta.id);
			if ('Err' in res) {
				errMsg = appErrorMessage(res.Err);
				return;
			}
			dropKey(meta.id);
			okMsg = `Deleted document #${meta.id} (and dropped its key).`;
			await load();
		} catch (err) {
			errMsg = appErrorMessage(err);
		}
	}

	function hasKey(id: bigint): boolean {
		return loadKey(id) !== null;
	}
</script>

<div class="space-y-6">
	<a class="ink-muted text-xs hover:underline" href={`/clients/${clientId}`}>← back to client</a>

	<header class="space-y-2">
		<div
			class="ink-muted inline-flex items-center gap-2 text-[11px] tracking-[0.18em] uppercase"
		>
			Client #{clientId.toString()} · client-side AES-256-GCM
		</div>
		<h1 class="font-serif text-3xl font-black tracking-tight">Documents</h1>
		<p class="ink-muted max-w-3xl text-sm">
			KYC files, signed mandates, contracts. Each document is encrypted in your browser
			with a fresh AES-256 key before upload. The canister stores ciphertext only — neither
			the engine creator nor a future canister controller can decrypt without the key,
			which lives in your browser's localStorage and never leaves this device.
		</p>
	</header>

	<section class="surface space-y-3 rounded p-6">
		<h2 class="font-serif text-lg font-black">Upload</h2>
		<label class="block">
			<span class="ink-muted text-xs tracking-[0.18em] uppercase">File</span>
			<input
				id="doc-file"
				type="file"
				onchange={onFileChange}
				class="rule-line surface mt-1 w-full rounded border px-3 py-2 text-sm"
			/>
		</label>
		<label class="block">
			<span class="ink-muted text-xs tracking-[0.18em] uppercase">Label</span>
			<input
				type="text"
				bind:value={label}
				placeholder="e.g. KYC chart 2026 Q1.pdf"
				class="rule-line surface mt-1 w-full rounded border px-3 py-2 text-sm"
			/>
		</label>
		<button
			type="button"
			onclick={upload}
			disabled={uploading || !file}
			class="rounded px-4 py-2 text-sm font-bold text-[var(--color-paper)] disabled:opacity-50"
			style="background: var(--color-burgundy);"
		>
			{uploading ? 'Encrypting + uploading…' : 'Encrypt + upload'}
		</button>
		{#if uploadProgress}
			<div class="ink-muted text-xs">{uploadProgress}</div>
		{/if}
	</section>

	{#if errMsg}
		<div
			class="surface rounded border-l-4 p-4 text-sm"
			style="border-color: var(--color-bad); color: var(--color-bad);"
		>
			{errMsg}
		</div>
	{/if}
	{#if okMsg}
		<div
			class="surface rounded border-l-4 p-4 text-sm"
			style="border-color: var(--color-good); color: var(--color-good);"
		>
			{okMsg}
		</div>
	{/if}

	<section class="space-y-3">
		<h2 class="font-serif text-lg font-black">Stored documents</h2>
		{#if loading && docs.length === 0}
			<div class="ink-muted text-sm">Loading…</div>
		{:else if docs.length === 0}
			<div class="ink-muted text-sm">No documents on file for this client.</div>
		{:else}
			<div class="surface overflow-hidden rounded">
				<table class="w-full text-sm">
					<thead class="rule-line border-b text-left">
						<tr class="ink-muted text-[11px] tracking-[0.18em] uppercase">
							<th class="px-4 py-3 font-bold">Label</th>
							<th class="px-4 py-3 font-bold">Size</th>
							<th class="px-4 py-3 font-bold">Uploaded</th>
							<th class="px-4 py-3 font-bold">Plaintext SHA-256</th>
							<th class="px-4 py-3 text-right font-bold">Actions</th>
						</tr>
					</thead>
					<tbody>
						{#each docs as d (d.id)}
							{@const known = hasKey(d.id)}
							<tr class="rule-line border-b last:border-b-0">
								<td class="px-4 py-3">
									<div class="font-bold">{d.label}</div>
									<div class="ink-muted font-mono text-[10px]">
										#{d.id.toString()} · by {shortPrincipal(d.uploaded_by)}
									</div>
								</td>
								<td class="px-4 py-3 font-mono text-xs">{formatBytes(d.size_bytes)}</td>
								<td class="px-4 py-3 font-mono text-xs">
									{formatDateTime(d.uploaded_at_ns)}
								</td>
								<td class="px-4 py-3 font-mono text-[10px]" title={d.plaintext_sha256}>
									{d.plaintext_sha256.slice(0, 16)}…
								</td>
								<td class="px-4 py-3 text-right">
									<button
										type="button"
										onclick={() => downloadDoc(d)}
										disabled={!known}
										title={known ? 'Decrypt + download' : 'No key in this browser'}
										class="rounded px-3 py-1 text-xs font-bold text-[var(--color-paper)] disabled:opacity-40"
										style="background: var(--color-burgundy);"
									>
										{known ? 'Decrypt' : 'No key'}
									</button>
									<button
										type="button"
										onclick={() => deleteDoc(d)}
										class="ml-1 rounded px-3 py-1 text-xs"
										style="color: var(--color-bad);"
									>
										Delete
									</button>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</section>

	<section class="surface rounded border-l-4 p-4 text-xs" style="border-color: var(--color-copper);">
		<div class="ink-muted mb-1 tracking-[0.18em] uppercase">Sovereignty model</div>
		<p>
			Each document gets a fresh random 256-bit AES-GCM key. Plaintext is encrypted in this
			browser; only ciphertext + IV go to the canister. The key lives in localStorage and
			never leaves your device. <strong>If you lose this browser's localStorage you lose the
			ability to decrypt these documents</strong> — there's no key escrow and no cross-device
			sync today. A real product would use vetkeys (when stable) or threshold-signed key
			wrapping among advisors.
		</p>
	</section>
</div>
