import { Actor, HttpAgent, type Identity } from '@dfinity/agent';
import { idlFactory as identityIdl } from '../declarations/identity.idl';
import { idlFactory as auditIdl } from '../declarations/audit.idl';
import { idlFactory as dataIdl } from '../declarations/data.idl';
import { idlFactory as aiIdl } from '../declarations/ai_assistant.idl';
import type { IdentityService } from '../declarations/identity.types';
import type { AuditService } from '../declarations/audit.types';
import type { DataService } from '../declarations/data.types';
import type { AiAssistantService } from '../declarations/ai_assistant.types';
import {
	getAiAssistantId,
	getAuditId,
	getDataId,
	getIdentityId
} from './ic-env';

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
	ai: AiAssistantService;
}

export async function buildBackends(identity?: Identity): Promise<Backends> {
	const agent = await buildAgent(identity);
	return {
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
		ai: Actor.createActor<AiAssistantService>(aiIdl, {
			agent,
			canisterId: getAiAssistantId()
		})
	};
}
