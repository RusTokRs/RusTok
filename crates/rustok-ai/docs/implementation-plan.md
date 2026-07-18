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
- FBA status: `boundary_ready` (`core_transport_ui`).
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
  `ai_router_policy_evidence_expanded` is locked by
  `scripts/verify/verify-ai-router-policy.mjs`: every router candidate is
  recorded with its policy decision in the durable run trace.

## Delivery status and next results

| Work item | Status | Completion evidence |
|---|---|---|
| Rig-only inference cutover and provider snapshot | `completed` | Rig is the only inference path and the 0.39 inventory is pinned. The descriptor owns factory binding, target-catalog loading rejects unavailable integrations, and executable factory coverage proves feature parity. `engine::catalog::tests` passes 8/8. |
| Deployment-owned provider targets | `completed` | `ProviderTargetId`, deployment catalog, GraphQL/native/Next selection, migration, egress guards, and safe DTOs are covered by server and GraphQL test gates. |
| Secret boundary | `in_progress` | Resolver policy, rotation invalidation, non-resolving validation, tenant-prefix tests, and secret-safe DTOs are covered by the dedicated secrets gate and server tests. Deployment composition supports env, mounted-file, Vault, Kubernetes, AWS Secrets Manager, GCP Secret Manager, and Azure Key Vault through server-owned resolver JSON. Explicit-JSON precedence, duplicate-alias rejection, and offline/lazy resolver registration are covered; Kubernetes namespace and Azure endpoint validation fail before cluster or credential discovery. Live Kubernetes/Azure identity and cloud-emulator coverage remain. |
| Agent approvals and restart | `completed` | Durable batches, CAS claims, staged outcomes, transactional finalization, recovery, and canonical-history restart are covered by the server test gate. |
| Agent principals and owner workflows | `in_progress` | Persisted principals, model assignments, workflows, stages, approval gates, recovery, dependency promotion, and canonical task-run execution are implemented. Owner descriptors validate every stage input and workflow binding; run/stage writes use lease- and state-aware compare-and-set guards. Principal create/update derives permissions solely from catalogued `TenantRbacCatalog` roles and enforces the owner descriptor permission floor. Native and GraphQL mutations fail closed without that catalog. The module-owned Leptos editor selects an owner descriptor and tenant-RBAC role checkboxes, then calls those typed mutations; it exposes no free-form role, permission, owner, or descriptor fields. Initiator authority is persisted and constrained for stage execution and approval continuation; principals and assignments are deactivated rather than deleted. |
| Streaming/cancellation | `completed` | Cancellation, sequence, terminal suppression, tool-call assembly, usage mapping, and cassettes are covered by server and GraphQL test gates. |
| Generic host contribution | `completed` | `ModuleRuntimeExtensions` now transfer typed deployment handles into `HostRuntimeContext` through a neutral foundation seam. AI publishes its deployment-owned secret registry, egress policy, provider targets, and generic work registration; `apps/server` imports no AI types or configuration. |
| Agent workflow platform contracts | `completed` | `TenantRbacCatalog` and `ModuleWorkScheduler` are available. The scheduler registers source/handler pairs by worker slug, and the generic host invokes module-owned registrations and runs the shared loop without AI persistence or task knowledge. |
| Agent model-assignment transport parity | `completed` | GraphQL and native transport expose the same typed create/update operations. The module-owned Leptos editor selects active tenant principals and active provider profiles, optionally accepts a model override, and limits execution mode to the closed `auto`/`direct`/`mcp_tooling` enum; provider capability validation remains in the service. |
| Agent workflow-run transport parity | `completed` | GraphQL and native transport both accept a workflow owner/slug plus an exact stage binding set. Owner surfaces assemble typed bindings and stage payloads; the generic AI panel deliberately does not expose a raw JSON workflow launcher. The service rejects duplicate, incomplete, cross-owner, inactive, or capability-incompatible bindings. |
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

Native and GraphQL adapters now both obtain their runtime from the same
module-owned `ai_host_runtime_from_context` factory over the neutral host
context. The factory is the only remaining `AiHostRuntime::new` call and
requires the deployment-owned secret registry, egress policy, and target
catalog from that context; it has no environment/default fallback, and runtime
construction is crate-private so another transport cannot create a second path.
Turning that
value into a process-wide generic runtime extension is implemented through
the foundation-owned `ModuleRuntimeExtensions` transfer seam. `apps/server`
does not import AI configuration or capability types.

