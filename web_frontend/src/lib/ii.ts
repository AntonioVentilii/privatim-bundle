import { getIiCanisterId } from './ic-env';

function deriveIiUrlFromWindow(iiCanisterId: string): string | null {
	if (typeof window === 'undefined' || !iiCanisterId) return null;
	const { protocol, hostname, port } = window.location;
	const firstDot = hostname.indexOf('.');
	if (firstDot <= 0) return null;
	const firstLabel = hostname.slice(0, firstDot);
	const rest = hostname.slice(firstDot + 1);
	if (!/^[a-z0-9-]+$/i.test(firstLabel) || !firstLabel.includes('-')) return null;
	const portPart = port ? `:${port}` : '';
	return `${protocol}//${iiCanisterId}.${rest}${portPart}`;
}

export function getIdentityProviderUrl(): string {
	return deriveIiUrlFromWindow(getIiCanisterId()) || 'https://id.ai';
}
