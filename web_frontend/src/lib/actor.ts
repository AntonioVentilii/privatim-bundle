import { Actor, HttpAgent, type Identity } from '@dfinity/agent';
import { idlFactory as appIdl } from '../declarations/app_backend.idl';
import { idlFactory as aiIdl } from '../declarations/ai_assistant.idl';
import type { AppBackendService } from '../declarations/app_backend.types';
import type { AiAssistantService } from '../declarations/ai_assistant.types';
import { getAiAssistantId, getAppBackendId } from './ic-env';

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

export async function buildBackends(identity?: Identity): Promise<{
	app: AppBackendService;
	ai: AiAssistantService;
}> {
	const agent = await buildAgent(identity);
	return {
		app: Actor.createActor<AppBackendService>(appIdl, {
			agent,
			canisterId: getAppBackendId()
		}),
		ai: Actor.createActor<AiAssistantService>(aiIdl, {
			agent,
			canisterId: getAiAssistantId()
		})
	};
}
