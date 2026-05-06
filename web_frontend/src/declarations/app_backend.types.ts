import type { ActorMethod } from '@dfinity/agent';
import type { Principal } from '@dfinity/principal';

export type Role = { Advisor: null } | { Compliance: null } | { Admin: null };
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

export type AuditAction =
	| { ClientCreated: { client_id: bigint } }
	| { ClientKycUpdated: { client_id: bigint; status: KycStatus } }
	| { ClientReassigned: { client_id: bigint; to: Principal } }
	| { MeetingAdded: { client_id: bigint; meeting_id: bigint } }
	| { TradeIdeaProposed: { client_id: bigint; trade_idea_id: bigint } }
	| { TradeIdeaStatusChanged: { trade_idea_id: bigint; status: TradeIdeaStatus } }
	| { RoleGranted: { grantee: Principal; role: Role } }
	| { RoleRevoked: { grantee: Principal; role: Role } }
	| { ClientAccessed: { client_id: bigint; purpose: AccessPurpose } }
	| { AssistantQueried: { client_id: [] | [bigint]; intent: string } }
	| {
			AssistantResponded: {
				client_id: [] | [bigint];
				intent: string;
				citations: bigint[];
			};
	  }
	| { ComplianceExport: { from_seq: bigint; to_seq: bigint } };

export interface AuditEntry {
	seq: bigint;
	prev_hash: string;
	hash: string;
	ts_ns: bigint;
	caller: Principal;
	action: AuditAction;
}

export interface AuditPage {
	entries: AuditEntry[];
	next_cursor: [] | [bigint];
	total: bigint;
}

export interface AuditHead {
	seq: bigint;
	hash: string;
}

export interface ComplianceExport {
	exported_at_ns: bigint;
	exporter: Principal;
	from_seq: bigint;
	to_seq: bigint;
	head_hash: string;
	entries: AuditEntry[];
}

export interface WhoAmI {
	principal: Principal;
	roles: Role[];
}

export type AppError =
	| { Unauthorized: null }
	| { NotFound: null }
	| { InvalidArgument: string };

export type Result = { Ok: null } | { Err: AppError };
export type ResultNat64 = { Ok: bigint } | { Err: AppError };
export type ResultClient = { Ok: Client } | { Err: AppError };
export type ResultPortfolio = { Ok: Portfolio } | { Err: AppError };
export type ResultMeetings = { Ok: Meeting[] } | { Err: AppError };
export type ResultTradeIdeas = { Ok: TradeIdea[] } | { Err: AppError };
export type ResultExport = { Ok: ComplianceExport } | { Err: AppError };

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

export interface AppBackendService {
	whoami: ActorMethod<[], WhoAmI>;
	list_clients: ActorMethod<[], Client[]>;
	get_client: ActorMethod<[bigint], ResultClient>;
	get_portfolio: ActorMethod<[bigint], ResultPortfolio>;
	list_meetings: ActorMethod<[bigint], ResultMeetings>;
	list_trade_ideas: ActorMethod<[bigint], ResultTradeIdeas>;
	audit_head: ActorMethod<[], AuditHead>;
	audit_log_page: ActorMethod<[[] | [bigint], bigint], AuditPage>;

	record_client_access: ActorMethod<[bigint, AccessPurpose], Result>;
	create_client: ActorMethod<[CreateClientArgs], ResultNat64>;
	update_kyc: ActorMethod<[bigint, KycStatus], Result>;
	assign_advisor: ActorMethod<[bigint, Principal], Result>;
	add_meeting: ActorMethod<[AddMeetingArgs], ResultNat64>;
	add_trade_idea: ActorMethod<[AddTradeIdeaArgs], ResultNat64>;
	set_trade_idea_status: ActorMethod<[bigint, TradeIdeaStatus], Result>;
	grant_role: ActorMethod<[Principal, Role], Result>;
	revoke_role: ActorMethod<[Principal, Role], Result>;
	signed_audit_export: ActorMethod<[bigint, bigint], ResultExport>;
	record_assistant_interaction: ActorMethod<
		[[] | [bigint], string, bigint[], Principal],
		Result
	>;
	admit_ai_assistant: ActorMethod<[Principal], Result>;
	reset_demo: ActorMethod<[], ResultNat64>;
}
