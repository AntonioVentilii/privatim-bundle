// Internet Identity is always resolved to the hosted id.ai, in both local dev
// and production. Since icp-cli 0.2.4 the local replica trusts mainnet subnet
// signatures, so a delegation issued by the real id.ai is accepted locally too
// — no local II canister needed. @dfinity/auth-client appends `#authorize`.
//
// The previous approach derived a per-gateway II URL by swapping the canister-id
// subdomain of window.location for an II canister id. On any gateway that wasn't
// icp0.io/ic0.app it produced a canister-origin II URL built from a *testnet* II
// canister, where users' id.ai-scoped passkeys don't resolve. Hardcoding id.ai
// removes that class of bug entirely.
export function getIdentityProviderUrl(): string {
	return 'https://id.ai';
}
