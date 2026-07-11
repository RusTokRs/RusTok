# rustok-ai implementation plan

## Current state

`rustok-ai` is the capability-owned AI host/orchestrator. It uses Rig 0.39 through a
single registry-driven engine; task-profile
routing; direct first-party and MCP execution; persisted sessions, runs,
traces, and approvals; GraphQL; and capability-owned Leptos and Next admin
surfaces. The detailed supported scope is maintained in the crate README.

The Rig cutover is owner-contained: provider profiles persist a stable
`provider_slug`, typed settings and external `credential_refs`; `ProviderSlug`,
the catalog and `ProviderFeature` prevent provider drift; and `rustok-secrets`
resolves tenant-authorized external secrets. `AiMigrationSource` is exported by
this crate for module-registry composition. No AI implementation or provider
knowledge belongs in `apps/server`.

The catalog is locked to
`contracts/rig-0.39-provider-catalog.json`. Updating Rig or adding a provider
requires an intentional snapshot change and the catalog factory test to pass.

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
4. **Complete Rig cutover evidence.** Maintain catalog factory tests, migration
   preflight tests, secret-resolver contract tests and opt-in provider live
   connectivity tests for every descriptor. Product vector-store schema and
   RAG UI remain outside this implementation wave.

## Verification

- `npm run verify:ai:admin-boundary`
- `npm run verify:ai:fba-baseline`
- `npm run verify:orchestrator:fba-runtime-order`
- `cargo test -p rustok-ai --features server metrics::tests direct::tests service::tests -- --nocapture`
- `cargo test -p rustok-ai --features server migrations::m20260710_000001_rig_provider_profiles::tests -- --nocapture`
- `cargo test -p rustok-ai --features server engine::agent_driver::tests -- --nocapture`
- `cargo test -p rustok-secrets`

## References

- [Capability README](../README.md)
- [Capability documentation](./README.md)
- [AI capability ADR](../../../DECISIONS/2026-04-03-rustok-ai-capability-module.md)
