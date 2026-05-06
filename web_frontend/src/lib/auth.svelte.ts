import { AuthClient } from '@dfinity/auth-client';
import type { Identity } from '@dfinity/agent';
import { Principal } from '@dfinity/principal';
import { buildBackends, type Backends } from './actor';
import { getIdentityProviderUrl } from './ii';
import type { Role } from '../declarations/identity.types';

const ONE_DAY_NS = BigInt(24) * BigInt(3_600_000_000_000);
const SESSION_NS = BigInt(30) * ONE_DAY_NS;

export interface AuthState {
	ready: boolean;
	authenticated: boolean;
	identity: Identity | null;
	principal: Principal | null;
	backends: Backends | null;
	roles: Role[];
}

function createAuth() {
	let state = $state<AuthState>({
		ready: false,
		authenticated: false,
		identity: null,
		principal: null,
		backends: null,
		roles: []
	});

	let client: AuthClient | null = null;

	async function loadRoles(b: Backends) {
		try {
			const me = await b.identity.whoami();
			state.roles = me.roles;
		} catch {
			state.roles = [];
		}
	}

	async function applyIdentity(identity: Identity) {
		const backends = await buildBackends(identity);
		state.identity = identity;
		state.principal = identity.getPrincipal();
		state.authenticated = !state.principal.isAnonymous();
		state.backends = backends;
		await loadRoles(backends);
		// First sign-in: try to claim Admin if nobody has yet. Idempotent
		// — fails silently if already bootstrapped.
		try {
			await backends.identity.bootstrap_admin();
			await loadRoles(backends);
		} catch {
			/* ignore */
		}
	}

	async function init() {
		if (typeof window === 'undefined') return;
		client = await AuthClient.create({ idleOptions: { disableIdle: true } });
		const isAuthed = await client.isAuthenticated();
		if (isAuthed) {
			await applyIdentity(client.getIdentity());
		} else {
			const backends = await buildBackends();
			state.backends = backends;
			state.identity = null;
			state.principal = null;
			state.authenticated = false;
			state.roles = [];
		}
		state.ready = true;
	}

	async function login(): Promise<void> {
		if (!client) client = await AuthClient.create({ idleOptions: { disableIdle: true } });
		const url = getIdentityProviderUrl();
		await new Promise<void>((resolve, reject) => {
			client!.login({
				identityProvider: url,
				maxTimeToLive: SESSION_NS,
				onSuccess: () => resolve(),
				onError: (err) => reject(new Error(err ?? 'login failed'))
			});
		});
		await applyIdentity(client.getIdentity());
	}

	async function logout(): Promise<void> {
		if (!client) return;
		await client.logout();
		const backends = await buildBackends();
		state.identity = null;
		state.principal = null;
		state.authenticated = false;
		state.backends = backends;
		state.roles = [];
	}

	function hasRole(r: 'Advisor' | 'Compliance' | 'Admin'): boolean {
		return state.roles.some((role) => r in role);
	}

	return {
		get state() {
			return state;
		},
		init,
		login,
		logout,
		hasRole
	};
}

export const auth = createAuth();
