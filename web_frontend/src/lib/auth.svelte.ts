import { AuthClient } from '@dfinity/auth-client';
import type { Identity } from '@dfinity/agent';
import { Principal } from '@dfinity/principal';
import { buildBackends } from './actor';
import { getIdentityProviderUrl } from './ii';
import type { AppBackendService, Role } from '../declarations/app_backend.types';
import type { AiAssistantService } from '../declarations/ai_assistant.types';

const ONE_DAY_NS = BigInt(24) * BigInt(3_600_000_000_000);
const SESSION_NS = BigInt(30) * ONE_DAY_NS;

export interface AuthState {
	ready: boolean;
	authenticated: boolean;
	identity: Identity | null;
	principal: Principal | null;
	app: AppBackendService | null;
	ai: AiAssistantService | null;
	roles: Role[];
}

function createAuth() {
	let state = $state<AuthState>({
		ready: false,
		authenticated: false,
		identity: null,
		principal: null,
		app: null,
		ai: null,
		roles: []
	});

	let client: AuthClient | null = null;

	async function loadRoles(app: AppBackendService) {
		try {
			const me = await app.whoami();
			state.roles = me.roles;
		} catch {
			state.roles = [];
		}
	}

	async function applyIdentity(identity: Identity) {
		const { app, ai } = await buildBackends(identity);
		state.identity = identity;
		state.principal = identity.getPrincipal();
		state.authenticated = !state.principal.isAnonymous();
		state.app = app;
		state.ai = ai;
		await loadRoles(app);
	}

	async function init() {
		if (typeof window === 'undefined') return;
		client = await AuthClient.create({ idleOptions: { disableIdle: true } });
		const isAuthed = await client.isAuthenticated();
		if (isAuthed) {
			await applyIdentity(client.getIdentity());
		} else {
			const { app, ai } = await buildBackends();
			state.app = app;
			state.ai = ai;
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
		const { app, ai } = await buildBackends();
		state.identity = null;
		state.principal = null;
		state.authenticated = false;
		state.app = app;
		state.ai = ai;
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
