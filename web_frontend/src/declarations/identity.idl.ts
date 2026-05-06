import { IDL } from '@dfinity/candid';

export const idlFactory: IDL.InterfaceFactory = ({ IDL }) => {
	const Role = IDL.Variant({
		Advisor: IDL.Null,
		Compliance: IDL.Null,
		Admin: IDL.Null
	});

	const WhoAmI = IDL.Record({
		id: IDL.Principal,
		roles: IDL.Vec(Role),
		assigned_clients: IDL.Vec(IDL.Nat64)
	});

	const IdentityError = IDL.Variant({
		Unauthorized: IDL.Null,
		AlreadyBootstrapped: IDL.Null
	});

	const IdentityEventKind = IDL.Variant({
		AdminBootstrapped: IDL.Null,
		RoleGranted: IDL.Record({ grantee: IDL.Principal, role: Role }),
		RoleRevoked: IDL.Record({ grantee: IDL.Principal, role: Role }),
		ClientAssigned: IDL.Record({ advisor: IDL.Principal, client_id: IDL.Nat64 }),
		ClientUnassigned: IDL.Record({ advisor: IDL.Principal, client_id: IDL.Nat64 }),
		AiAssistantAdmitted: IDL.Record({ ai: IDL.Principal })
	});

	const IdentityEvent = IDL.Record({
		ts_ns: IDL.Nat64,
		by: IDL.Principal,
		kind: IdentityEventKind
	});

	const Result = IDL.Variant({ Ok: IDL.Null, Err: IdentityError });

	return IDL.Service({
		whoami: IDL.Func([], [WhoAmI], ['query']),
		roles_of: IDL.Func([IDL.Principal], [IDL.Vec(Role)], ['query']),
		has_role: IDL.Func([IDL.Principal, Role], [IDL.Bool], ['query']),
		is_assigned: IDL.Func([IDL.Principal, IDL.Nat64], [IDL.Bool], ['query']),
		assigned_clients: IDL.Func([IDL.Principal], [IDL.Vec(IDL.Nat64)], ['query']),
		ai_assistant_principal: IDL.Func([], [IDL.Opt(IDL.Principal)], ['query']),
		admin_bootstrapped: IDL.Func([], [IDL.Bool], ['query']),
		recent_events: IDL.Func([IDL.Nat64], [IDL.Vec(IdentityEvent)], ['query']),
		bootstrap_admin: IDL.Func([], [Result], []),
		grant_role: IDL.Func([IDL.Principal, Role], [Result], []),
		revoke_role: IDL.Func([IDL.Principal, Role], [Result], []),
		assign_client: IDL.Func([IDL.Principal, IDL.Nat64], [Result], []),
		unassign_client: IDL.Func([IDL.Principal, IDL.Nat64], [Result], []),
		admit_ai_assistant: IDL.Func([IDL.Principal], [Result], [])
	});
};
