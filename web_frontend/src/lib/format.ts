export function shortPrincipal(p: { toText(): string } | string): string {
	const t = typeof p === 'string' ? p : p.toText();
	if (t.length <= 13) return t;
	return `${t.slice(0, 5)}…${t.slice(-3)}`;
}

export function formatChf(n: bigint | number): string {
	const v = typeof n === 'bigint' ? n : BigInt(Math.trunc(n));
	const s = v.toString();
	// Swiss number format: thousands separator is '
	return s.replace(/\B(?=(\d{3})+(?!\d))/g, '\u2019');
}

export function formatChfFromCents(cents: bigint): string {
	const negative = cents < 0n;
	const abs = negative ? -cents : cents;
	const francs = abs / 100n;
	return `${negative ? '-' : ''}${formatChf(francs)}`;
}

export function formatRelativeTime(ts_ns: bigint): string {
	const now = Date.now();
	const then = Number(ts_ns / 1_000_000n);
	const delta_ms = then - now;
	const future = delta_ms > 0;
	const abs = Math.abs(delta_ms);
	const s = Math.floor(abs / 1000);
	const m = Math.floor(s / 60);
	const h = Math.floor(m / 60);
	const d = Math.floor(h / 24);
	let label: string;
	if (d > 0) label = `${d}d`;
	else if (h > 0) label = `${h}h`;
	else if (m > 0) label = `${m}m`;
	else label = `${s}s`;
	return future ? `in ${label}` : `${label} ago`;
}

export function formatDateTime(ts_ns: bigint): string {
	const ms = Number(ts_ns / 1_000_000n);
	return new Date(ms).toLocaleString('de-CH', {
		dateStyle: 'medium',
		timeStyle: 'short'
	});
}

export function formatDate(ts_ns: bigint): string {
	const ms = Number(ts_ns / 1_000_000n);
	return new Date(ms).toLocaleDateString('de-CH', { dateStyle: 'medium' });
}

export function variantKey<T extends Record<string, unknown>>(v: T): string {
	const k = Object.keys(v)[0];
	return k ?? '';
}
