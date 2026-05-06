/**
 * Client-side AES-256-GCM document encryption for the documents canister.
 *
 * Sovereignty model:
 *
 * - Each document gets a fresh, random 256-bit AES-GCM key generated in the
 *   user's browser via `crypto.getRandomValues`.
 * - The plaintext is encrypted in-browser; only the ciphertext + IV are
 *   uploaded to the canister. The canister never sees plaintext or key.
 * - The key is base64-encoded and stored in `localStorage` keyed by
 *   `<localStorage_prefix>:<doc_id>`, scoped to the user's browser. An engine
 *   creator with full canister-controller access cannot decrypt — there's
 *   nothing on the canister to decrypt with.
 *
 * Tradeoffs (showcase-grade, named in PITCH.md):
 *
 * - Lose the browser, lose the document (no cross-device key sync, no
 *   key escrow). Production uses vetkeys when stable, or threshold-signed
 *   key wrapping among advisors.
 * - No "share with another advisor" flow today. The uploader's localStorage
 *   is the only place the key lives.
 */

const ALGO_NAME = 'AES-GCM';
const KEY_BITS = 256;
const IV_BYTES = 12;
const STORAGE_PREFIX = 'privatim:doc-key:v1';

function bytesToBase64(bytes: Uint8Array): string {
	let binary = '';
	for (let i = 0; i < bytes.byteLength; i++) {
		binary += String.fromCharCode(bytes[i]!);
	}
	return btoa(binary);
}

function base64ToBytes(b64: string): Uint8Array {
	const binary = atob(b64);
	const out = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) {
		out[i] = binary.charCodeAt(i);
	}
	return out;
}

function toUint8(arr: Uint8Array | number[]): Uint8Array {
	return arr instanceof Uint8Array ? arr : new Uint8Array(arr);
}

function asArrayBuffer(arr: Uint8Array | number[]): ArrayBuffer {
	const u = toUint8(arr);
	return u.buffer.slice(u.byteOffset, u.byteOffset + u.byteLength) as ArrayBuffer;
}

export async function generateKey(): Promise<CryptoKey> {
	return crypto.subtle.generateKey({ name: ALGO_NAME, length: KEY_BITS }, true, [
		'encrypt',
		'decrypt'
	]);
}

export function generateIv(): Uint8Array {
	return crypto.getRandomValues(new Uint8Array(IV_BYTES));
}

export async function exportKey(key: CryptoKey): Promise<string> {
	const raw = await crypto.subtle.exportKey('raw', key);
	return bytesToBase64(new Uint8Array(raw));
}

export async function importKey(b64: string): Promise<CryptoKey> {
	const raw = base64ToBytes(b64);
	return crypto.subtle.importKey(
		'raw',
		raw.buffer.slice(raw.byteOffset, raw.byteOffset + raw.byteLength) as ArrayBuffer,
		ALGO_NAME,
		true,
		['encrypt', 'decrypt']
	);
}

export interface EncryptedPayload {
	iv: Uint8Array;
	ciphertext: Uint8Array;
	plaintextSha256: string;
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
	const digest = await crypto.subtle.digest(
		'SHA-256',
		bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer
	);
	return Array.from(new Uint8Array(digest))
		.map((b) => b.toString(16).padStart(2, '0'))
		.join('');
}

export async function encryptFile(
	file: File,
	key: CryptoKey
): Promise<EncryptedPayload> {
	const plain = new Uint8Array(await file.arrayBuffer());
	const iv = generateIv();
	const ct = await crypto.subtle.encrypt(
		{ name: ALGO_NAME, iv: iv.buffer.slice(0) as ArrayBuffer },
		key,
		plain.buffer.slice(0) as ArrayBuffer
	);
	const plaintextSha256 = await sha256Hex(plain);
	return { iv, ciphertext: new Uint8Array(ct), plaintextSha256 };
}

export async function decryptDocument(
	ciphertext: Uint8Array | number[],
	iv: Uint8Array | number[],
	key: CryptoKey
): Promise<Uint8Array> {
	const plain = await crypto.subtle.decrypt(
		{ name: ALGO_NAME, iv: asArrayBuffer(iv) },
		key,
		asArrayBuffer(ciphertext)
	);
	return new Uint8Array(plain);
}

// ─────────── localStorage key cache ───────────

export function storeKey(docId: bigint, b64: string): void {
	if (typeof localStorage === 'undefined') return;
	localStorage.setItem(`${STORAGE_PREFIX}:${docId}`, b64);
}

export function loadKey(docId: bigint): string | null {
	if (typeof localStorage === 'undefined') return null;
	return localStorage.getItem(`${STORAGE_PREFIX}:${docId}`);
}

export function dropKey(docId: bigint): void {
	if (typeof localStorage === 'undefined') return;
	localStorage.removeItem(`${STORAGE_PREFIX}:${docId}`);
}

export function listKnownKeys(): bigint[] {
	if (typeof localStorage === 'undefined') return [];
	const out: bigint[] = [];
	for (let i = 0; i < localStorage.length; i++) {
		const k = localStorage.key(i);
		if (k && k.startsWith(`${STORAGE_PREFIX}:`)) {
			try {
				out.push(BigInt(k.slice(`${STORAGE_PREFIX}:`.length)));
			} catch {
				/* skip */
			}
		}
	}
	return out;
}

export async function verifyPlaintextSha(
	plaintext: Uint8Array,
	expectedHex: string
): Promise<boolean> {
	const actual = await sha256Hex(plaintext);
	return actual === expectedHex;
}

export function downloadBlob(bytes: Uint8Array, filename: string): void {
	const blob = new Blob([bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer]);
	const url = URL.createObjectURL(blob);
	const a = document.createElement('a');
	a.href = url;
	a.download = filename;
	a.click();
	URL.revokeObjectURL(url);
}

export function formatBytes(n: bigint | number): string {
	const v = typeof n === 'bigint' ? Number(n) : n;
	if (v < 1024) return `${v} B`;
	if (v < 1024 * 1024) return `${(v / 1024).toFixed(1)} KiB`;
	return `${(v / 1024 / 1024).toFixed(2)} MiB`;
}
