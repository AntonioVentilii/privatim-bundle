import type { AuditAction, AuditEntry } from '../declarations/audit.types';
import { variantKey } from './format';

function keyOf(o: Record<string, unknown>): string {
	return Object.keys(o)[0] ?? '';
}

function actionRepr(a: AuditAction): string {
	if ('ClientCreated' in a) return `client_created:${a.ClientCreated.client_id}`;
	if ('ClientKycUpdated' in a)
		return `client_kyc:${a.ClientKycUpdated.client_id}:${keyOf(
			a.ClientKycUpdated.status as unknown as Record<string, unknown>
		)}`;
	if ('ClientReassigned' in a)
		return `client_reassigned:${a.ClientReassigned.client_id}:${a.ClientReassigned.to.toText()}`;
	if ('MeetingAdded' in a)
		return `meeting:${a.MeetingAdded.client_id}:${a.MeetingAdded.meeting_id}`;
	if ('TradeIdeaProposed' in a)
		return `trade_idea:${a.TradeIdeaProposed.client_id}:${a.TradeIdeaProposed.trade_idea_id}`;
	if ('TradeIdeaStatusChanged' in a)
		return `trade_idea_status:${a.TradeIdeaStatusChanged.trade_idea_id}:${keyOf(
			a.TradeIdeaStatusChanged.status as unknown as Record<string, unknown>
		)}`;
	if ('RoleGranted' in a)
		return `role_granted:${a.RoleGranted.grantee.toText()}:${keyOf(
			a.RoleGranted.role as unknown as Record<string, unknown>
		)}`;
	if ('RoleRevoked' in a)
		return `role_revoked:${a.RoleRevoked.grantee.toText()}:${keyOf(
			a.RoleRevoked.role as unknown as Record<string, unknown>
		)}`;
	if ('ClientAssigned' in a)
		return `client_assigned:${a.ClientAssigned.advisor.toText()}:${a.ClientAssigned.client_id}`;
	if ('ClientUnassigned' in a)
		return `client_unassigned:${a.ClientUnassigned.advisor.toText()}:${a.ClientUnassigned.client_id}`;
	if ('AdminBootstrapped' in a) return `admin_bootstrapped:${a.AdminBootstrapped.admin.toText()}`;
	if ('AiAssistantAdmitted' in a) return `ai_admitted:${a.AiAssistantAdmitted.ai.toText()}`;
	if ('ClientAccessed' in a)
		return `client_accessed:${a.ClientAccessed.client_id}:${keyOf(
			a.ClientAccessed.purpose as unknown as Record<string, unknown>
		)}`;
	if ('AssistantQueried' in a)
		return `assistant_query:${optDebug(a.AssistantQueried.client_id)}:${
			a.AssistantQueried.intent
		}`;
	if ('AssistantResponded' in a) {
		const cites = a.AssistantResponded.citations.map((c) => c.toString()).join(',');
		return `assistant_response:${optDebug(a.AssistantResponded.client_id)}:${
			a.AssistantResponded.intent
		}:[${cites}]`;
	}
	if ('ComplianceExport' in a)
		return `compliance_export:${a.ComplianceExport.from_seq}-${a.ComplianceExport.to_seq}`;
	const _: never = a;
	return _;
}

function optDebug(o: [] | [bigint]): string {
	return o.length === 0 ? 'None' : `Some(${o[0]})`;
}

function u64BE(n: bigint): Uint8Array {
	const out = new Uint8Array(8);
	new DataView(out.buffer).setBigUint64(0, n, false);
	return out;
}

function concat(parts: Uint8Array[]): Uint8Array {
	const total = parts.reduce((s, p) => s + p.byteLength, 0);
	const out = new Uint8Array(total);
	let off = 0;
	for (const p of parts) {
		out.set(p, off);
		off += p.byteLength;
	}
	return out;
}

