import { getIiCanisterId } from './ic-env';

// Mainnet HTTP gateways. In production the app is served from a canister
// subdomain on one of these (e.g. abc-….icp0.io); sign-in must go to
// https://id.ai, since Internet Identity passkeys are scoped to the id.ai
// origin and would not resolve against a canister-origin II URL. Only local
// replicas (…localhost) and dev farms (…farm.dfinity.systems) derive a
// per-gateway II URL from the window.
const MAINNET_GATEWAYS = ['icp0.io', 'ic0.app'];

function deriveIiUrlFromWindow(iiCanisterId: string): string | null {
	if (typeof window === 'undefined' || !iiCanisterId) return null;
	const { protocol, hostname, port } = window.location;
	const firstDot = hostname.indexOf('.');
	if (firstDot <= 0) return null;
	const firstLabel = hostname.slice(0, firstDot);
	const rest = hostname.slice(firstDot + 1);
	if (!/^[a-z0-9-]+$/i.test(firstLabel) || !firstLabel.includes('-')) return null;
	if (MAINNET_GATEWAYS.some((g) => rest === g || rest.endsWith(`.${g}`))) return null;
	const portPart = port ? `:${port}` : '';
	return `${protocol}//${iiCanisterId}.${rest}${portPart}`;
}

export function getIdentityProviderUrl(): string {
	return deriveIiUrlFromWindow(getIiCanisterId()) || 'https://id.ai';
}
