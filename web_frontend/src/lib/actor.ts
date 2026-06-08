import { Actor, HttpAgent, type Identity } from '@dfinity/agent';
import { idlFactory as identityIdl } from '../declarations/identity.idl';
import { idlFactory as auditIdl } from '../declarations/audit.idl';
import { idlFactory as dataIdl } from '../declarations/data.idl';
import { idlFactory as documentsIdl } from '../declarations/documents.idl';
import { idlFactory as aiIdl } from '../declarations/ai_assistant.idl';
import type { IdentityService } from '../declarations/identity.types';
import type { AuditService } from '../declarations/audit.types';
import type { DataService } from '../declarations/data.types';
import type { DocumentsService } from '../declarations/documents.types';
import type { AiAssistantService } from '../declarations/ai_assistant.types';
import {
	getAiAssistantId,
	getAuditId,
	getDataId,
	getDocumentsId,
	getIdentityId
} from './ic-env';
import { AI_ENABLED } from './features';

async function buildAgent(identity?: Identity): Promise<HttpAgent> {
	const agent = await HttpAgent.create({
		host: window.location.origin,
		identity,
		shouldFetchRootKey: true,
		verifyQuerySignatures: false
	});
	await agent.fetchRootKey();
	return agent;
}

export interface Backends {
	identity: IdentityService;
	audit: AuditService;
	data: DataService;
	documents: DocumentsService;
	// Only wired when AI is enabled (see lib/features.ts). The ai_assistant
	// canister isn't shipped as a node while AI is disabled, so consumers must
	// guard on `backends.ai` before calling it.
	ai?: AiAssistantService;
}

export async function buildBackends(identity?: Identity): Promise<Backends> {
	const agent = await buildAgent(identity);
	const backends: Backends = {
		identity: Actor.createActor<IdentityService>(identityIdl, {
			agent,
			canisterId: getIdentityId()
		}),
		audit: Actor.createActor<AuditService>(auditIdl, {
			agent,
			canisterId: getAuditId()
		}),
		data: Actor.createActor<DataService>(dataIdl, {
			agent,
			canisterId: getDataId()
		}),
		documents: Actor.createActor<DocumentsService>(documentsIdl, {
			agent,
			canisterId: getDocumentsId()
		})
	};
	if (AI_ENABLED) {
		backends.ai = Actor.createActor<AiAssistantService>(aiIdl, {
			agent,
			canisterId: getAiAssistantId()
		});
	}
	return backends;
}
