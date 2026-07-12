# rustok-ai implementation plan

## Current state

`rustok-ai` is the capability-owned AI host/orchestrator. Rig 0.39 is the only
inference path for chat, streaming, tools, typed output, images, embeddings,
and reranking. RusToK keeps tenancy, RBAC, routing, policy, approvals,
persistence, traces, GraphQL/native transport, and first-party domain
operations at this boundary.

The active repair wave moves connectivity from tenant-owned profile settings to
deployment-owned `AiProviderTargetCatalog` entries. A profile selects a stable
target id, model, policy, and permitted external credential references; it
never supplies an endpoint, cloud project/region/identity, or plaintext
secret. The catalog remains locked to
`contracts/rig-0.39-provider-catalog.json`; updating Rig or adding a provider
requires an intentional snapshot change and factory evidence.

`apps/server` still contains legacy direct AI runtime construction. That is a
platform-owned prerequisite, not a `rustok-ai` implementation concern: the
target is a generic manifest/runtime contribution contract with no AI-specific
imports in the host. The capability must not be marked boundary-complete until
the platform owner removes that coupling.

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

## Delivery status and next results

| Work item | Status | Completion evidence |
|---|---|---|
| Rig-only inference cutover and provider snapshot | `in_progress` | Descriptors now carry a closed typed `ProviderIntegration` dispatch key used by chat/image and vector factories. Still required: registry/factory parity tests, protocol cassettes, and opt-in live checks for every declared integration. |
| Deployment-owned provider targets | `in_progress` | `ProviderTargetId`, deployment catalog, GraphQL/native/Next target selection, and the irreversible profile migration are implemented. Still required: deployment configuration fixtures and rejection/parity tests for every non-catalogued target. |
| Secret boundary | `in_progress` | Resolver policy, rotation invalidation, resolver emulator tests, and no secret values in transport/telemetry. |
| Agent approvals and restart | `in_progress` | A model turn now persists a durable `approval_batch_id` for all sensitive calls, claims each approval with compare-and-set, and migration-tests the legacy backfill. Still required: failure-recovery transaction coverage and persisted-history restart evidence. |
| Streaming/cancellation | `in_progress` | Cancellation tokens, `cancelled` state, per-run monotonic event sequences, hub-side duplicate-terminal suppression, Rig tool-call assembly, and final-response token usage mapping are implemented. Still required: protocol cassette coverage. |
| Generic host contribution | `blocked_platform` | Platform-owned manifest/runtime extension removes direct AI imports and construction from `apps/server`. |
| Vector-store schema and RAG UI | `not_started` | Explicitly outside this wave; engine entrypoints are the only deliverable here. |

The current wave has replaced tenant-facing provider settings with a deployment
target selector in the new contract and transport forms. The migration rejects
legacy custom endpoints rather than silently converting them: an operator must
first create the named deployment target. A target owns endpoint and cloud
settings; `SecretRef` remains the only tenant-controlled connection input,
constrained by the server-owned resolver policy. The descriptor-owned typed
integration key is now the factory dispatch source; the remaining registry
work is evidence coverage for each pinned Rig integration.

Runtime materialization repeats target schema, credential-shape, and egress
validation before constructing a Rig client. A deployment configuration change
therefore cannot bypass the checks that were applied when a profile was saved.

Rig agent recovery never executes unknown or policy-denied tool calls. It
persists a synthetic skipped result and lets the model finish the turn. A
sensitive model turn is represented as one approval batch: non-sensitive
allowed results are persisted immediately, each sensitive call is independently
approved or rejected, and the run is restored only when the batch is complete.
The final policy check occurs immediately before an approved MCP invocation.

## Remaining implementation plan

This section is the execution backlog for the unfinished Rig cutover. Completed
cutover work remains documented above and must not be reintroduced as a second
implementation path. Work proceeds in order; a wave moves to `completed` only
after its listed evidence exists. Long compilation and frontend suites belong
to R4 and are intentionally deferred while the implementation restriction is
active.

