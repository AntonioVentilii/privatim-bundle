import type { ActorMethod } from '@dfinity/agent';
import type { Principal } from '@dfinity/principal';

export interface Document {
	id: bigint;
	client_id: bigint;
	label: string;
	iv: Uint8Array | number[];
	ciphertext: Uint8Array | number[];
	plaintext_sha256: string;
	uploaded_by: Principal;
	uploaded_at_ns: bigint;
	size_bytes: bigint;
}

export interface DocumentMeta {
	id: bigint;
	client_id: bigint;
	label: string;
	plaintext_sha256: string;
	uploaded_by: Principal;
	uploaded_at_ns: bigint;
	size_bytes: bigint;
}

export interface UploadArgs {
	client_id: bigint;
	label: string;
	iv: Uint8Array | number[];
	ciphertext: Uint8Array | number[];
	plaintext_sha256: string;
}

export type DocumentsError =
	| { Unauthorized: null }
	| { NotFound: null }
	| { InvalidArgument: string }
	| { AuditCanisterNotConfigured: null }
	| { UpstreamFailed: string };

export type DocumentsResult = { Ok: null } | { Err: DocumentsError };
export type DocumentsResultNat64 = { Ok: bigint } | { Err: DocumentsError };
export type DocumentsResultDoc = { Ok: Document } | { Err: DocumentsError };
export type DocumentsResultMeta = { Ok: DocumentMeta } | { Err: DocumentsError };
export type DocumentsResultMetas = { Ok: DocumentMeta[] } | { Err: DocumentsError };

export interface DocumentsService {
	list_documents: ActorMethod<[bigint], DocumentsResultMetas>;
	get_document_meta: ActorMethod<[bigint], DocumentsResultMeta>;
	config: ActorMethod<[], [] | [Principal]>;
	set_audit_canister: ActorMethod<[Principal], DocumentsResult>;
	fetch_document: ActorMethod<[bigint], DocumentsResultDoc>;
	upload_document: ActorMethod<[UploadArgs], DocumentsResultNat64>;
	delete_document: ActorMethod<[bigint], DocumentsResult>;
	reset_demo: ActorMethod<[], DocumentsResultNat64>;
}
