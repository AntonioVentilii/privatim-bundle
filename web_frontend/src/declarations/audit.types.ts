import type { ActorMethod } from '@dfinity/agent';
import type { Principal } from '@dfinity/principal';

export type Role = { Advisor: null } | { Compliance: null } | { Admin: null };
export type KycStatus = { Pending: null } | { Approved: null } | { Expired: null };
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

export type AuditAction =
	| { ClientCreated: { client_id: bigint } }
	| { ClientKycUpdated: { client_id: bigint; status: KycStatus } }
	| { ClientReassigned: { client_id: bigint; to: Principal } }
	| { MeetingAdded: { client_id: bigint; meeting_id: bigint } }
	| { TradeIdeaProposed: { client_id: bigint; trade_idea_id: bigint } }
	| { TradeIdeaStatusChanged: { trade_idea_id: bigint; status: TradeIdeaStatus } }
	| { RoleGranted: { grantee: Principal; role: Role } }
	| { RoleRevoked: { grantee: Principal; role: Role } }
	| { ClientAssigned: { advisor: Principal; client_id: bigint } }
	| { ClientUnassigned: { advisor: Principal; client_id: bigint } }
	| { AdminBootstrapped: { admin: Principal } }
	| { AiAssistantAdmitted: { ai: Principal } }
	| { ClientAccessed: { client_id: bigint; purpose: AccessPurpose } }
	| { AssistantQueried: { client_id: [] | [bigint]; intent: string } }
	| {
			AssistantResponded: {
				client_id: [] | [bigint];
				intent: string;
				citations: bigint[];
			};
	  }
	| { ComplianceExport: { from_seq: bigint; to_seq: bigint } }
	| {
			DocumentUploaded: {
				client_id: bigint;
				doc_id: bigint;
				plaintext_sha256: string;
			};
	  }
	| { DocumentAccessed: { client_id: bigint; doc_id: bigint } }
	| { DocumentDeleted: { client_id: bigint; doc_id: bigint } };

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

export type AuditError =
	| { Unauthorized: null }
	| { InvalidArgument: string }
	| { IdentityCanisterNotConfigured: null };

export type AuditResult = { Ok: null } | { Err: AuditError };
export type AuditResultNat64 = { Ok: bigint } | { Err: AuditError };
export type AuditResultExport = { Ok: ComplianceExport } | { Err: AuditError };

export interface AuditService {
	audit_head: ActorMethod<[], AuditHead>;
	total_entries: ActorMethod<[], bigint>;
	audit_log_page: ActorMethod<[[] | [bigint], bigint], AuditPage>;
	identity_canister: ActorMethod<[], [] | [Principal]>;
	set_identity_canister: ActorMethod<[Principal], AuditResult>;
	append: ActorMethod<[AuditAction, Principal], AuditResultNat64>;
	signed_audit_export: ActorMethod<[bigint, bigint], AuditResultExport>;
	reset_demo: ActorMethod<[], AuditResultNat64>;
}