function hex(bytes: Uint8Array): string {
	return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

async function sha256(bytes: Uint8Array): Promise<string> {
	const buf = await crypto.subtle.digest(
		'SHA-256',
		bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer
	);
	return hex(new Uint8Array(buf));
}

const enc = new TextEncoder();

export async function recomputeHash(entry: AuditEntry): Promise<string> {
	return sha256(
		concat([
			u64BE(entry.seq),
			u64BE(entry.ts_ns),
			entry.caller.toUint8Array(),
			enc.encode(actionRepr(entry.action)),
			enc.encode(entry.prev_hash)
		])
	);
}

export interface ChainVerification {
	ok: boolean;
	firstBadSeq?: bigint;
	reason?: string;
}

export async function verifyChain(entries: AuditEntry[]): Promise<ChainVerification> {
	let prev = '';
	for (const e of entries) {
		if (e.prev_hash !== prev) {
			return { ok: false, firstBadSeq: e.seq, reason: 'prev_hash mismatch' };
		}
		const expected = await recomputeHash(e);
		if (expected !== e.hash) {
			return { ok: false, firstBadSeq: e.seq, reason: 'hash mismatch' };
		}
		prev = e.hash;
	}
	return { ok: true };
}

export function describeAction(a: AuditAction): string {
	if ('ClientCreated' in a) return `Created client #${a.ClientCreated.client_id}`;
	if ('ClientKycUpdated' in a)
		return `Updated KYC for client #${a.ClientKycUpdated.client_id} → ${variantKey(
			a.ClientKycUpdated.status as unknown as Record<string, unknown>
		)}`;
	if ('ClientReassigned' in a)
		return `Reassigned client #${a.ClientReassigned.client_id}`;
	if ('MeetingAdded' in a)
		return `Added meeting #${a.MeetingAdded.meeting_id} on client #${a.MeetingAdded.client_id}`;
	if ('TradeIdeaProposed' in a)
		return `Proposed trade idea #${a.TradeIdeaProposed.trade_idea_id} on client #${a.TradeIdeaProposed.client_id}`;
	if ('TradeIdeaStatusChanged' in a)
		return `Set trade idea #${a.TradeIdeaStatusChanged.trade_idea_id} → ${variantKey(
			a.TradeIdeaStatusChanged.status as unknown as Record<string, unknown>
		)}`;
	if ('RoleGranted' in a)
		return `Granted ${variantKey(
			a.RoleGranted.role as unknown as Record<string, unknown>
		)} to ${a.RoleGranted.grantee.toText()}`;
	if ('RoleRevoked' in a)
		return `Revoked ${variantKey(
			a.RoleRevoked.role as unknown as Record<string, unknown>
		)} from ${a.RoleRevoked.grantee.toText()}`;
	if ('ClientAssigned' in a)
		return `Assigned client #${a.ClientAssigned.client_id} to advisor`;
	if ('ClientUnassigned' in a)
		return `Unassigned client #${a.ClientUnassigned.client_id} from advisor`;
	if ('AdminBootstrapped' in a) return `Admin bootstrapped`;
	if ('AiAssistantAdmitted' in a) return `Admitted AI assistant`;
	if ('ClientAccessed' in a)
		return `Accessed client #${a.ClientAccessed.client_id} (${variantKey(
			a.ClientAccessed.purpose as unknown as Record<string, unknown>
		)})`;
	if ('AssistantQueried' in a) {
		const cid = a.AssistantQueried.client_id[0];
		return `AI queried [${a.AssistantQueried.intent}]${cid !== undefined ? ` on client #${cid}` : ''}`;
	}
	if ('AssistantResponded' in a) {
		const cid = a.AssistantResponded.client_id[0];
		return `AI responded [${a.AssistantResponded.intent}]${
			cid !== undefined ? ` on client #${cid}` : ''
		} (${a.AssistantResponded.citations.length} citations)`;
	}
	if ('ComplianceExport' in a)
		return `Exported audit slice ${a.ComplianceExport.from_seq}–${a.ComplianceExport.to_seq}`;
	const _: never = a;
	return _;
}

export function appErrorMessage(err: unknown): string {
	if (typeof err === 'object' && err !== null) {
		const e = err as Record<string, unknown>;
		if ('Unauthorized' in e) return 'Not authorised.';
		if ('NotFound' in e) return 'Not found.';
		if ('AlreadyBootstrapped' in e) return 'Already bootstrapped.';
		if ('InvalidArgument' in e) return `Invalid argument: ${String(e.InvalidArgument)}.`;
		if ('BackendUnreachable' in e) return `Backend unreachable: ${String(e.BackendUnreachable)}.`;
		if ('NotConfigured' in e)
			return `Canister not configured: ${String(e.NotConfigured)}. Run /admin/bootstrap.`;
		if ('IdentityCanisterNotConfigured' in e)
			return 'Identity canister not configured. Run /admin/bootstrap.';
		if ('AuditCanisterNotConfigured' in e)
			return 'Audit canister not configured. Run /admin/bootstrap.';
		if ('UpstreamFailed' in e) return `Upstream call failed: ${String(e.UpstreamFailed)}.`;
	}
	return err instanceof Error ? err.message : 'Unknown error';
}
