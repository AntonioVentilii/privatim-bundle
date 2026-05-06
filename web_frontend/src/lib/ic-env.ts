/**
 * Reads the asset canister's `ic_env` cookie at runtime — the Cloud Engines
 * installer plants it on every HTML response. Falls back to `VITE_*`
 * build-time vars in dev (when no cookie is present).
 *
 * We deliberately do NOT use `safeGetCanisterEnv()` from the SDK; it rejects
 * the whole env when `ic_root_key` is missing, and the installer doesn't
 * populate that key. We pick up canister IDs from the cookie ourselves and
 * fetch the root key separately via `agent.fetchRootKey()`.
 */
function readCookie(): Record<string, string> {
	try {
		if (typeof document === 'undefined') return {};
		const cookie = document.cookie
			.split(';')
			.map((c) => c.trim())
			.find((c) => c.startsWith('ic_env='));
		if (!cookie) return {};
		const raw = decodeURIComponent(cookie.slice('ic_env='.length));
		const out: Record<string, string> = {};
		for (const pair of raw.split('&')) {
			const i = pair.indexOf('=');
			if (i > 0) out[pair.slice(0, i)] = pair.slice(i + 1);
		}
		return out;
	} catch {
		return {};
	}
}

let _cache: Record<string, string> | null = null;
function env(): Record<string, string> {
	if (_cache !== null) return _cache;
	_cache = readCookie();
	return _cache;
}

export function getAppBackendId(): string {
	const fromCookie = env()['PUBLIC_CANISTER_ID:app_backend'];
	if (fromCookie) return fromCookie;
	const fromBuild = import.meta.env.VITE_CANISTER_ID_APP_BACKEND;
	if (fromBuild) return fromBuild;
	throw new Error(
		'Missing app_backend canister id: neither ic_env cookie nor VITE_CANISTER_ID_APP_BACKEND is set'
	);
}

export function getAiAssistantId(): string {
	const fromCookie = env()['PUBLIC_CANISTER_ID:ai_assistant'];
	if (fromCookie) return fromCookie;
	const fromBuild = import.meta.env.VITE_CANISTER_ID_AI_ASSISTANT;
	if (fromBuild) return fromBuild;
	throw new Error(
		'Missing ai_assistant canister id: neither ic_env cookie nor VITE_CANISTER_ID_AI_ASSISTANT is set'
	);
}

const II_DEFAULT = 'uqzsh-gqaaa-aaaaq-qaada-cai';

export function getIiCanisterId(): string {
	return env()['PUBLIC_II_CANISTER_ID'] || II_DEFAULT;
}
