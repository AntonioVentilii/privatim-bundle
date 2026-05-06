import type { ActorMethod } from '@dfinity/agent';
import type { Principal } from '@dfinity/principal';

export type Role = { Advisor: null } | { Compliance: null } | { Admin: null };

export interface WhoAmI {
	id: Principal;
	roles: Role[];
	assigned_clients: bigint[];
}

export type IdentityError = { Unauthorized: null } | { AlreadyBootstrapped: null };

export type IdentityEventKind =
	| { AdminBootstrapped: null }
	| { RoleGranted: { grantee: Principal; role: Role } }
	| { RoleRevoked: { grantee: Principal; role: Role } }
	| { ClientAssigned: { advisor: Principal; client_id: bigint } }
	| { ClientUnassigned: { advisor: Principal; client_id: bigint } }
	| { AiAssistantAdmitted: { ai: Principal } };

export interface IdentityEvent {
	ts_ns: bigint;
	by: Principal;
	kind: IdentityEventKind;
}

export type IdentityResult = { Ok: null } | { Err: IdentityError };

export interface IdentityService {
	whoami: ActorMethod<[], WhoAmI>;
	roles_of: ActorMethod<[Principal], Role[]>;
	has_role: ActorMethod<[Principal, Role], boolean>;
	is_assigned: ActorMethod<[Principal, bigint], boolean>;
	assigned_clients: ActorMethod<[Principal], bigint[]>;
	ai_assistant_principal: ActorMethod<[], [] | [Principal]>;
	admin_bootstrapped: ActorMethod<[], boolean>;
	recent_events: ActorMethod<[bigint], IdentityEvent[]>;
	bootstrap_admin: ActorMethod<[], IdentityResult>;
	grant_role: ActorMethod<[Principal, Role], IdentityResult>;
	revoke_role: ActorMethod<[Principal, Role], IdentityResult>;
	assign_client: ActorMethod<[Principal, bigint], IdentityResult>;
	unassign_client: ActorMethod<[Principal, bigint], IdentityResult>;
	admit_ai_assistant: ActorMethod<[Principal], IdentityResult>;
}
