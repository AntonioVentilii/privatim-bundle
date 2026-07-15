/**
 * Reads canister IDs from the asset canister's `ic_env` cookie at runtime,
 * falling back to `VITE_*` build-time vars in dev (when the cookie isn't
 * present). Five canisters in this bundle: identity, audit, data,
 * ai_assistant, plus the asset canister serving the SPA.
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

function lookup(name: string, viteName: string): string {
	const fromCookie = env()[`PUBLIC_CANISTER_ID:${name}`];
	if (fromCookie) return fromCookie;
	const fromBuild = (import.meta.env as Record<string, string | undefined>)[viteName];
	if (fromBuild) return fromBuild;
	throw new Error(
		`Missing ${name} canister id: neither ic_env cookie nor ${viteName} is set`
	);
}

export const getIdentityId = () => lookup('identity', 'VITE_CANISTER_ID_IDENTITY');
export const getAuditId = () => lookup('audit', 'VITE_CANISTER_ID_AUDIT');
export const getDataId = () => lookup('data', 'VITE_CANISTER_ID_DATA');
export const getDocumentsId = () => lookup('documents', 'VITE_CANISTER_ID_DOCUMENTS');
export const getAiAssistantId = () =>
	lookup('ai_assistant', 'VITE_CANISTER_ID_AI_ASSISTANT');
