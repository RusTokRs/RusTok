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
requires an intentional snapshot change and executable factory evidence. The
snapshot is an inventory guard, not evidence that every advertised feature can
be materialized by the selected build.

The platform now supplies a generic manifest/runtime contribution contract with
no AI-specific imports in `apps/server`. AI GraphQL surfaces and runtime data
are composed through generated generic contributions; final boundary status
still requires targeted platform verification evidence.

## FFA/FBA readiness

- FFA status: `in_progress`.
- FBA status: `in_progress` (`core_transport_ui`).
- Structural shape: `core_transport_ui`.
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
| Rig-only inference cutover and provider snapshot | `completed` | Rig is the only inference path and the 0.39 inventory is pinned. The descriptor owns factory binding, target-catalog loading rejects unavailable integrations, and executable factory coverage proves feature parity. `engine::catalog::tests` passes 8/8. |
| Deployment-owned provider targets | `completed` | `ProviderTargetId`, deployment catalog, GraphQL/native/Next selection, migration, egress guards, and safe DTOs are covered by server and GraphQL test gates. |
| Secret boundary | `completed` | Resolver policy, rotation invalidation, non-resolving validation, tenant-prefix tests, and secret-safe DTOs are covered by the dedicated secrets gate and server tests. |
| Agent approvals and restart | `completed` | Durable batches, CAS claims, staged outcomes, transactional finalization, recovery, and canonical-history restart are covered by the server test gate. |
| Streaming/cancellation | `completed` | Cancellation, sequence, terminal suppression, tool-call assembly, usage mapping, and cassettes are covered by server and GraphQL test gates. |
| Generic host contribution | `completed` | The platform-owned manifest/runtime extension removes direct AI imports and construction from `apps/server`; generic `ModuleRuntimeExtensions` and manifest-generated GraphQL surfaces carry host composition. Source boundary audit is clean and `cargo check -p rustok-core` passes. |
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
| R0. Static contract repair | `completed` | Provider events are normalized before publication; native DTO conversion is canonical. `ProviderIntegration` owns compiler-exhaustive factory capability binding and target loading rejects unavailable integrations. | `engine::catalog::tests` passes 8/8, including compiled-catalog filtering, target-load rejection, and descriptor/factory parity. |
| R1. Approval recovery and restart | `completed` | Durable approved outcomes, transactional finalization, batch compare-and-set ownership, recovery, and canonical-history restart are implemented. | `cargo test -p rustok-ai --features server --lib` passes 59 tests with one deployment-live probe intentionally ignored. |
| R2. Provider protocol evidence | `completed` | Offline cassettes cover OpenAI-compatible, Anthropic, Gemini, cloud-auth, and deployment-local normalized Rig streams; live probes remain opt-in. | Server and GraphQL test gates pass; `verify-ai-rig-cutover.mjs` passes. |
| R3. Security, migration, and transport parity | `completed` | Migration, secret validation, target/egress policy, and safe GraphQL/native DTO contracts are implemented. | `cargo test -p rustok-secrets` passes 7 tests; server + GraphQL library gate passes 60 tests with one ignored live probe. |
| R4. Final verification and evidence | `in_progress` | Fast, Rust, Next Admin, i18n, module-validation, registry-integrity, and generic host-contribution evidence pass. The generic contribution compiles through `cargo check -p rustok-core`; `apps/server` has no AI-specific imports. The aggregate FFA verifier reaches an unrelated product locale-contract failure. `cargo check --workspace` reaches an unrelated `rustok-storage` failure: `PutObjectError::code()` lacks the `aws_sdk_s3::error::ProvideErrorMetadata` trait import in `src/s3.rs:178`. | All required gates pass, or each external failure is recorded with owner and reproducible evidence. |

### Verified R4 evidence

- `cargo xtask module validate ai` passes for the registered `runtime = "extension"`
  capability contract.
- `cargo test -p rustok-secrets` passes 7 tests.
- `cargo test -p rustok-ai --features server --lib` passes 59 tests with one
  deployment-live probe intentionally ignored; the GraphQL variant passes 60
  tests with the same ignored probe.
- `cargo test -p rustok-ai-admin --features ssr --lib` passes 17 tests.
- Next Admin typecheck/lint and `npm run verify:i18n:ui` pass.

### Platform dependency (outside the AI change set)

`P1` is implemented: foundation now exposes `GraphqlRuntimeInputs` and a
typed contribution descriptor; the manifest-driven server codegen composes AI
query/mutation/subscription roots and applies its runtime-data factory.
`rustok-ai` owns `AiGraphqlRuntimeData` and materializes it from neutral host
values. `apps/server` no longer imports AI types, constructs `AiHostRuntime`,
or declares AI GraphQL roots; it only builds the neutral input carrier.

### Explicitly later product work

Product RAG UI, vector-store schema, and remote Alloy transport are not hidden
tasks in R0-R4. The current wave exposes embeddings, reranking, and local
FastEmbed engine entrypoints only. Remote Alloy transport remains owned by the
`rustok-ai-alloy` plan.

## Verification

- `npm run verify:ai:admin-boundary`
- `npm run verify:ai:rig-cutover`
- `npm run verify:ai:fba-baseline`
- `npm run verify:orchestrator:fba-runtime-order`
- `cargo test -p rustok-ai --features server metrics::tests -- --nocapture`
- `cargo test -p rustok-ai --features server direct::tests -- --nocapture`
- `cargo test -p rustok-ai --features server service::approval_outcome_tests -- --nocapture`
- `cargo test -p rustok-ai --features server service::helpers::tests -- --nocapture`
- `cargo test -p rustok-ai --features server migrations::m20260710_000001_rig_provider_profiles::tests -- --nocapture`
- `cargo test -p rustok-ai --features server migrations::m20260712_000001_provider_targets::tests -- --nocapture`
- `cargo test -p rustok-ai --features server engine::catalog::tests -- --nocapture`
- `cargo test -p rustok-ai --features server engine::agent_driver::tests -- --nocapture`
- `cargo test -p rustok-ai --features server engine::inference::usage_tests -- --nocapture`
- `cargo test -p rustok-ai --features server engine::inference::live_connectivity_tests -- --ignored probes_each_declared_live_provider_target`
- `cargo test -p rustok-ai --features server streaming::tests -- --nocapture`
- `cargo test -p rustok-ai --features server,graphql graphql::types::stream_usage_tests -- --nocapture`
- `cargo test -p rustok-ai-admin --features ssr model::provider_profile_payload_tests -- --nocapture`
- `cargo test -p rustok-ai-admin contract_tests -- --nocapture`
- `cargo test -p rustok-secrets`
- `cargo test -p rustok-ai --features server,graphql --lib`
- Next admin typecheck/lint and Leptos native/GraphQL target-catalog parity tests

## References

- [Capability README](../README.md)
- [Capability documentation](./README.md)
- [AI capability ADR](../../../DECISIONS/2026-04-03-rustok-ai-capability-module.md)
