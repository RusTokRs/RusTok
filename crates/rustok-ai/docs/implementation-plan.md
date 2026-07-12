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

`apps/server` still contains legacy direct AI runtime construction. That is a
platform-owned prerequisite, not a `rustok-ai` implementation concern: the
target is a generic manifest/runtime contribution contract with no AI-specific
imports in the host. The capability must not be marked boundary-complete until
the platform owner removes that coupling.

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
| Rig-only inference cutover and provider snapshot | `in_progress` | Rig is the only inference path and the 0.39 inventory is pinned. The active registry-integrity repair must make the descriptor own the factory binding, reject integrations not compiled into this deployment at target-catalog load time, and replace the current feature-membership assertion with executable factory coverage. |
| Deployment-owned provider targets | `completed` | `ProviderTargetId`, deployment catalog, GraphQL/native/Next selection, migration, egress guards, and safe DTOs are covered by server and GraphQL test gates. |
| Secret boundary | `completed` | Resolver policy, rotation invalidation, non-resolving validation, tenant-prefix tests, and secret-safe DTOs are covered by the dedicated secrets gate and server tests. |
| Agent approvals and restart | `completed` | Durable batches, CAS claims, staged outcomes, transactional finalization, recovery, and canonical-history restart are covered by the server test gate. |
| Streaming/cancellation | `completed` | Cancellation, sequence, terminal suppression, tool-call assembly, usage mapping, and cassettes are covered by server and GraphQL test gates. |
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
| R0. Static contract repair | `in_progress` | Provider token fields are normalized into `ProviderUsage`; both direct and agent streaming paths use one provider-event normalizer before hub publication; the native DTO owns its conversion from the canonical event. GraphQL/native/Next/Leptos expose typed tool calls, usage, sequence, cancellation, errors, and terminal states without a second provider protocol mapping. The remaining registry-integrity slice moves the Rig factory binding into the typed integration entry (or an equally compiler-exhaustive entry-owned binding), validates target settings and egress at deployment-catalog construction, and makes public catalog surfaces return only compiled integrations. `fastembed` remains in the pinned inventory but cannot be selected or advertised unless the `fastembed` feature is compiled. | Focused normalization and DTO tests were added. Add target-load rejection, compiled-catalog filtering, and per-feature factory materialization tests before this wave is complete. Changed-file formatting and `git diff --check` pass; `verify:ai:admin-boundary`, `verify:ai:fba-baseline`, and `verify:orchestrator:fba-runtime-order` pass. |
| R1. Approval recovery and restart | `in_progress` | An approved external tool result is now staged in approval metadata under `executed` before history finalization; retry replays that durable outcome instead of invoking MCP again, and trace/message/approval/run finalization is transactional. A `rustok-test-utils` SQLite fixture proves durable staging, compare-and-set rejection of a duplicate resolver, mixed reject/approve service-level progression (`waiting_approval` → next id → `running`), and rollback of an earlier trace when a later finalization write fails. Stale policy rejects a still-pending call but cannot silently discard an already-executed outcome. Agent tests cover failed tool traces, max-turn enforcement, and restart from canonical persisted messages/tool results without a Rig checkpoint. Implementation is complete; targeted Rust execution remains deferred with the requested no-long-compilation constraint. | Run the targeted service and agent tests in R4; they prove compare-and-set ownership, complete-batch resume, retry semantics, transaction rollback, and restart without a serialized Rig checkpoint. |
| R2. Provider protocol evidence | `in_progress` | `rig-0.39-stream-cassettes.json` and a reusable isolated-hub harness cover OpenAI-compatible, Anthropic, Gemini, cloud-auth, and deployment-local normalized Rig events: text deltas, assembled tool calls, usage, accumulated content, provider errors, cancellation, monotonic sequencing, and exactly-once terminal suppression. The registry checks the pinned inventory, descriptor/integration parity, build availability, and actual factory branches; a descriptor feature must have a corresponding entrypoint or be omitted. An ignored live probe reads only deployment-owned `AiProviderConfig` JSON and `RUSTOK_AI_LIVE_*` env secret refs. | Offline cassette and registry tests cover every declared integration family that is compiled into the deployment. The ignored live probe is excluded from default gates and must run only against explicitly configured deployment targets. |
| R3. Security, migration, and transport parity | `in_progress` | Migration tests now preserve all three legacy slugs and map their no-endpoint profiles to matching deployment target ids; existing tests retain plaintext preflight and legacy-column removal evidence. Service contract tests reject unknown target ids, private origins, and tenant credential refs for workload-identity targets. `rustok-secrets` now validates resolver alias/key policy without resolving a value, and provider profile create/update applies that validation before persistence. GraphQL query documents and native DTO serialization tests lock the safe target/credential/stream shape and reject endpoint/plaintext fields. Empty capabilities now derive through one shared descriptor function in both GraphQL and native create paths. Remaining work is executing the full transport suites. | Security, migration, and parity suites exercise identical owner-owned semantics at both transports and expose no plaintext secret fallback. |
| R4. Final verification and evidence | `in_progress` | Observed fast evidence: changed-file `rustfmt --check`, `git diff --check`, `verify:ai:admin-boundary`, `verify:ai:rig-cutover`, `verify:ai:fba-baseline`, `verify:orchestrator:fba-runtime-order`, `verify:ai:domain-verticals`, and `verify:i18n:ui` pass. Rust gates passed before the registry-integrity repair: `rustok-secrets` 7/7, `rustok-ai` server 59/59 plus 1 ignored deployment-live probe, server + GraphQL 60/60 plus 1 ignored probe, and `rustok-ai-admin` SSR 17/17. Next Admin typecheck and lint pass. The aggregate FFA verifier is blocked by missing structural-shape lines in 15 unrelated module plans; this plan supplies its required line. `cargo xtask module validate ai` is intentionally not a valid gate: `rustok-ai` is a capability/support crate, not a tenant-toggled manifest module. Adding it to `modules.toml` merely to satisfy scoped validation would violate the stated host boundary and force premature server coupling. The platform prerequisite must instead provide a capability-contribution validation path, then make that path the AI scoped gate. A workspace check remains. | All required gates pass, or each external failure is recorded with owner and reproducible evidence. |

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
