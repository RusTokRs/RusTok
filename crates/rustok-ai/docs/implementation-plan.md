# rustok-ai implementation plan

## Current state

`rustok-ai` is the capability-owned AI host/orchestrator. It provides
OpenAI-compatible, Anthropic, and Gemini provider adapters; task-profile
routing; direct first-party and MCP execution; persisted sessions, runs,
traces, and approvals; GraphQL; and capability-owned Leptos and Next admin
surfaces. The detailed supported scope is maintained in the crate README.

## FFA/FBA readiness

- FFA status: `in_progress`.
- FBA status: `in_progress` (`core_transport_ui`).
- `rustok-ai` composes owner-provided registration APIs from `ai-content`,
  `ai-order`, `ai-product`, `ai-media`, and `ai-alloy`; it must not reclaim
  their task identity or policy.
- Leptos remains native `#[server]` first with the parallel GraphQL/headless
  contract. `apps/server` is a composition root, not the owner of AI
  capability logic.
- Static evidence: `crates/rustok-ai/contracts/ai-fba-registry.json`,
  `crates/rustok-ai/contracts/evidence/ai-runtime-static-matrix.json`,
  `crates/rustok-ai/contracts/evidence/ai-runtime-fallback-smoke.json`,
  `scripts/verify/verify-ai-fba-baseline.mjs`, and
  `scripts/verify/verify-orchestrator-fba-runtime-order.mjs`. Domain support
  ownership is checked by `scripts/verify/verify-ai-domain-verticals.mjs`.

## Next results

1. **Complete the remaining host-boundary extraction.** Move any AI-specific
   runtime, transport, or policy artifact still owned by `apps/server` to its
   capability or support-crate owner, leaving server composition adapters
   only. Done when the FFA boundary verifier and the FBA registry identify no
   AI capability implementation under the host.
2. **Make routing decisions actionable in persisted diagnostics.** Persist and
   expose the selected provider, rejected candidates, and fallback reason with
   bounded retention in both admin surfaces. Done when an operator can explain
   a failed or fallback run without inspecting process-local metrics.
3. **Obtain live transport evidence.** Exercise provider streaming, direct
   execution, approval, and GraphQL/native admin paths against an available
   runtime environment. Done when the evidence package covers normal and
   degraded execution without replacing the parallel GraphQL contract.

## Verification

- `npm run verify:ai:admin-boundary`
- `npm run verify:ai:fba-baseline`
- `npm run verify:orchestrator:fba-runtime-order`
- `cargo test -p rustok-ai --features server metrics::tests direct::tests service::tests -- --nocapture`

## References

- [Capability README](../README.md)
- [Capability documentation](./README.md)
- [AI capability ADR](../../../DECISIONS/2026-04-03-rustok-ai-capability-module.md)
