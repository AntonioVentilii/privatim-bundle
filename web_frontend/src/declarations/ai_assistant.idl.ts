import { IDL } from '@dfinity/candid';

export const idlFactory: IDL.InterfaceFactory = ({ IDL }) => {
	const AssistantIntent = IDL.Variant({
		PortfolioOverview: IDL.Null,
		RiskAssessment: IDL.Null,
		KycStatus: IDL.Null,
		MeetingDigest: IDL.Null,
		OpenTradeIdeas: IDL.Null,
		FxExposureBook: IDL.Null,
		KycActionList: IDL.Null
	});

	const AssistantRequest = IDL.Record({
		intent: AssistantIntent,
		client_id: IDL.Opt(IDL.Nat64),
		raw_prompt: IDL.Text
	});

	const CitationKind = IDL.Variant({
		Client: IDL.Null,
		Portfolio: IDL.Null,
		Meeting: IDL.Null,
		TradeIdea: IDL.Null
	});

	const AssistantCitation = IDL.Record({
		kind: CitationKind,
		id: IDL.Nat64,
		label: IDL.Text
	});

	const AssistantResponse = IDL.Record({
		answer: IDL.Text,
		citations: IDL.Vec(AssistantCitation),
		audit_seq: IDL.Nat64,
		model: IDL.Text,
		inference_ms: IDL.Nat64
	});

	const AssistantError = IDL.Variant({
		Unauthorized: IDL.Null,
		NotFound: IDL.Null,
		BackendUnreachable: IDL.Text,
		NotConfigured: IDL.Text
	});

	const ResultUnit = IDL.Variant({ Ok: IDL.Null, Err: AssistantError });
	const ResultResponse = IDL.Variant({ Ok: AssistantResponse, Err: AssistantError });

	return IDL.Service({
		whoami: IDL.Func([], [IDL.Principal], ['query']),
		config: IDL.Func([], [IDL.Opt(IDL.Principal), IDL.Opt(IDL.Principal)], ['query']),
		set_data_canister: IDL.Func([IDL.Principal], [ResultUnit], []),
		set_audit_canister: IDL.Func([IDL.Principal], [ResultUnit], []),
		ask: IDL.Func([AssistantRequest], [ResultResponse], [])
	});
};
