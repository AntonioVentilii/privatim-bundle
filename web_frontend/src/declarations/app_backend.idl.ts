import { IDL } from '@dfinity/candid';

export const idlFactory: IDL.InterfaceFactory = ({ IDL }) => {
	const Role = IDL.Variant({
		Advisor: IDL.Null,
		Compliance: IDL.Null,
		Admin: IDL.Null
	});

	const ClientType = IDL.Variant({
		Individual: IDL.Null,
		Family: IDL.Null,
		Corporate: IDL.Null
	});
	const KycStatus = IDL.Variant({
		Pending: IDL.Null,
		Approved: IDL.Null,
		Expired: IDL.Null
	});
	const RiskProfile = IDL.Variant({
		Conservative: IDL.Null,
		Balanced: IDL.Null,
		Growth: IDL.Null,
		Speculative: IDL.Null
	});
	const AssetClass = IDL.Variant({
		Equity: IDL.Null,
		FixedIncome: IDL.Null,
		Cash: IDL.Null,
		Fx: IDL.Null,
		Commodity: IDL.Null
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

	const Position = IDL.Record({
		ticker: IDL.Text,
		asset_class: AssetClass,
		quantity: IDL.Nat64,
		avg_cost_chf_cents: IDL.Nat64,
		current_price_chf_cents: IDL.Nat64
	});

	const Portfolio = IDL.Record({
		id: IDL.Nat64,
		client_id: IDL.Nat64,
		name: IDL.Text,
		base_currency: IDL.Text,
		positions: IDL.Vec(Position),
		cash_chf_cents: IDL.Int64,
		last_valued_at_ns: IDL.Nat64
	});

	const Client = IDL.Record({
		id: IDL.Nat64,
		display_name: IDL.Text,
		legal_name: IDL.Text,
		client_type: ClientType,
		tax_residency: IDL.Text,
		primary_advisor: IDL.Principal,
		kyc_status: KycStatus,
		kyc_expires_ns: IDL.Nat64,
		risk_profile: RiskProfile,
		aum_chf: IDL.Nat64,
		created_at_ns: IDL.Nat64,
		portfolio_ids: IDL.Vec(IDL.Nat64)
	});

	const Meeting = IDL.Record({
		id: IDL.Nat64,
		client_id: IDL.Nat64,
		advisor: IDL.Principal,
		occurred_at_ns: IDL.Nat64,
		title: IDL.Text,
		notes_md: IDL.Text,
		decisions: IDL.Vec(IDL.Text),
		follow_ups: IDL.Vec(IDL.Text)
	});

	const TradeIdea = IDL.Record({
		id: IDL.Nat64,
		client_id: IDL.Nat64,
		portfolio_id: IDL.Opt(IDL.Nat64),
		proposed_by: IDL.Principal,
		proposed_at_ns: IDL.Nat64,
		title: IDL.Text,
		rationale: IDL.Text,
		status: TradeIdeaStatus
	});

	const AuditAction = IDL.Variant({
		ClientCreated: IDL.Record({ client_id: IDL.Nat64 }),
		ClientKycUpdated: IDL.Record({ client_id: IDL.Nat64, status: KycStatus }),
		ClientReassigned: IDL.Record({ client_id: IDL.Nat64, to: IDL.Principal }),
		MeetingAdded: IDL.Record({ client_id: IDL.Nat64, meeting_id: IDL.Nat64 }),
		TradeIdeaProposed: IDL.Record({
			client_id: IDL.Nat64,
			trade_idea_id: IDL.Nat64
		}),
		TradeIdeaStatusChanged: IDL.Record({
			trade_idea_id: IDL.Nat64,
			status: TradeIdeaStatus
		}),
		RoleGranted: IDL.Record({ grantee: IDL.Principal, role: Role }),
		RoleRevoked: IDL.Record({ grantee: IDL.Principal, role: Role }),
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

	const WhoAmI = IDL.Record({
		principal: IDL.Principal,
		roles: IDL.Vec(Role)
	});

	const AppError = IDL.Variant({
		Unauthorized: IDL.Null,
		NotFound: IDL.Null,
		InvalidArgument: IDL.Text
	});

	const CreateClientArgs = IDL.Record({
		display_name: IDL.Text,
		legal_name: IDL.Text,
		client_type: ClientType,
		tax_residency: IDL.Text,
		risk_profile: RiskProfile
	});

	const AddMeetingArgs = IDL.Record({
		client_id: IDL.Nat64,
		title: IDL.Text,
		notes_md: IDL.Text,
		decisions: IDL.Vec(IDL.Text),
		follow_ups: IDL.Vec(IDL.Text)
	});

	const AddTradeIdeaArgs = IDL.Record({
		client_id: IDL.Nat64,
		portfolio_id: IDL.Opt(IDL.Nat64),
		title: IDL.Text,
		rationale: IDL.Text
	});

	const Result = IDL.Variant({ Ok: IDL.Null, Err: AppError });
	const ResultNat64 = IDL.Variant({ Ok: IDL.Nat64, Err: AppError });
	const ResultClient = IDL.Variant({ Ok: Client, Err: AppError });
	const ResultPortfolio = IDL.Variant({ Ok: Portfolio, Err: AppError });
	const ResultMeetings = IDL.Variant({ Ok: IDL.Vec(Meeting), Err: AppError });
	const ResultTradeIdeas = IDL.Variant({ Ok: IDL.Vec(TradeIdea), Err: AppError });
	const ResultExport = IDL.Variant({ Ok: ComplianceExport, Err: AppError });

	return IDL.Service({
		whoami: IDL.Func([], [WhoAmI], ['query']),
		list_clients: IDL.Func([], [IDL.Vec(Client)], ['query']),
		get_client: IDL.Func([IDL.Nat64], [ResultClient], ['query']),
		get_portfolio: IDL.Func([IDL.Nat64], [ResultPortfolio], ['query']),
		list_meetings: IDL.Func([IDL.Nat64], [ResultMeetings], ['query']),
		list_trade_ideas: IDL.Func([IDL.Nat64], [ResultTradeIdeas], ['query']),
		audit_head: IDL.Func([], [AuditHead], ['query']),
		audit_log_page: IDL.Func([IDL.Opt(IDL.Nat64), IDL.Nat64], [AuditPage], ['query']),

		record_client_access: IDL.Func([IDL.Nat64, AccessPurpose], [Result], []),
		create_client: IDL.Func([CreateClientArgs], [ResultNat64], []),
		update_kyc: IDL.Func([IDL.Nat64, KycStatus], [Result], []),
		assign_advisor: IDL.Func([IDL.Nat64, IDL.Principal], [Result], []),
		add_meeting: IDL.Func([AddMeetingArgs], [ResultNat64], []),
		add_trade_idea: IDL.Func([AddTradeIdeaArgs], [ResultNat64], []),
		set_trade_idea_status: IDL.Func([IDL.Nat64, TradeIdeaStatus], [Result], []),
		grant_role: IDL.Func([IDL.Principal, Role], [Result], []),
		revoke_role: IDL.Func([IDL.Principal, Role], [Result], []),
		signed_audit_export: IDL.Func([IDL.Nat64, IDL.Nat64], [ResultExport], []),
		record_assistant_interaction: IDL.Func(
			[IDL.Opt(IDL.Nat64), IDL.Text, IDL.Vec(IDL.Nat64), IDL.Principal],
			[Result],
			[]
		),
		admit_ai_assistant: IDL.Func([IDL.Principal], [Result], []),
		reset_demo: IDL.Func([], [ResultNat64], [])
	});
};
