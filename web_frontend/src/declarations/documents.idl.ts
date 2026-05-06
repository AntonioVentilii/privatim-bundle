import { IDL } from '@dfinity/candid';

export const idlFactory: IDL.InterfaceFactory = ({ IDL }) => {
	const Document = IDL.Record({
		id: IDL.Nat64,
		client_id: IDL.Nat64,
		label: IDL.Text,
		iv: IDL.Vec(IDL.Nat8),
		ciphertext: IDL.Vec(IDL.Nat8),
		plaintext_sha256: IDL.Text,
		uploaded_by: IDL.Principal,
		uploaded_at_ns: IDL.Nat64,
		size_bytes: IDL.Nat64
	});

	const DocumentMeta = IDL.Record({
		id: IDL.Nat64,
		client_id: IDL.Nat64,
		label: IDL.Text,
		plaintext_sha256: IDL.Text,
		uploaded_by: IDL.Principal,
		uploaded_at_ns: IDL.Nat64,
		size_bytes: IDL.Nat64
	});

	const UploadArgs = IDL.Record({
		client_id: IDL.Nat64,
		label: IDL.Text,
		iv: IDL.Vec(IDL.Nat8),
		ciphertext: IDL.Vec(IDL.Nat8),
		plaintext_sha256: IDL.Text
	});

	const DocumentsError = IDL.Variant({
		Unauthorized: IDL.Null,
		NotFound: IDL.Null,
		InvalidArgument: IDL.Text,
		AuditCanisterNotConfigured: IDL.Null,
		UpstreamFailed: IDL.Text
	});

	const Result = IDL.Variant({ Ok: IDL.Null, Err: DocumentsError });
	const ResultNat64 = IDL.Variant({ Ok: IDL.Nat64, Err: DocumentsError });
	const ResultDoc = IDL.Variant({ Ok: Document, Err: DocumentsError });
	const ResultMeta = IDL.Variant({ Ok: DocumentMeta, Err: DocumentsError });
	const ResultMetas = IDL.Variant({ Ok: IDL.Vec(DocumentMeta), Err: DocumentsError });

	return IDL.Service({
		list_documents: IDL.Func([IDL.Nat64], [ResultMetas], ['query']),
		get_document_meta: IDL.Func([IDL.Nat64], [ResultMeta], ['query']),
		config: IDL.Func([], [IDL.Opt(IDL.Principal)], ['query']),
		set_audit_canister: IDL.Func([IDL.Principal], [Result], []),
		fetch_document: IDL.Func([IDL.Nat64], [ResultDoc], []),
		upload_document: IDL.Func([UploadArgs], [ResultNat64], []),
		delete_document: IDL.Func([IDL.Nat64], [Result], []),
		reset_demo: IDL.Func([], [ResultNat64], [])
	});
};