AI no longer queries RBAC tables for actor roles. Provider profiles cannot
accept role slugs, and any legacy persisted role restriction is fail-closed
until the platform-owned `TenantRbacCatalog` supplies a typed selection.

The first platform prerequisite is now available: `rustok-api` defines the
generic `TenantRbacCatalog` contract and `rustok-rbac` publishes its built-in
tenant-scoped role/permission provider through `ModuleRuntimeExtensions`.
Agent-principal GraphQL and native create/update now consume that catalog
directly: role slugs are validated, permissions are derived from selected
roles, descriptor permission floors are enforced, and a missing catalog rejects
the mutation. The owner-owned role-selection UI now selects an owner descriptor
and typed tenant-RBAC role checkboxes before calling those mutations; it adds no
free-form role, permission, owner, or descriptor input and no host wiring.
The native bootstrap and GraphQL role/permission catalog queries also fail
explicitly when that generic capability is unavailable; they never disguise a
composition fault as an empty RBAC catalog.

`rustok-ai` now also owns a `ModuleWorkSource`/`ModuleWorkHandler` adapter for
agent workflow stages. It claims the existing `ready` stage through the
canonical lease CAS and delegates execution to the canonical stage executor;
the generic scheduler has no AI persistence or task knowledge. Before every
claim, the adapter discovers its own expired AI leases and requeues them through
the tenant-scoped service, so stale stages do not require a host-specific
recovery loop.

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
| R3. Security, migration, and transport parity | `in_progress` | Migration, secret validation, target/egress policy, safe GraphQL/native DTO contracts, and deployment-owned resolver composition are implemented. Explicit-JSON precedence and duplicate aliases are verified for offline/lazy resolver families; live Kubernetes/Azure identity and cloud-emulator coverage remain. | Run the dedicated secret resolver configuration tests, then rerun the existing secrets/server gates. |
| R4. Final verification and evidence | `in_progress` | The generic runtime composition now publishes module extension values and invokes registered durable workers with no AI-specific server construction. The scheduler observes the shared stop signal before every new claim and exits cleanly while already-claimed work completes canonically. The remaining work is targeted static and later full Rust/frontend verification evidence. | Run the full requested gates in the separate verification pass. |

### Verified R4 evidence

- `cargo xtask module validate ai` passes for the registered `runtime = "extension"`
  capability contract.
- `cargo test -p rustok-secrets --lib` passes 12 tests.
- `cargo test -p rustok-ai --features server --lib` passes 94 tests with one
  deployment-live probe intentionally ignored; the GraphQL variant passes 95
  tests with the same ignored probe.
- `cargo test -p rustok-ai-admin --features ssr --lib` passes 22 tests.
- Next Admin typecheck/lint and `npm run verify:i18n:ui` pass.

### Generic platform composition

Foundation now transfers module runtime-extension values into the neutral host
context and invokes generic durable-work registrations. `rustok-ai` publishes
its deployment handles and worker registration through this seam; `apps/server`
does not construct AI runtime, provider clients, secrets, policies, or GraphQL
roots. Incomplete composition remains fail-closed at the AI factory boundary.

### Explicitly later product work

Product RAG UI, vector-store schema, and remote Alloy transport are not hidden
tasks in R0-R4. The current wave exposes embeddings, reranking, and local
FastEmbed engine entrypoints only. Remote Alloy transport remains owned by the
`rustok-ai-alloy` plan.

### RAG v0.1 architecture track

The next RAG slice is planned as two deployment profiles behind one
`rustok-ai` retrieval contract:

- **Basic RAG** owns source references, source versions, citations and
  structure-aware retrieval through Athanor's document/atom graph. Tantivy
  provides lexical candidates and metadata filters, while optional Rig
  reranking improves the result set.
- **Semantic RAG** adds Rig embeddings and Athanor/SurrealDB vector similarity
  search. Additional Athanor modules may provide parsers, connectors,
  embedding providers or retrieval strategies; no `pgvector` installation is
  required for the embedded RusToK + Athanor deployment.

The base AI schema remains provider-neutral. `rustok-search` keeps ownership
of its own FTS/trigram read model; RAG does not reuse Search tables. Version
0.1 uses embedded Athanor as the lexical, structural and semantic data plane,
while future external providers must be added behind the same retrieval
contract.

## Verification

- `npm run verify:ai:admin-boundary`
- `npm run verify:ai:rig-cutover`
- `npm run verify:ai:fba-baseline`
- `node scripts/verify/verify-ai-router-policy.mjs`
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
