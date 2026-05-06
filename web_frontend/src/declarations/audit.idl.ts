import { IDL } from '@dfinity/candid';

export const idlFactory: IDL.InterfaceFactory = ({ IDL }) => {
	const Role = IDL.Variant({
		Advisor: IDL.Null,
		Compliance: IDL.Null,
		Admin: IDL.Null
	});
	const KycStatus = IDL.Variant({
		Pending: IDL.Null,
		Approved: IDL.Null,
		Expired: IDL.Null
	});
	const TradeIdeaStatus = IDL.Variant({
		Draft: IDL.Null,
		Approved: IDL.Null,
		Rejected: IDL.Null,
		Executed: IDL.Null
	});
	const AccessPurpose = IDL.Variant({
		ManualReview: IDL.Null,
		TradeIdeaPreparation: IDL.Null,
		AssistantQuery: IDL.Null,
		ComplianceReview: IDL.Null,
		KycRefresh: IDL.Null
	});

	const AuditAction = IDL.Variant({
		ClientCreated: IDL.Record({ client_id: IDL.Nat64 }),
		ClientKycUpdated: IDL.Record({ client_id: IDL.Nat64, status: KycStatus }),
		ClientReassigned: IDL.Record({ client_id: IDL.Nat64, to: IDL.Principal }),
		MeetingAdded: IDL.Record({ client_id: IDL.Nat64, meeting_id: IDL.Nat64 }),
		TradeIdeaProposed: IDL.Record({ client_id: IDL.Nat64, trade_idea_id: IDL.Nat64 }),
		TradeIdeaStatusChanged: IDL.Record({
			trade_idea_id: IDL.Nat64,
			status: TradeIdeaStatus
		}),
		RoleGranted: IDL.Record({ grantee: IDL.Principal, role: Role }),
		RoleRevoked: IDL.Record({ grantee: IDL.Principal, role: Role }),
		ClientAssigned: IDL.Record({ advisor: IDL.Principal, client_id: IDL.Nat64 }),
		ClientUnassigned: IDL.Record({ advisor: IDL.Principal, client_id: IDL.Nat64 }),
		AdminBootstrapped: IDL.Record({ admin: IDL.Principal }),
		AiAssistantAdmitted: IDL.Record({ ai: IDL.Principal }),
		ClientAccessed: IDL.Record({ client_id: IDL.Nat64, purpose: AccessPurpose }),
		AssistantQueried: IDL.Record({
			client_id: IDL.Opt(IDL.Nat64),
			intent: IDL.Text
		}),
		AssistantResponded: IDL.Record({
			client_id: IDL.Opt(IDL.Nat64),
			intent: IDL.Text,
			citations: IDL.Vec(IDL.Nat64)
		}),
		ComplianceExport: IDL.Record({ from_seq: IDL.Nat64, to_seq: IDL.Nat64 })
	});

	const AuditEntry = IDL.Record({
		seq: IDL.Nat64,
		prev_hash: IDL.Text,
		hash: IDL.Text,
		ts_ns: IDL.Nat64,
		caller: IDL.Principal,
		action: AuditAction
	});

	const AuditPage = IDL.Record({
		entries: IDL.Vec(AuditEntry),
		next_cursor: IDL.Opt(IDL.Nat64),
		total: IDL.Nat64
	});

	const AuditHead = IDL.Record({ seq: IDL.Nat64, hash: IDL.Text });

	const ComplianceExport = IDL.Record({
		exported_at_ns: IDL.Nat64,
		exporter: IDL.Principal,
		from_seq: IDL.Nat64,
		to_seq: IDL.Nat64,
		head_hash: IDL.Text,
		entries: IDL.Vec(AuditEntry)
	});

	const AuditError = IDL.Variant({
		Unauthorized: IDL.Null,
		InvalidArgument: IDL.Text,
		IdentityCanisterNotConfigured: IDL.Null
	});

	const Result = IDL.Variant({ Ok: IDL.Null, Err: AuditError });
	const ResultNat64 = IDL.Variant({ Ok: IDL.Nat64, Err: AuditError });
	const ResultExport = IDL.Variant({ Ok: ComplianceExport, Err: AuditError });

	return IDL.Service({
		audit_head: IDL.Func([], [AuditHead], ['query']),
		total_entries: IDL.Func([], [IDL.Nat64], ['query']),
		audit_log_page: IDL.Func(
			[IDL.Opt(IDL.Nat64), IDL.Nat64],
			[AuditPage],
			['composite_query']
		),
		writers: IDL.Func([], [IDL.Vec(IDL.Principal)], ['query']),
		identity_canister: IDL.Func([], [IDL.Opt(IDL.Principal)], ['query']),
		admit_writer: IDL.Func([IDL.Principal], [Result], []),
		revoke_writer: IDL.Func([IDL.Principal], [Result], []),
		set_identity_canister: IDL.Func([IDL.Principal], [Result], []),
		record: IDL.Func([AuditAction, IDL.Principal], [ResultNat64], []),
		signed_audit_export: IDL.Func([IDL.Nat64, IDL.Nat64], [ResultExport], []),
		reset_demo: IDL.Func([], [ResultNat64], [])
	});
};
