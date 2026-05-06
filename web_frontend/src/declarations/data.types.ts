import type { ActorMethod } from '@dfinity/agent';
import type { Principal } from '@dfinity/principal';

export type ClientType = { Individual: null } | { Family: null } | { Corporate: null };
export type KycStatus = { Pending: null } | { Approved: null } | { Expired: null };
export type RiskProfile =
	| { Conservative: null }
	| { Balanced: null }
	| { Growth: null }
	| { Speculative: null };
export type AssetClass =
	| { Equity: null }
	| { FixedIncome: null }
	| { Cash: null }
	| { Fx: null }
	| { Commodity: null };
export type TradeIdeaStatus =
	| { Draft: null }
	| { Approved: null }
	| { Rejected: null }
	| { Executed: null };
export type AccessPurpose =
	| { ManualReview: null }
	| { TradeIdeaPreparation: null }
	| { AssistantQuery: null }
	| { ComplianceReview: null }
	| { KycRefresh: null };

export interface Position {
	ticker: string;
	asset_class: AssetClass;
	quantity: bigint;
	avg_cost_chf_cents: bigint;
	current_price_chf_cents: bigint;
}
export interface Portfolio {
	id: bigint;
	client_id: bigint;
	name: string;
	base_currency: string;
	positions: Position[];
	cash_chf_cents: bigint;
	last_valued_at_ns: bigint;
}
export interface Client {
	id: bigint;
	display_name: string;
	legal_name: string;
	client_type: ClientType;
	tax_residency: string;
	primary_advisor: Principal;
	kyc_status: KycStatus;
	kyc_expires_ns: bigint;
	risk_profile: RiskProfile;
	aum_chf: bigint;
	created_at_ns: bigint;
	portfolio_ids: bigint[];
}
export interface Meeting {
	id: bigint;
	client_id: bigint;
	advisor: Principal;
	occurred_at_ns: bigint;
	title: string;
	notes_md: string;
	decisions: string[];
	follow_ups: string[];
}
export interface TradeIdea {
	id: bigint;
	client_id: bigint;
	portfolio_id: [] | [bigint];
	proposed_by: Principal;
	proposed_at_ns: bigint;
	title: string;
	rationale: string;
	status: TradeIdeaStatus;
}

export type DataError =
	| { Unauthorized: null }
	| { NotFound: null }
	| { InvalidArgument: string }
	| { IdentityCanisterNotConfigured: null }
	| { AuditCanisterNotConfigured: null }
	| { UpstreamFailed: string };

export type DataResult = { Ok: null } | { Err: DataError };
export type DataResultNat64 = { Ok: bigint } | { Err: DataError };
export type DataResultClient = { Ok: Client } | { Err: DataError };
export type DataResultPortfolio = { Ok: Portfolio } | { Err: DataError };
export type DataResultMeetings = { Ok: Meeting[] } | { Err: DataError };
export type DataResultTradeIdeas = { Ok: TradeIdea[] } | { Err: DataError };

export interface CreateClientArgs {
	display_name: string;
	legal_name: string;
	client_type: ClientType;
	tax_residency: string;
	risk_profile: RiskProfile;
}
export interface AddMeetingArgs {
	client_id: bigint;
	title: string;
	notes_md: string;
	decisions: string[];
	follow_ups: string[];
}
export interface AddTradeIdeaArgs {
	client_id: bigint;
	portfolio_id: [] | [bigint];
	title: string;
	rationale: string;
}

export interface DataService {
	list_clients: ActorMethod<[], Client[]>;
	get_client: ActorMethod<[bigint], DataResultClient>;
	get_portfolio: ActorMethod<[bigint], DataResultPortfolio>;
	list_meetings: ActorMethod<[bigint], DataResultMeetings>;
	list_trade_ideas: ActorMethod<[bigint], DataResultTradeIdeas>;
	config: ActorMethod<[], [[] | [Principal], [] | [Principal]]>;
	set_identity_canister: ActorMethod<[Principal], DataResult>;
	set_audit_canister: ActorMethod<[Principal], DataResult>;
	record_client_access: ActorMethod<[bigint, AccessPurpose], DataResult>;
	create_client: ActorMethod<[CreateClientArgs], DataResultNat64>;
	update_kyc: ActorMethod<[bigint, KycStatus], DataResult>;
	assign_advisor: ActorMethod<[bigint, Principal], DataResult>;
	add_meeting: ActorMethod<[AddMeetingArgs], DataResultNat64>;
	add_trade_idea: ActorMethod<[AddTradeIdeaArgs], DataResultNat64>;
	set_trade_idea_status: ActorMethod<[bigint, TradeIdeaStatus], DataResult>;
	reset_demo: ActorMethod<[], DataResultNat64>;
}