| Wave | Status | Deliverable | Exit evidence |
|---|---|---|---|
| R0. Static contract repair | `completed` | Provider token fields are normalized into `ProviderUsage`; both direct and agent streaming paths use one provider-event normalizer before hub publication; the native DTO owns its conversion from the canonical event. GraphQL/native/Next/Leptos expose typed tool calls, usage, sequence, cancellation, errors, and terminal states without a second provider protocol mapping. | Focused normalization and DTO tests were added. Changed-file formatting and `git diff --check` pass; `verify:ai:admin-boundary`, `verify:ai:fba-baseline`, and `verify:orchestrator:fba-runtime-order` pass. |
| R1. Approval recovery and restart | `in_progress` | An approved external tool result is now staged in approval metadata under `executed` before history finalization; retry replays that durable outcome instead of invoking MCP again, and trace/message/approval/run finalization is transactional. A `rustok-test-utils` SQLite fixture proves durable staging plus compare-and-set rejection of a duplicate resolver. Still cover several sensitive calls, mixed approve/reject results, stale-policy rejection before execution, tool failure, maximum turns, transaction rollback, and reconstruction exclusively from persisted RusToK history. | Targeted service tests prove compare-and-set ownership, complete-batch resume, retry semantics, transaction rollback, and restart without a serialized Rig checkpoint. |
| R2. Provider protocol evidence | `not_started` | Add one reusable cassette harness and fixtures for OpenAI-compatible, Anthropic, Gemini, cloud-auth, and deployment-owned local targets. Exercise text deltas, fragmented tool-call assembly, normalized usage, provider errors, cancellation, and exactly-once terminal publication. Add descriptor/factory/schema/feature parity against `rig-0.39-provider-catalog.json`; make a snapshot change mandatory for a Rig upgrade. | Offline cassette and registry tests cover every declared integration family. Opt-in live connectivity tests exist but are excluded from default gates. |
| R3. Security, migration, and transport parity | `not_started` | Add rejection tests for unknown/forbidden target ids, tenant-supplied endpoint or cloud coordinates, forbidden resolver alias/key, private origins, and cross-tenant secret access without value leakage. Complete migration evidence for the three legacy slugs, base URL to operator-owned target handling, plaintext preflight refusal, and removal of legacy columns. Prove GraphQL/native parity for catalogs, target selection, masked credential refs, validation errors, stream usage/tool calls, and approval states. | Security, migration, and parity suites exercise identical owner-owned semantics at both transports and expose no plaintext secret fallback. |
| R4. Final verification and evidence | `deferred` | Run the full targeted Rust suites, frontend typecheck/lint, module validation, AI boundary/FBA checks, i18n/FFA checks, and workspace check. Update README, local status rows, central registry, and FFA/FBA evidence to reflect observed results rather than planned claims. | All required gates pass, or each external failure is recorded with owner and reproducible evidence. |

### Platform dependency (outside the AI change set)

`P1` remains `blocked_platform`: foundation/runtime owners must provide the
generic manifest-backed runtime and GraphQL/native contribution contract, then
remove direct AI imports and runtime construction from `apps/server`.
`rustok-ai` subsequently registers its shared runtime through that neutral
extension context. AI work must not edit `apps/server` or claim the host
boundary complete before P1 lands.

### Explicitly later product work

Product RAG UI, vector-store schema, and remote Alloy transport are not hidden
tasks in R0-R4. The current wave exposes embeddings, reranking, and local
FastEmbed engine entrypoints only. Remote Alloy transport remains owned by the
`rustok-ai-alloy` plan.

## Verification

- `npm run verify:ai:admin-boundary`
- `npm run verify:ai:fba-baseline`
- `npm run verify:orchestrator:fba-runtime-order`
- `cargo test -p rustok-ai --features server metrics::tests direct::tests service::tests -- --nocapture`
- `cargo test -p rustok-ai --features server migrations::m20260710_000001_rig_provider_profiles::tests -- --nocapture`
- `cargo test -p rustok-ai --features server engine::agent_driver::tests -- --nocapture`
- `cargo test -p rustok-ai --features server service::tests::approval_batch_recovery -- --nocapture`
- `cargo test -p rustok-ai --features server engine::inference::usage_tests -- --nocapture`
- `cargo test -p rustok-ai --features server streaming::tests::preserves_usage_payload -- --nocapture`
- `cargo test -p rustok-ai --features server,graphql graphql::types::stream_usage_tests -- --nocapture`
- `cargo test -p rustok-secrets`
- `cargo test -p rustok-ai --features server,graphql --lib`
- Next admin typecheck/lint and Leptos native/GraphQL target-catalog parity tests

## References

- [Capability README](../README.md)
- [Capability documentation](./README.md)
- [AI capability ADR](../../../DECISIONS/2026-04-03-rustok-ai-capability-module.md)
