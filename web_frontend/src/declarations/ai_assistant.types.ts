import type { ActorMethod } from '@dfinity/agent';
import type { Principal } from '@dfinity/principal';

export type AssistantIntent =
	| { PortfolioOverview: null }
	| { RiskAssessment: null }
	| { KycStatus: null }
	| { MeetingDigest: null }
	| { OpenTradeIdeas: null }
	| { FxExposureBook: null }
	| { KycActionList: null };

export interface AssistantRequest {
	intent: AssistantIntent;
	client_id: [] | [bigint];
	raw_prompt: string;
}

export type CitationKind =
	| { Client: null }
	| { Portfolio: null }
	| { Meeting: null }
	| { TradeIdea: null };

export interface AssistantCitation {
	kind: CitationKind;
	id: bigint;
	label: string;
}

export interface AssistantResponse {
	answer: string;
	citations: AssistantCitation[];
	audit_seq: bigint;
	model: string;
	inference_ms: bigint;
}

export type AssistantError =
	| { Unauthorized: null }
	| { NotFound: null }
	| { BackendUnreachable: string }
	| { NotConfigured: string };

export type ResultUnit = { Ok: null } | { Err: AssistantError };
export type ResultResponse = { Ok: AssistantResponse } | { Err: AssistantError };

export interface AiAssistantService {
	whoami: ActorMethod<[], Principal>;
	config: ActorMethod<[], [[] | [Principal], [] | [Principal]]>;
	llm_config: ActorMethod<[], [] | [string]>;
	set_data_canister: ActorMethod<[Principal], ResultUnit>;
	set_audit_canister: ActorMethod<[Principal], ResultUnit>;
	set_llm_base_url: ActorMethod<[string], ResultUnit>;
	ask: ActorMethod<[AssistantRequest], ResultResponse>;
}
