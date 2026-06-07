// Bundle-wide feature flags.
//
// AI is temporarily disabled end-to-end. The `ai_assistant` canister is not
// shipped as a node in the bundle manifests (farm/local.manifest.json,
// icp.yaml), so nothing in the UI may call it. Keep this `false` until AI
// nodes are available again; flipping it back to `true` re-enables the
// assistant route, the `ai` actor, and the bootstrap wiring probes.
//
// Typed `boolean` (not the literal `false`) on purpose, so flipping the value
// doesn't turn the guarded branches into "unreachable code".
export const AI_ENABLED: boolean = false;
