import { IDL } from '@dfinity/candid';

export const idlFactory: IDL.InterfaceFactory = ({ IDL }) => {
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

	const DataError = IDL.Variant({
		Unauthorized: IDL.Null,
		NotFound: IDL.Null,
		InvalidArgument: IDL.Text,
		IdentityCanisterNotConfigured: IDL.Null,
		AuditCanisterNotConfigured: IDL.Null,
		UpstreamFailed: IDL.Text
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

	const Result = IDL.Variant({ Ok: IDL.Null, Err: DataError });
	const ResultNat64 = IDL.Variant({ Ok: IDL.Nat64, Err: DataError });
	const ResultClient = IDL.Variant({ Ok: Client, Err: DataError });
	const ResultPortfolio = IDL.Variant({ Ok: Portfolio, Err: DataError });
	const ResultMeetings = IDL.Variant({ Ok: IDL.Vec(Meeting), Err: DataError });
	const ResultTradeIdeas = IDL.Variant({ Ok: IDL.Vec(TradeIdea), Err: DataError });

	return IDL.Service({
		list_clients: IDL.Func([], [IDL.Vec(Client)], ['query']),
		get_client: IDL.Func([IDL.Nat64], [ResultClient], ['query']),
		get_portfolio: IDL.Func([IDL.Nat64], [ResultPortfolio], ['query']),
		list_meetings: IDL.Func([IDL.Nat64], [ResultMeetings], ['query']),
		list_trade_ideas: IDL.Func([IDL.Nat64], [ResultTradeIdeas], ['query']),
		config: IDL.Func(
			[],
			[IDL.Opt(IDL.Principal), IDL.Opt(IDL.Principal)],
			['query']
		),
		set_identity_canister: IDL.Func([IDL.Principal], [Result], []),
		set_audit_canister: IDL.Func([IDL.Principal], [Result], []),
		record_client_access: IDL.Func([IDL.Nat64, AccessPurpose], [Result], []),
		create_client: IDL.Func([CreateClientArgs], [ResultNat64], []),
		update_kyc: IDL.Func([IDL.Nat64, KycStatus], [Result], []),
		assign_advisor: IDL.Func([IDL.Nat64, IDL.Principal], [Result], []),
		add_meeting: IDL.Func([AddMeetingArgs], [ResultNat64], []),
		add_trade_idea: IDL.Func([AddTradeIdeaArgs], [ResultNat64], []),
		set_trade_idea_status: IDL.Func([IDL.Nat64, TradeIdeaStatus], [Result], []),
		reset_demo: IDL.Func([], [ResultNat64], [])
	});
};
