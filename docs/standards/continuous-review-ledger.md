---
id: doc://docs/standards/continuous-review-ledger.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: active
---

## Deep Full-Stack Audit Cycle — 2026-09-28

**Status:** ACTIVE  
**Active phase:** FS-22 — `apps/server` composition root  
**Current main SHA:** `070f2bf6b2bec3229cdd0e091491adddf2364392`  
**Active branch:** `main`

**Purpose:** perform a fresh, sequential, root-to-leaf audit of the entire repository. Older ACRE component-round completion and the 2026-09-27 FS-00..FS-20 audit are historical evidence only; no current component is considered closed merely because it was previously audited.

### Deep Audit Execution Protocol — mandatory for every phase

The audit must move slowly enough to discover second-order defects. A phase is not a single scan and not a single patch. Every phase uses the following closed loop:

1. **Discovery pass:** read the complete in-scope production path end-to-end, including callers, persistence, configuration, transport adapters, failure paths, and adjacent boundaries. Pattern scanners are supporting evidence only.
2. **Invariant map:** write down the business, security, tenancy, authorization, transaction, concurrency, retry/idempotency, lifecycle, and compatibility invariants that the path must preserve.
3. **Finding isolation:** group findings by root cause. Do not batch unrelated fixes merely because they touch the same file. Prefer one small coherent remediation unit at a time.
4. **Implementation pass:** fix the root cause, not the visible symptom. Do not redesign unrelated code during the same remediation unit.
5. **Immediate re-audit:** after every remediation unit, re-read the entire changed function/module and all direct callers/callees. Explicitly inspect what assumptions the change invalidated.
6. **Adjacent-boundary re-audit:** inspect the nearest upstream and downstream boundaries (transport, auth, tenant/channel/locale context, DB schema, events/outbox, cache, worker, UI/CLI contract as applicable). This is mandatory even when the initial finding appeared local.
7. **Regression audit:** compare the pre-change and post-change behavior as a reviewer would: removed behavior, newly reachable states, error mapping, fallback behavior, defaults, feature-flag interactions, concurrency behavior, and observability. Ask specifically: "What new bug could this change have introduced?"
8. **Fresh second pass:** repeat the audit against the modified area without relying on the original finding list. New findings discovered here are treated as first-class findings, not dismissed as out of scope merely because the first pass missed them.
9. **Static verification:** run only repository-approved static/source verification available in the environment; never claim tests, clippy, or gatekeeper success unless actually executed.
10. **Commit boundary:** commit only after the re-audit loop is clean. The phase branch must stay small and reviewable.
11. **Pre-PR review:** inspect the complete branch diff against refreshed main; verify that documentation/ledger statements exactly match implementation and that no unrelated file drift entered the branch.
12. **Post-merge refresh:** refresh main, then perform a lightweight reconciliation of the merged result before starting the next phase.

**Regression prohibition:** a fix that introduces a new correctness, security, data-integrity, availability, performance, or compatibility defect is not considered a successful remediation. The phase must remain open until the introduced defect is also fixed and re-audited.

**Small-step rule:** if a phase uncovers a large architectural problem, split it into additional subphases in this same ledger rather than making a large speculative rewrite.
### Execution contract

- Canonical trigger: `реализуй план аудита`.
- This section is the temporary living plan for the current audit cycle; do not create a second audit checklist.
- Governance preflight is recorded here, but the implementation audit starts at the server/runtime boundary and continues through every affected layer to the final shared-library surface.
- Execute phases strictly in order, one phase at a time.
- Within the active phase, execute the ordered module tracks strictly in order. A module track may span multiple consecutive iterations when the code warrants it.
- Before each iteration, refresh `main`, record its SHA, and create/use only a dedicated iteration branch from that refreshed SHA.
- Audit first, then implement every repository-owned in-scope root-cause defect assigned to the current primary module and coherent iteration slice, using the mandatory closed-loop re-audit protocol above.
- After every remediation unit, re-audit the changed area and adjacent boundaries for newly introduced defects before continuing.
- Commit only after the current iteration's complete branch diff has passed the fresh re-audit.
- Commit the audit/implementation result, open a PR to `main`, merge it, then refresh `main` and perform the post-merge reconciliation before the next phase.
- Do not mutate another agent's branch or force-update shared history.
- Tests are run by the maintainer/user. The agent MUST NOT run test suites unless this rule is explicitly changed; tests may be inspected and static/source checks may be performed.
- Do not declare a phase complete until repository-owned findings are fixed or explicitly blocked by an owner decision/ADR, the phase branch is integrated into `main`, and the ledger is updated.
- Quality bar: business invariants, persistence, authorization, tenant/channel/locale boundaries, transactions, concurrency, retries/idempotency, events/outbox, projections, transports, UI/operator paths, dependencies, and documentation must be audited—not only pattern counts or lint.

### Session Contract Read Record

- [x] `AGENTS.md` read and treated as the canonical agent/governance file.
- [x] `agents.md` checked; absent. No parallel governance file is to be created.
- [x] `docs/index.md` read.
- [x] `docs/CONTINUOUS_CODE_REVIEW.md` read.
- [x] `docs/standards/continuous-review-ledger.md` read.
- [x] `docs/verification/README.md` read.
- [x] `docs/standards/coding.md` read.
- [x] `docs/modules/module-authoring.md` read.
- [x] `docs/modules/registry.md` read.
- [x] Relevant architecture contracts reviewed: principles, API, database, i18n, and dependency rules.
- [x] User conditions recorded: full repository/deep business-logic audit; sequential server-to-library coverage; no half-measures/root-cause fixes; dedicated branch before implementation; commit + PR + merge to `main` after each phase; maintainer-owned tests; one repeatable trigger `реализуй план аудита`.
- [x] Initial `main` SHA recorded before this cycle.

### FS-21 Closeout

- **Status:** COMPLETE and integrated into `main`.
- **Audit start:** `main` SHA `7e1d342c1a7bb846cd7bc13443708cc493fe45ce`.
- **Implementation head before merge:** `21b9327830b7d61c5f59411812f2486df0d065da`.
- **Merged to main:** `81ab275fda9788c7727d95f089e220307833f203` via PR #4157.
- **Coverage completed:** deployment/runtime config resolution, production image contract, environment/path safety, health routing/probes, Unix shutdown signal handling, and runtime stop-handle bootstrap race; topology documentation was reconciled with the actual compose behavior.
- **Verification:** changed sources and cross-file contracts were statically inspected through repository contents and commit diff. No test suites, cargo clippy, or other test commands were executed by the agent, per the maintainer-owned test rule. Repository access from the execution container could not clone the GitHub repository, so the local gatekeeper command was not run here; no claim of a passed local gatekeeper is made.
- **Deferred by scope:** full worker lifecycle/join/abort coverage remains FS-24, where all background worker implementations will be audited together rather than partially patched in the server-host phase.
- **Next phase:** FS-22 — `apps/server` composition root.

### FS-21 Findings and Implementation

- [x] **SERVER-21-01 — production config path was compile-time coupled to the build workspace.** `load_config` used `env!("CARGO_MANIFEST_DIR")`, while the production image copies configuration to `/app/config`. Release binaries built in the image therefore could not resolve the copied runtime config. Config resolution now supports `RUSTOK_CONFIG_DIR`, prefers a `config` sibling next to the executable for deployed binaries, and retains the source-tree fallback for development.
- [x] **SERVER-21-02 — release binaries defaulted to the development environment.** Missing `RUSTOK_ENV`/`APP_ENV` selected `development` regardless of build mode. Release builds now default to `production`, making omitted environment selection fail closed when a production config is absent.
- [x] **SERVER-21-03 — environment name was used directly as a config filename.** `RUSTOK_ENV`/`APP_ENV` could contain path traversal segments. Environment names are now bounded to ASCII letters, digits, `-` and `_` before constructing the filename.
- [x] **SERVER-21-04 — development startup health probe targeted a nonexistent endpoint.** `scripts/dev-start.sh` polled `/api/health`, while the server exposes health under the root health namespace. The script and quickstart now use `/health`.
- [x] **SERVER-21-05 — canonical liveness route disagreed with its documented path.** The health router registered `/health/` while documentation and container health checks use `/health`. The route is now canonicalized to `/health`.
- [x] **SERVER-21-06 — Docker SIGTERM was not part of host shutdown handling.** The production image declares `STOPSIGNAL SIGTERM`, but `shutdown_signal` listened only for Ctrl-C. Unix hosts now handle SIGTERM as a graceful shutdown trigger while retaining Ctrl-C handling.
- [x] **SERVER-21-07 — runtime worker stop-handle initialization had a bootstrap check-then-insert race.** `connect_runtime_workers_with_runtime` now uses the context's atomic `StopHandle::ensure` path instead of separate `shared_contains`/`shared_insert`/`expect` steps.

- [x] **SERVER-21-08 — production image contained development/test configuration and did not self-declare its production config contract.** The production stage copied the entire `apps/server/config` directory, including known development/test credentials. It now creates an empty operator-owned `/app/config` mount point, sets `RUSTOK_ENV=production`, `RUSTOK_CONFIG_DIR=/app/config`, and `RUSTOK_HTTPS=true`, and deliberately excludes development/test YAML files from the production image.

### Phase Granularity Rule

The numbered FS phases define architectural ownership, not a permission to inspect an entire subsystem in one pass. Before implementation, the active phase must be decomposed in the ledger into ordered module tracks small enough that each production module can be read end-to-end and re-audited after each coherent remediation. A module track may require multiple iterations when the module contains several independent flows, root-cause findings, or substantial implementation surface. Do not advance to the next module while an introduced regression or unexplained invariant violation remains.

**Phase Granularity Rule — one primary production module per iteration, with multiple iterations allowed per module**

The numbered FS phases define architectural ownership, not permission to audit a subsystem, route family, or collection of modules in one pass.

Each audit iteration has exactly **one primary production module/component**. The primary unit is normally one Rust module/file such as `apps/server/src/middleware/metrics_auth.rs`, or one similarly bounded owner module elsewhere in the repository.

**A single module may require multiple consecutive audit iterations.** This is expected when the module contains several independent flows, multiple root-cause findings, complex failure behavior, or a remediation that must be split into several safe steps. Multiple iterations on the same module are allowed and should be preferred over widening one iteration or batching unrelated fixes.

The iteration may read direct callers/callees, contracts, configuration, persistence, tests, and adjacent boundaries only as necessary to prove the primary module's invariants. Those surrounding reads are evidence for the primary module, not additional primary scope.

Hard limits for every iteration:

1. Do not audit multiple sibling modules as primary scope.
2. Do not make unrelated fixes discovered outside the primary module; record them for their own later module track.
3. Every iteration must complete the mandatory discovery, invariant mapping, implementation/assessment, immediate re-audit, adjacent-boundary re-audit, regression audit, and fresh second pass for the portion of the primary module addressed in that iteration.
4. One iteration produces one small coherent branch/PR/merge. After merge, refresh `main`; the next iteration may continue the same primary module or move to the next module according to the module track's remaining work.
5. When a module exposes several flows, audit those flows sequentially inside that same module track rather than expanding to neighboring modules.
6. Finding counts do not justify widening the scope. If the module reveals a large cross-module defect, isolate the root cause and create a later dedicated iteration for the owning module.
7. A module is not marked complete until a module-level fresh second pass finds no remaining repository-owned in-scope defects for that module, or all remaining issues are explicitly blocked by an owner decision/ADR.
8. The phrase `реализуй план аудита` means: continue the **current open module track** with its next coherent iteration; when that module track is complete, take the **next unchecked primary module**.

### Current module-by-module execution queue

**FS-22 — apps/server composition root**

- [x] **FS-22.02.01 — `apps/server/src/middleware/metrics_auth.rs`** — module track: observability authentication, readiness sanitization, bearer parsing, production/development fail-closed behavior, response status contract, and direct middleware placement evidence. **This module may take multiple consecutive iterations; do not advance to FS-22.02.02 until its module-level second pass is clean or remaining issues are explicitly blocked.**
- [x] **FS-22.02.02 — `apps/server/src/middleware/registry_artifact_access.rs`** — one-module audit.
- [x] **FS-22.02.03 — `apps/server/src/middleware/registry_remote_claim.rs`** — one-module audit.
- [x] **FS-22.02.04 — `apps/server/src/middleware/registry_publish_policy.rs`** — one-module audit.
- [x] **FS-22.02.05 — `apps/server/src/middleware/rate_limit.rs`** — one-module audit.
- [x] **FS-22.02.06 — `apps/server/src/middleware/auth_context.rs`** — one-module audit.
- [x] **FS-22.02.07 — `apps/server/src/middleware/channel.rs`** — one-module audit.
- [ ] **FS-22.02.08 — `apps/server/src/middleware/locale.rs`** — one-module audit.
- [ ] **FS-22.02.09 — `apps/server/src/middleware/tenant.rs`** — one-module audit.
- [ ] **FS-22.02.10 — `apps/server/src/middleware/guest_access_http.rs` or its host adapter** — one-module audit.
- [ ] **FS-22.02.11 — `apps/server/src/middleware/security_headers.rs`** — one-module audit.
- [ ] **FS-22.02.12 — `apps/server/src/services/server_bootstrap.rs`** — one-module audit.
- [ ] **FS-22.02.13 — `apps/server/src/services/app_runtime.rs`** — one-module audit.
- [ ] **FS-22.02.14 — `apps/server/src/services/server_runtime_context.rs`** — one-module audit.
- [ ] **FS-22.02.15 — `apps/server/src/services/graphql_schema.rs`** — one-module audit.
- [ ] **FS-22.02.16 — `apps/server/src/controllers/graphql.rs`** — one-module audit.
- [ ] **FS-22.02.17 — `apps/server/src/controllers/auth.rs`** — one-module audit.
- [ ] **FS-22.02.18 — `apps/server/src/controllers/oauth.rs`** — one-module audit.
- [ ] **FS-22.02.19 — `apps/server/src/controllers/users.rs`** — one-module audit.
- [ ] **FS-22.02.20 — `apps/server/src/controllers/health.rs`** — one-module audit.
- [ ] **FS-22.02.21 — `apps/server/src/controllers/metrics.rs`** — one-module audit.
- [ ] **FS-22.02.22 — `apps/server/src/controllers/marketplace_registry.rs`** — one-module audit.
- [ ] **FS-22.02.23 — `apps/server/src/controllers/artifact_http.rs`** — one-module audit.
- [ ] **FS-22.02.24 — `apps/server/src/controllers/artifact_permissions.rs`** — one-module audit.
- [ ] **FS-22.02.25 — `apps/server/src/controllers/admin_events.rs`** — one-module audit.
- [ ] **FS-22.02.26 — `apps/server/src/controllers/channel.rs`** — one-module audit.
- [ ] **FS-22.02.27 — `apps/server/src/controllers/flex.rs`** — one-module audit.
- [ ] **FS-22.02.28 — `apps/server/src/controllers/installer.rs`** — one-module audit.
- [ ] **FS-22.02.29 — `apps/server/src/controllers/mcp.rs`** — one-module audit.
- [ ] **FS-22.02.30 — `apps/server/src/controllers/oauth_metadata.rs`** — one-module audit.
- [ ] **FS-22.02.31 — `apps/server/src/controllers/swagger.rs`** — one-module audit.
- [ ] **FS-22.02.32 — `apps/server/src/channels/builds.rs`** — one-module audit.
- [ ] **FS-22.03 — identity/auth propagation:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.04 — tenant/channel/locale propagation:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.05 — GraphQL composition:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.06 — REST/controller composition:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.07 — Server-function composition:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.08 — Embedded UI composition:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.09 — Feature/config interaction matrix:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.10 — Error/observability boundary:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.11 — Fresh second-pass composition audit:** perform this only after the module queue above has been completed, still one primary module per iteration.

### FS-22.02.01 Iteration 1 — `metrics_auth.rs`

- **Base:** refreshed `main` at `fc092d993bf455c0f591d4b622f4001c3b0eaa4b` before the refreshed iteration branch was created.
- **Dedicated iteration branch:** `audit/fs-22.02.01-i1-refresh-20260928`. The previous branch was not merged because `main` advanced after its creation; no stale branch was merged.
- **Invariant map:** observability endpoints must not expose detailed runtime/module diagnostics without a valid bearer token in production; development/non-production must retain local unauthenticated observability; readiness must expose only the aggregate status publicly; valid authenticated requests may receive the diagnostic body; protected paths must return stable 401/503 contracts; auth must remain independent of tenant resolution and must execute across all host profiles.
- **Finding OBS-22.02.01-01:** `metrics_auth.rs` used compile-time `cfg!(debug_assertions)` to decide whether a missing observability token was allowed. Server production semantics are runtime-derived from `RUSTOK_ENV`/`RUST_ENV`/`APP_ENV`. A debug build launched with `RUSTOK_ENV=production` therefore bypassed the documented production fail-closed behavior and could expose unauthenticated detailed observability/runtime/module endpoints and readiness details.
- **Implementation:** replaced the compile-profile check with the runtime production predicate from `crate::common::is_production_environment()`, evaluated once per request and shared by protected observability and readiness authorization. Added a focused policy test covering production/non-production behavior.
- **Immediate/adjacent re-audit:** re-read the complete changed module and direct registration in `services/app_router.rs`; rechecked `health.rs`, `metrics.rs`, `http_stack.rs`, and runtime settings. No introduced panic, header, bearer parsing, response-status, body-bound, token-precedence, or middleware-placement defect was found.
- **Deferred cross-module finding:** public `/health/ready` is not covered by the current path-rate-limit policies (which are limited to `/api/`) while performing multiple database/network/runtime checks per request. This is recorded for the dedicated `FS-22.02.05` rate-limit module audit rather than changing a sibling module here.
- **Fresh second pass:** independently rechecked path coverage, fail-closed policy, bearer parsing, constant-time comparison, readiness sanitization/status normalization, body-size bound, header behavior, environment fallback, direct placement, and adjacent health/metrics/HTTP-edge contracts. No remaining repository-owned in-scope defect was found in `metrics_auth.rs`.
- **Verification:** repository-content/static inspection and branch-diff review only. No tests, clippy, gatekeeper, build, or runtime commands were executed by the agent, per maintainer-owned verification rules.
- **Status:** module-level second pass clean; `FS-22.02.01` is complete and the next audit trigger should start `FS-22.02.02`.

### FS-22.02.02 Iteration 1 — `registry_artifact_access.rs`

- **Base:** refreshed `main` at `df08de4d1d3dbf57a2655ae884392652c7deb52e` before the refreshed branch was created; the earlier stale iteration branch was intentionally not merged.
- **Dedicated iteration branch:** `audit/fs-22.02.02-i1-refresh2-20260928`.
- **Invariant map:** session-backed registry mutations require authenticated user authority; OAuth service tokens and legacy registry actor headers cannot become registry authority; request-specific publish actions must be owner/requester/modules:manage constrained; runner access is a host-global shared-token contract unless the protocol explicitly carries claim binding; uploads must be bounded and use owner-issued immutable storage slots; owner-transfer validation must not be mistaken for the authoritative persistence invariant; routing/method classification must not create an authorization bypass.
- **Discovery:** read the complete middleware and direct app-router placement, plus the registry marketplace controller route family, registry publish policy, registry principal mapping, remote-runner claim/transition adapters, server governance adapter, and owner persistence implementation for artifact upload/download, validation/review, release yank, owner transfer, and remote validation claim lifecycle.
- **Finding isolation:** no repository-owned security or data-access bypass remains in `registry_artifact_access.rs`. The seemingly broad runner-token artifact-download path is consistent with the current owner contract: the remote claim exposes a request-specific download URL while runner authentication and download routing remain host-owned. Adding claim-id binding here would require a protocol change not represented by the existing contract.
- **Confirmed deferred root cause — REGISTRY-GOV-01:** the middleware checks that an owner-transfer target user is active and in the caller tenant, but the authoritative `SeaOrmModuleGovernanceService::transfer_owner` transaction validates only principal shape before updating `registry_module_owners`. A concurrent target deactivation/deletion between the middleware preflight and persistence can therefore bind the registry owner to an inactive/nonexistent user. The fix belongs in the owning persistence module at the transaction boundary and is recorded separately below.
- **Immediate re-audit:** complete middleware re-read found zero runtime `unwrap`, `expect`, `panic!`, `todo!`, or silent-error suppression; DB access is parameterized; body reads are bounded; authentication context is copied before the relevant await; runner-token comparison uses constant-time equality; upload content type is normalized deliberately; unknown/non-matching routes pass through to their owning controller.
- **Adjacent-boundary re-audit:** live publish/upload/validation/review controller paths reconstruct authority through `authority_from_auth`, which rejects OAuth service tokens and legacy headers; owner services reauthorize request mutations at the owner boundary; remote runner claim/heartbeat/terminal transitions use atomic claim/lease predicates; release yank and owner transfer are authorized in owner transactions; artifact storage slots are content-addressed/create-only.
- **Regression audit:** verified profile-independent placement in `compose_application_router`, method/path dispatch, body-size behavior, anonymous access to request status/artifacts, runner-token failure behavior, and duplicated controller authorization. No new bypass was introduced by existing middleware behavior.
- **Fresh second pass:** independently compared middleware classification against all registry controller route families. Specialized external-prebuilt/platform-build/signature/yank paths intentionally retain controller + owner authorization rather than duplicating partial policy here. The module itself remains clean.
- **Verification:** repository-content/static inspection and branch-diff review only. No test suites, clippy, gatekeeper, build, or runtime commands were executed by the agent, per maintainer-owned test rules.
- **Status:** module-level second pass clean; `FS-22.02.02` complete. Next planned primary module remains `FS-22.02.03 — registry_remote_claim.rs`; REGISTRY-GOV-01 must be handled in a dedicated later owning-module iteration.


### FS-22.02.03 Iteration 1 — `registry_remote_claim.rs`

- **Base:** refreshed `main` at `42fe339197e7c491e8a0d29b873fc162bd3bce17`; the earlier branch from `63dbd301b407bca16d8a4fcb1a2b104da00cd543` was intentionally not merged because `main` advanced concurrently.
- **Dedicated iteration branch:** `audit/fs-22.02.03-i1-refresh-20260928`.
- **Invariant map:** remote runner routes are available only through the enabled host-global runner boundary with a configured shared token; token comparison is constant-time; runner request bodies are bounded before JSON parsing; schema version and runner identity are validated consistently; claim/heartbeat/complete/fail remain owner-authoritative CAS transitions bound by claim id, canonical runner id, lease state, and request revision; route matching must not create aliases beyond the canonical controller contract; remote runner access does not inherit tenant/user auth because it is a separate shared-token protocol.
- **Discovery:** re-read the complete middleware plus direct app-router placement, registry controller runner routes, remote runner adapter, remote transition adapter, owner-side remote validation claim/lease commands, runtime host modes, rate-limit placement, deployment-profile documentation, and the registry runner worker contract. The existing owner CAS/revision/lease design remains intact.
- **Finding REGISTRY-RUNNER-01:** the controller canonicalizes `runner_id` with `trim()` before every remote transition, while the globally installed middleware forwarded heartbeat/terminal input unchanged. The owner claim transaction stores the canonical trimmed runner identity, so a runner using accepted surrounding whitespace could successfully claim a lease but then fail its heartbeat/terminal CAS with a runner-mismatch. This was a real transport normalization defect, not an authorization redesign issue.
- **Implementation REGISTRY-RUNNER-01:** added one middleware-local `normalize_runner_id` helper and routed claim, heartbeat, and terminal operations through it. The owner/transition layer still receives the same canonical identity it previously expected; no CAS semantics were changed.
- **Finding REGISTRY-RUNNER-02:** `runner_route` used `trim_matches('/')`, so the globally short-circuiting middleware accepted undocumented leading/trailing slash variants that the canonical controller routes do not register. This created additional transport aliases outside the declared path contract.
- **Implementation REGISTRY-RUNNER-02:** changed route parsing to exact path-segment matching, rejecting extra leading/trailing/doubled slash forms; added focused boundary assertions for the canonical path and rejected aliases.
- **Documentation reconciliation:** the current composition deliberately installs the runner boundary before host-profile divergence, including worker mode when remote execution is enabled. The server README still described worker HTTP as health/metrics-only, so the documentation was updated to describe the profile-independent runner boundary explicitly. Runtime profile behavior was not changed.
- **Immediate re-audit:** re-read the full changed middleware and direct caller/callee boundaries. Token fail-closed behavior, constant-time comparison, body bound, schema checks, claim/lease CAS delegation, request revision propagation, terminal reason-code normalization, error-category mapping, and non-runner pass-through remain unchanged. Production code contains no unwrap/expect/panic/todo or silent-error suppression.
- **Adjacent-boundary re-audit:** controller `validate_runner_id` trims the same identity; transition adapter forwards the normalized identity; owner heartbeat/terminal predicates still bind claim id + runner id + running remote lease; owner claim still normalizes supported stages and advances the request revision atomically; controller route constants remain the canonical route source; rate limiting remains outer to this middleware in all composed profiles.
- **Regression audit:** verified the patch changes only identity canonicalization and route exactness in the primary module plus the directly related server documentation and audit ledger. No runner-token semantics, lease TTL, request-revision CAS, claim ownership, stage authorization, or status mapping was loosened.
- **Fresh second pass:** independently rechecked the module from the transport entrypoint outward, including unusual path forms, whitespace identities, empty identities, unsupported stages, malformed/oversized bodies, disabled/misconfigured executor, missing/invalid token, stale terminal revision, expired lease, and non-runner pass-through. No remaining repository-owned in-scope defect was found in `registry_remote_claim.rs`.
- **Verification:** repository-content/static inspection and branch-diff review only. No tests, clippy, gatekeeper, build, or runtime commands were executed by the agent, per maintainer-owned verification rules.
- **Status:** module-level second pass clean; `FS-22.02.03` complete. Next planned primary module is `FS-22.02.04 — registry_publish_policy.rs`.
- **Merged:** PR #4170, merge commit `a1207f53863e6a57e764d4b4a9a08fc549c80d73`.


### FS-22.02.04 Iteration 1 — `registry_publish_policy.rs`

- **Base:** refreshed `main` at the GitHub-reported commit `fabe0214d19693a2807887db6e5b9b92e9d4f28b`; branch created from the current `main` ref before implementation.
- **Dedicated iteration branch:** `audit/fs-22.02.04-i1-20260928`.
- **Invariant map:** the publish policy may only affect the canonical `POST /v2/catalog/publish` route; request bodies must remain bounded before buffering; ownership and trust classification values persisted downstream must be from the accepted canonical vocabulary; live publication must require the accepted Registry V2 session-backed direct-user authority and `modules:manage`; OAuth service/delegated-user tokens and legacy registry headers must not become live registry authority; dry-run must remain non-mutating and available as preview; normalized body forwarding must preserve downstream handler behavior and headers consistently; the owner remains the authoritative persistence/concurrency boundary.
- **Discovery:** re-read the complete middleware and its actual invocation path through `registry_remote_claim`, the app-router layer order, the publish controller, `AuthContext`/principal classification, registry authority adapter, owner `ModulePublishRequestCreateCommand`, marketplace request DTO, registry architecture/ADR documentation, and the owner publication lifecycle. The middleware runs after tenant/locale/auth context resolution and before the registry controller, and it is intentionally invoked by the global registry middleware rather than installed as a separate top-level layer.
- **Finding isolation:** no repository-owned defect was found inside `registry_publish_policy.rs` itself. Its canonical route match, body bound, classification normalization, live auth gate, constant-time-independent permission check semantics, dry-run behavior, body reconstruction, and pass-through behavior match the current host/controller contract.
- **Confirmed deferred root cause — REGISTRY-GOV-02:** the accepted owner implementation plan states that `ModulePublishRequestCreateCommand` owns publish-request metadata validation, including ownership/trust metadata, but `SeaOrmModuleGovernanceService::create_publish_request` currently validates only that `ownership` and `trust_level` are non-empty strings. The database columns are plain text without a corresponding enum/check invariant. A non-HTTP owner caller could therefore persist an arbitrary ownership/trust classification even though the HTTP middleware accepts only the canonical vocabulary. The authoritative fix belongs in `crates/modules/rustok-modules/src/governance.rs`, not in this middleware iteration.
- **Permission review:** the middleware's `client_id.is_some() && session_id.is_nil()` rejection intentionally excludes both OAuth `authorization_code` delegated users and `client_credentials` service tokens. `AuthPrincipalKind` defines those combinations explicitly, while ADR 2026-04-19 requires live Registry V2 authority to be session-backed user bearer auth. No permission relaxation is appropriate here.
- **Immediate re-audit:** re-read the complete middleware after the source-level audit and rechecked request ownership/trust normalization, 256 KiB body bound, JSON decoding, content-length reconstruction, extension lookup, dry-run/live branching, direct-user/session gate, effective `modules:manage`, canonical pass-through, and response status/error contracts. Production code contains no runtime unwrap/expect/panic/todo or silent error suppression.
- **Adjacent-boundary re-audit:** app-router order proves `auth_context::resolve_optional` precedes the registry middleware; the controller revalidates schema/version, derives authenticated registry authority, requires non-nil `Idempotency-Key`, and delegates persistence to the owner; the owner create transaction binds actor/context, checks durable owner authority, persists request metadata, receipt, translation, and governance event atomically; no cross-tenant owner selection is introduced by this middleware.
- **Regression audit:** no code change was necessary, so no runtime behavior was altered. The audit specifically rejected speculative fixes to delegated OAuth authorization, private trust-level visibility, or owner-layer enum semantics from this primary module because those would either contradict accepted Registry V2 architecture or belong to the owner module.
- **Fresh second pass:** independently rechecked route exactness, body size, content type/header handling, auth-extension timing, direct-user versus OAuth principal classification, permission inheritance, dry-run/live divergence, third-party rejection, artifact-origin policy delegation, downstream JSON extraction, and owner atomicity. No remaining repository-owned in-scope defect was found in `registry_publish_policy.rs`.
- **Verification:** repository-content/static inspection and branch-diff review only. No tests, clippy, gatekeeper, build, or runtime commands were executed by the agent, per maintainer-owned verification rules.
- **Status:** module-level second pass clean; `FS-22.02.04` complete. Next planned primary module is `FS-22.02.05 — rate_limit.rs`.
- **Merged:** PR #4171, merge commit `ca7a569a58cb8ebde50efce51de74b2b9123215c`.

### FS-22.02.05 Iteration 1 — `rate_limit.rs`

- **Base:** refreshed `main` to `9074d5a7635e3660d294c8e8d6a55cbea688e41f` after concurrent work advanced `main`; the earlier branch based on `18b1d203...` was deliberately not merged.
- **Dedicated iteration branch:** `audit/fs-22.02.05-i1-refresh2-20260928`.
- **Invariant map:** rate limiting must fail closed when a configured Redis backend is unavailable; memory updates must remain atomically race-safe and bounded; distributed keys must remain opaque; client-IP resolution must obey the canonical proxy-trust policy; verified tenant/OAuth dimensions must never come from spoofable input; path policies must cover public expensive endpoints without changing unrelated routing behavior; response status/header contracts must remain stable; middleware ordering must preserve independent authenticated/tenant policy resolution; all host profiles must retain the required abuse boundary.
- **Confirmed finding RATE-22.02.05-01:** public `GET /health/ready` was outside the path-rate-limit policy even though it executes multiple database, cache, event-transport, search, outbox, worker, and module-health checks. The production observability middleware sanitizes its response but intentionally leaves aggregate readiness public. The route is now attached to the distributed `api` limiter, using the same backend/configuration across host profiles.
- **Confirmed finding RATE-22.02.05-02:** the memory backend logged the complete rate-limit identity, including IP/tenant/OAuth dimensions, while the Redis path was already opaque. The memory rejection warning and legacy middleware debug path now log only a stable 16-hex SHA-256 fingerprint.
- **Confirmed finding RATE-22.02.05-03:** the memory backend could calculate `Retry-After: 0` during the final sub-second of an active fixed window. The retry value is now clamped to at least one second; Redis already used the same minimum.
- **Confirmed finding RATE-22.02.05-04:** the limiter bearer parser accepted only exact `Bearer`/`bearer` prefixes while the normal HTTP authentication contract treats the auth scheme case-insensitively. Uppercase or mixed-case bearer schemes could therefore lose verified tenant/OAuth rate-limit dimensions and collapse requests onto the plain IP bucket. The parser now normalizes the scheme case-insensitively and rejects empty tokens.
- **Confirmed finding RATE-22.02.05-05:** source and guide documentation described the implementation as a sliding-window limiter, while both memory and Redis implementations are fixed-window counters. The implementation comments and rate-limit guide were reconciled, including the actual `services/app_runtime.rs` wiring path and public readiness coverage.
- **Adjacent-boundary review:** current router composition still installs the path limiter before authenticated tenant/context middleware and before registry guards, while health is always mounted. `metrics_auth` continues to expose only the readiness aggregate publicly and protects detailed runtime/module diagnostics in production. `RequestTrustSettings` remains the canonical source for forwarded-IP trust.
- **Backend/concurrency review:** memory retains the 100,000-entry cap, atomic Moka upsert, and idle expiry; Redis remains an atomic Lua `INCR/EXPIRE/TTL` operation with a two-second timeout and hashed storage key; backend failures remain HTTP 503/fail-closed. No silent backend or cleanup errors were introduced.
- **Regression audit:** no rate quota values, tenant isolation model, Redis namespace, registry lease behavior, or host-profile router contract was loosened. The remediation changes only rate-limit boundary coverage, diagnostics, bearer parsing, retry semantics, and related documentation/tests.
- **Fresh second pass:** re-read the complete changed rate-limit wrapper/base/tests, the actual `app_runtime` policy vector, current `app_router` and HTTP edge composition after concurrent main changes, health/observability behavior, request-trust implementation, settings defaults, and the rate-limit guide. No remaining repository-owned in-scope defect was found in `rate_limit.rs`.
- **Deferred finding SETTINGS-RATE-01:** `SettingsService::RateLimitSettingsValidator` validates a different JSON schema (`requests_per_second`/`burst_size`) than the live `RustokSettings.rate_limit` contract (`requests_per_minute`, `burst`, auth/oauth variants), while runtime bootstrap reads the host settings snapshot rather than these DB category overrides. Owning follow-up: settings/configuration integration boundary; not folded into this middleware iteration.
- **Verification:** repository-content inspection, static reasoning, and branch diff review only. Per maintainer execution rules, no tests, clippy, gatekeeper, build, or runtime commands were executed by the agent.
- **Status:** module-level fresh second pass clean; `FS-22.02.05` complete. Next planned primary module is `FS-22.02.06 — apps/server/src/middleware/auth_context.rs`.
- **Merged:** PR #4173, merge commit `84f7553d1e13a63aee1afaf7d03a50de81a07fd7`.

### FS-22.02.06 Iteration 1 — `auth_context.rs`

- **Base:** refreshed `main` to `2afdf813171dc6a596343770045433044b66ab0a`. Concurrent `main` changes were limited to the HTTP edge/CORS boundary and did not modify the auth-context contract; the stale pre-refresh branch was abandoned.
- **Dedicated iteration branch:** `audit/fs-22.02.06-i1-refresh2-20260928`.
- **Invariant map:** optional authentication must remain anonymous only when no credentials are presented; any presented user bearer must be validated and must never become authorization through client-supplied tenant/identity data; global/operator routes must not fabricate a tenant context; self-resolving transports must retain their own tenant contract; service principals must remain excluded from human-only routes; host-only credentials must remain separate and request-scoped; RBAC request snapshots must be isolated per request.
- **Confirmed finding AUTH-22.02.06-01:** `auth_context` invoked the canonical auth extractor for every authenticated request, but `resolve_current_user` required a `TenantContextExtension`. The tenant route policy intentionally skips tenant resolution for `GlobalOperator` routes and `SelfResolvingHandshake`, including registry endpoints. Presented bearer credentials therefore produced HTTP 500 before authenticated registry/global handlers could receive `AuthContextExtension`. The middleware now derives only the signed JWT `tenant_id` for those explicit non-tenant route scopes and delegates the complete principal/session/OAuth validation to the existing canonical token resolver. It does not create a fake `TenantContextExtension`.
- **Confirmed finding AUTH-22.02.06-02:** `metrics_auth` is intentionally the authentication boundary for observability bearer credentials, but the actual Axum layer order places `auth_context` outside it. A Prometheus/observability Bearer token could therefore be consumed as a user JWT and rejected before reaching `metrics_auth`. `auth_context` now passes the exact observability paths through untouched so `metrics_auth` remains authoritative for those credentials, including aggregate readiness.
- **Confirmed refinement AUTH-22.02.06-03:** tenantless JWT resolution is now gated by the canonical `tenant_route_policy` scope rather than merely by absence of a tenant extension. An accidental tenant-middleware omission on an otherwise tenant-bound route therefore remains fail-closed instead of silently falling back to JWT tenant claims.
- **Adjacent-boundary review:** app-router ordering confirms `tenant -> locale -> auth_context -> invite/channel/rate-limit` in the tenant-enabled profiles, while registry-only/worker-only profiles intentionally omit tenant resolution. Observability paths are globally mounted and separately protected/sanitized by `metrics_auth`. Host authority is removed before downstream dispatch and carried only as typed task-local context.
- **Auth-policy review:** direct users require active sessions; delegated users are restricted by OAuth app scopes/consent; service tokens are classified separately and remain forbidden on human-only/forum personal interaction paths. `AuthContextExtension` and `AuthPrincipalContextExtension` are populated only from the verified `CurrentUser` result.
- **Regression audit:** the remediation does not fabricate tenant state, relax session/user checks, broaden OAuth scopes, accept legacy registry headers, or bypass host authority. Observability tokens no longer collide with user JWT parsing, while normal tenant-bound requests still require the ordinary tenant middleware path.
- **Fresh second pass:** re-read the complete changed middleware, canonical auth extractor/principal classifier, host-authority scope, RBAC task-local scope, tenant route policy, metrics auth, application router ordering, auth/registry/health controllers, and current auth tests. No remaining repository-owned defect was found inside `auth_context.rs`.
- **Deferred finding AUTH-EXTRACTOR-01:** `apps/server/src/extractors/auth/mod.rs` validates tenant ownership after loading sessions/users by primary key, but the underlying session/user queries are not tenant-filtered at SQL level. The subsequent equality checks prevent cross-tenant authorization, but the query boundary does not itself carry the canonical tenant predicate required by the repository data-isolation standard. **Owning component:** `apps/server/src/extractors/auth/mod.rs`; handle as a dedicated extractor/auth iteration.
- **Verification:** repository-content inspection, static reasoning, and branch diff review only. Per maintainer execution rules, no tests, clippy, gatekeeper, build, or runtime commands were executed by the agent.
- **Status:** module-level fresh second pass clean; `FS-22.02.06` complete. Next planned primary module is `FS-22.02.07 — apps/server/src/middleware/channel.rs`.
- **Merged:** PR #4174, merge commit `284e9a4d860c8adc9d38548e9ec859961226bd76`.

### FS-22.02.07 Iteration 1 — `channel.rs`

- **Base:** refreshed `main` to `41c0386f1f99bdbdde570b824a217dfbdbcd11c4` immediately before starting this iteration.
- **Dedicated iteration branch:** `codex/audit-fs-22.02.07-channel-middleware`.
- **Invariant map:** channel resolution must consume the already-resolved tenant, trusted effective host, canonical effective locale, and verified OAuth app identity; explicit selector precedence must remain unchanged; cache identity must represent semantic resolution facts; request-derived selector data must remain bounded before database resolution or trace/cache retention; successful channel mutations must invalidate local and durable generations through the existing REST/native boundaries.
- **Confirmed finding CHANNEL-22.02.07-01:** `channel_slug_from_header` and `channel_slug_from_query` accepted arbitrarily long selector values even though the authoritative `channels.slug` storage contract is `string_len(100)`. These raw values reached owner resolution and, on fallback, could be copied into the resolution trace retained in `ChannelContext` and therefore into the weighted cache. This violates the module plan's bounded-request-facts invariant and creates avoidable DB/trace/cache amplification from a single request.
- **Confirmed finding CHANNEL-22.02.07-02:** the resolver canonicalizes web-domain hosts before host matching, but `channel_cache_key_from_facts` hashed the raw effective host. Equivalent hosts such as casing, an optional port, or a trailing dot therefore produced different cache identities while resolving to the same channel. This does not break tenant isolation, but it weakens cache efficiency and allows unnecessary cache churn within the bounded capacity.
- **Adjacent-boundary review:** normal tenant-enabled router order supplies tenant, locale, and auth context before channel resolution; the channel middleware uses the canonical request-trust host helper and owner `ChannelResolver`, whose host/default/policy queries remain tenant-scoped. REST channel mutations call the shared invalidation publisher directly, while native mutations are covered by `channel_native_wrapper`; durable generation remains database-owned.
- **Remediation:** bound channel selector values at the HTTP parsing boundary to the storage contract, use canonical URL query decoding, and canonicalize the host only for cache-key identity so resolution precedence and owner semantics remain unchanged.
- **Fresh second pass:** re-read the complete changed middleware, native wrapper, REST channel controller, ChannelResolver, ChannelReadPort, locale/tenant middleware, and the application-router contract. The remediation preserves explicit selector precedence, tenant scope, trusted host derivation, durable invalidation ownership, and fail-safe cache generation behavior; no additional repository-owned defect was found inside `channel.rs`.
- **Verification:** repository-content inspection, static reasoning, and branch diff review only. Per maintainer execution rules, no test suite, clippy, build, or runtime command was executed by the agent.
- **Status:** module-level fresh second pass clean; `FS-22.02.07` complete and integrated into `main`. Next planned primary module is `FS-22.02.08 — apps/server/src/middleware/locale.rs`.
- **Merged:** PR #4175, merge commit `545ee456246a0a34c9a8b7f6f2e9284ef213f197`.


### FS-22.02.08 Iteration 1 — `locale.rs`

- **Base:** refreshed `main` to `a5e48ba1d0cc0e581f188d9286d816da23d02e41` immediately before starting this iteration.
- **Dedicated iteration branch:** `codex/audit-fs-22.02.08-locale-middleware`.
- **Invariant map:** effective locale selection must preserve host precedence and canonical normalization; tenant policy is authoritative whenever a trusted tenant exists; disabled/unlisted requested locales must never escape the tenant allowlist; locale-policy cache fills must not resurrect data after an intervening invalidation; cache state must remain bounded and safe across generation rollover; locale middleware must remain a pure request-context provider and must not invent a second fallback chain.
- **Confirmed finding LOCALE-22.02.08-01:** `resolve_locale` only constrained the resolved locale when the cached tenant locale list was non-empty. The owner projection deliberately allows a legacy empty locale list to be represented even though writes require at least one locale. In that state, any normalized caller-requested locale passed through unchanged, bypassing the tenant policy and potentially contradicting the tenant default. The middleware now constrains every tenant-bound request; an empty policy falls back to the trusted tenant default (or platform `en` only if that default is malformed) instead of honoring the caller's requested locale.
- **Confirmed finding LOCALE-22.02.08-02:** `get_or_load` keyed the async fill only by tenant UUID. A tenant-locale mutation could invalidate the cache while a previous miss was still loading; the old fill could then insert after invalidation and repopulate stale policy for the remainder of the TTL. The cache now uses a bounded monotonic per-tenant version namespace, so every invalidation moves future requests to a new key; an in-flight fill can at worst populate an obsolete key that is never read again. Version exhaustion fails closed by bypassing the cache rather than reusing an old token.
- **Adjacent-boundary review:** locale runs after tenant resolution and before auth/channel; it consumes only the trusted tenant context and the owner `TenantLocalePolicyPort`, while the dedicated durable locale-generation listener performs cross-replica invalidation. No direct `tenant_locales` query exists in the middleware. Response `Content-Language` is derived from the same effective value inserted into request extensions.
- **Deferred adjacent finding LOCALE-API-01:** `rustok-api::request::extract_locale_from_query` parses the raw URI query without form URL-decoding, so percent-encoded locale selectors such as `locale=ru%2Dby` are rejected instead of canonicalized. Owning component: `crates/libs/rustok-api/src/request.rs`; keep as a dedicated request-locale iteration rather than duplicating query parsing here.
- **Deferred adjacent finding LOCALE-LEGACY-01:** `locale.rs` still inserts the legacy `rustok-core::i18n::Locale` enum after producing the richer `ResolvedRequestLocale`. The enum currently represents only `en/ru/es/de/fr/zh`, while the platform locale model now accepts a world-language Unicode surface. The correct fix is a separate owner-level migration of remaining legacy consumers to the canonical runtime/UI locale contracts, not silently changing middleware behavior in this iteration.
- **Remediation:** add the versioned cache namespace and bounded rollover; always enforce tenant policy, including the empty-policy case; add focused regression tests for invalidation-version rotation, fail-closed exhaustion, and empty-policy fallback.
- **Fresh second pass:** re-read the changed middleware, tenant locale generation listener, tenant policy owner, request locale primitives, application-router ordering, cache architecture guard, and current locale-generation guard. The remediation preserves query/cookie/header precedence, tenant ownership, durable invalidation semantics, and response/request locale parity; no additional repository-owned defect was found inside `locale.rs`.
- **Verification:** repository-content inspection, static reasoning, and branch diff review only; no tests, clippy, build, or runtime commands were executed by the agent, per maintainer-owned test policy.
- **Status:** module-level fresh second pass clean; `FS-22.02.08` complete and integrated into `main`. Next planned primary module is `FS-22.02.09 — apps/server/src/middleware/tenant.rs`.
- **Merged:** PR #4176, merge commit `a1ae5b507cb877522256b215920648f2a217ffef`.

### Deferred owning-module findings discovered during FS-22

- [ ] **REGISTRY-GOV-01 — owner transfer target liveness is checked only in host middleware, not at the authoritative owner transaction boundary.** `registry_artifact_access.rs` verifies an active/same-tenant target user before dispatch, but `SeaOrmModuleGovernanceService::transfer_owner` only validates the new owner principal shape before the owner-binding update. A concurrent user deletion/deactivation can therefore violate the active-user owner invariant. **Owning component:** `crates/modules/rustok-modules/src/governance.rs` owner-transfer transaction. Handle as a separate primary-module iteration; do not fold it into another server middleware pass.

### FS-22 Legacy Phase Index

The earlier FS-22.01 route-graph pass is retained as historical evidence and is already closed. The detailed module queue above is now the authoritative execution granularity for all remaining FS-22 work.

- [x] **FS-22.01 Route graph:** enumerate every server route family and fallback; prove which host modes expose which routes, detect accidental shadowing/overlap, and reconcile route documentation.

**FS-22.01 WIP audit record — route-graph pre-implementation pass**

- Base refreshed from `main` at `8034b3ecba98c6e84f598a734adb6e264ae50c2e` before branch creation.
- Dedicated phase branch: `audit/fs-22.01-route-graph-20260928`.
- Repository governance and user execution conditions re-read before implementation; `AGENTS.md` is canonical and lowercase `agents.md` is absent. Maintainer owns tests; no CI/test execution by the agent.
- Route inventory covers host/base controllers, optional owner-declared Axum providers, webhooks, embedded storefront/admin surfaces, and the generated optional-route composition path.
- **Root-cause finding:** default `apps/server` composition enables both `embed-admin` and `mod-commerce`. Commerce contributes explicit `/admin/*` routes, while `mount_application_shell()` used `Router::nest("/admin", admin_router)`. With Axum 0.8.9 this is an outer nested route conflicting with existing concrete `/admin/*` registrations and can panic during route composition. The fix must preserve Commerce `/admin/*` precedence while still serving the embedded Admin SPA for otherwise-unmatched `/admin...` paths.
- No other exact route-prefix collision was confirmed in the inspected optional HTTP providers; remaining FS-22.01 work is the remediation, direct/adjacent re-audit, fresh second pass, and static branch-diff review.

**FS-22.01 closeout**

- Remediation: `mount_application_shell` no longer registers an `/admin` nested wildcard. It installs a final fallback that delegates only unmatched `/admin` and `/admin/...` requests to the embedded Admin router after stripping the host prefix; explicit Commerce `/admin/*` routes remain authoritative.
- Runtime wiring: the ready Admin router receives a cloned `ServerAuthRuntime` via `with_state`, while the host composition keeps the existing auth runtime for the normal middleware chain.
- Immediate re-audit: changed imports, mount helper, fallback URI rewriting, Admin build call, and the regression case were re-read after the remediation. The self-review initially caught and corrected the missing `tower::ServiceExt` import.
- Adjacent-boundary audit: host route composition, generated optional-module Axum registration, default feature matrix, Commerce `/admin/*`, embedded storefront routes, observability/registry outer guards, and Admin asset fallback behavior were rechecked.
- Fresh second pass: repeated from the composition-root surface without relying on the original finding list; no additional route shadowing or newly introduced fallback bug was found in the inspected scope. `/adminfoo`-style paths are not delegated because the fallback requires an exact `/admin` boundary.
- Static verification: branch diff contains only `apps/server/src/services/app_router.rs` and this ledger; branch is based directly on `main` SHA `8034b3ecba98c6e84f598a734adb6e264ae50c2e`. No test suite or CI job was executed by the agent under the maintainer-owned test contract.

- [ ] **FS-22.02 Global middleware order:** trace the actual Axum layer nesting and request lifecycle; verify security headers, metrics auth, registry guards, rate limiting, auth context, channel, locale, tenant, and guest-access ordering against trust assumptions.
- [ ] **FS-22.03 Identity/auth propagation:** trace token parsing, principal construction, optional/required auth, session/refresh behavior, impersonation/agent paths, and transport boundary identity reconstruction.
- [ ] **FS-22.04 Tenant/channel/locale propagation:** follow context from HTTP headers/claims through middleware, GraphQL, REST, server functions, cache keys, DB access, and downstream module calls; specifically test conceptual cross-tenant/channel/locale leakage cases by code inspection.
- [ ] **FS-22.05 GraphQL composition:** trace schema construction, resolver registration, runtime data factories, error conversion, request context, authorization, limits, introspection/IDE exposure, and feature-flag/module interactions.
- [ ] **FS-22.06 REST/controller composition:** trace controller registration, shared state extraction, response envelopes, status mapping, body/multipart handling, and per-route authorization.
- [ ] **FS-22.07 Server-function composition:** inspect `/api/fn/*`, context provisioning, CSRF/browser trust assumptions, auth and tenant propagation, and error/serialization boundaries.
- [ ] **FS-22.08 Embedded UI composition:** trace admin/storefront mounting, asset fallback behavior, nonce/CSP interaction, cache validators, route precedence, and headless/embedded profile combinations.
- [ ] **FS-22.09 Feature/config interaction matrix:** inspect compile-time feature flags vs runtime module enablement/host modes and identify states that compile but produce incomplete or unsafe runtime composition.
- [ ] **FS-22.10 Error/observability boundary:** inspect server-wide error mapping and logging for secret, identity, tenant, raw domain-error, and stack/payload leakage; verify stable public contracts.
- [ ] **FS-22.11 Fresh second-pass composition audit:** after all FS-22 fixes, re-read the composition root from scratch without using the original findings list and record any newly discovered defects.

### Phase Order

| Phase | Scope | Audit focus | Status |
|---|---|---|:---:|
| FS-21 | Deployment/server/runtime boundary | processes, HTTP/TLS/proxy assumptions, runtime config, startup/shutdown, secrets, environment, fail-closed behavior, observability, resource limits | [x] |
| FS-22 | `apps/server` composition root | routing, middleware, request context, auth/session, tenant/channel/locale resolution, error mapping, GraphQL/REST/server functions, host composition | [ ] |
| FS-23 | Stable foundation/API crates | `rustok-api`, runtime/web/context contracts, dependency direction, shared types, transport/error contracts, accidental domain leakage | [ ] |
| FS-24 | Workers, jobs, queue, outbox | ownership, retries/idempotency, leases, concurrency, delivery guarantees, dead-letter paths, shutdown/recovery, telemetry | [ ] |
| FS-25 | Core platform modules | modules/control-plane, tenant, auth, RBAC, channel, cache, email, index/search/outbox/events, lifecycle/settings | [ ] |
| FS-26 | Commerce domain | cart, customer, product, relations, pricing, inventory, order, payment, fulfillment, orchestration, marketplace family | [ ] |
| FS-27 | Content/social domain | content, taxonomy, translation, profiles, social graph, reactions, groups, moderation, comments | [ ] |
| FS-28 | Publishing/community domain | blog, pages, forum, navigation, page-builder, SEO, notifications and cross-module projections | [ ] |
| FS-29 | Capability/extension modules | AI, MCP, Iggy/connectors, Alloy, Flex, repository connectors and external/provider seams | [ ] |
| FS-30 | Module-owned UI packages | all module `admin/`, `storefront/`, `next-admin/` packages; transport ownership, auth, locale, tenant and UI/data parity | [ ] |
| FS-31 | Leptos applications | `apps/admin`, `apps/storefront`; SSR/hydration, routing, server functions, browser trust, caching, i18n, forms and operator paths | [ ] |
| FS-32 | Next.js applications | `apps/next-admin`, `apps/next-frontend`; server/client boundaries, proxying, auth, GraphQL, SEO, caching, browser security and tenant propagation | [ ] |
| FS-33 | Shared frontend/browser packages | `packages/*`, UI cores, richtext, generated clients, shared state, URL/security helpers, duplicated semantics | [ ] |
| FS-34 | Storage/schema/migrations | all module migrations, entity/schema parity, cross-backend behavior, constraints, indexes, rollback/down paths, data-loss hazards | [ ] |
| FS-35 | Utilities/installer/build/release tooling | `crates/utils/*`, installer, source/publication/signing, CLI tooling, build scripts, deployment tooling and operator safety | [ ] |
| FS-36 | Shared libraries | `crates/libs/*`, UI foundations, common infrastructure and reusable abstractions; ownership, API stability, hidden coupling, dependency direction | [ ] |
| FS-37 | Dependency & supply-chain closure | Cargo/npm lockfiles, duplicate/unused dependencies, feature flags, unsafe/advisory surfaces, generated code provenance, licenses/policies | [ ] |
| FS-38 | Runtime/server closure pass | re-check server/runtime after all lower-layer changes; request lifecycle, controllers, server functions, file/WS surfaces, error mapping, blocking I/O, panic/resource hazards, auth/tenant propagation | [ ] |
| FS-39 | Final architecture reconciliation | dependency graph, boundary violations, dead/duplicate paths, stale docs/ADRs, generated artifacts, canonical vocabulary, remaining TODO/placeholder risk | [ ] |
| FS-40 | Release-readiness handoff | final ledger reconciliation, unresolved findings, maintainer test matrix, verification/evidence gaps, clean `main` baseline | [ ] |

### Definition of Done for Every Phase

- [ ] Every relevant production path and boundary in scope was inspected, not only obvious entrypoints.
- [ ] Business invariants and important failure states were checked/documented.
- [ ] Cross-tenant / cross-principal / cross-channel / cross-locale leakage risks were checked.
- [ ] Concurrency, retry, idempotency and transaction boundaries were checked where applicable.
- [ ] Persistence, migrations and rollback implications were checked where applicable.
- [ ] Every remediation unit received an immediate re-audit, adjacent-boundary re-audit, and fresh second pass.
- [ ] The final branch diff received a regression-focused review specifically looking for defects introduced by the fixes.
- [ ] All repository-owned defects found in scope were implemented on the phase branch or explicitly blocked by an owner decision/ADR.
- [ ] Tests were inspected but left for maintainer execution under the current contract.
- [ ] Static/source checks feasible without running test suites were performed where relevant.
- [ ] Phase result was committed, PR'd, merged to `main`, post-merge reconciled, and the ledger was updated afterward.
## Deep Full-Stack Audit Cycle — 2026-09-27

**Status:** COMPLETE  
**Active phase:** none — 2026-09-27 Deep Full-Stack Audit Cycle complete. The next `реализуй план аудита` invocation must create a new dated audit round.  
**Final main baseline before handoff merge:** `fa47d64d6faee9507891d0bca145b4672c39dfc9`

### FS-01 Pre-Implementation Audit Findings

- [x] **SERVER-01 — database URI secret exposure in startup logs.** `apps/server/src/host.rs::resolve_database_uri` logs the complete fallback database URI. The configured URI may contain credentials, violating the repository rule that secrets/credentials must never enter logs.
- [x] **SERVER-02 — effective-policy cache initialization is not atomic.** `ServerRuntimeContext::effective_policy_cache` performs check-then-insert on the shared TypeId map. Concurrent callers can construct distinct cache instances; one can be returned while another becomes the shared owner, creating divergent policy-cache state and invalidation behavior.
**Purpose:** perform a fresh, sequential, root-to-leaf audit of the entire repository. The previous ACRE component round remains historical evidence; its `100%` component status does **not** close this deeper cross-layer audit.

**Execution contract**
- Canonical trigger: `реализуй план аудита`
- Plan state: this section plus the phase table below are the temporary living plan for the current cycle.
- Progress state: maintained here and in this ledger entry; do not create a second audit checklist.
- Ordering: one phase at a time, strictly in order.
- Base: begin each phase from freshly refreshed `main`.
- Integration: phase branch → implementation/audit commit → PR to `main` → merge → refresh `main`.
- Tests: maintainer runs test suites. The agent does not run test suites unless this rule is explicitly changed.
- Completion: a phase is complete only after its repository-owned findings are fixed or explicitly recorded as blocked by an ADR/user decision, and the branch has been integrated into `main`.
- Quality bar: audit root causes and business invariants, not just syntax, lint, or pattern counts.

### Session Contract Read Record

- [x] `AGENTS.md` read.
- [x] `docs/index.md` read.
- [x] `ARCHITECTURE.md` read.
- [x] `docs/CONTINUOUS_CODE_REVIEW.md` read.
- [x] `docs/standards/continuous-review-ledger.md` read.
- [x] User audit contract recorded: full-stack/server-to-libraries scope, sequential execution, branch/commit/merge workflow, maintainer-owned tests, single trigger `реализуй план аудита`.
- [x] `agents.md` checked and confirmed absent; `AGENTS.md` is the canonical governance file.
- **Initial main SHA:** `3ba0ced2de0ad84a4e35a6a867ef1be6c350f05d`

### Phase Order

| Phase | Scope | Audit focus | Status |
|---|---|---|:---:|
| FS-00 | Governance & repository topology | manifests, workspace graph, ADR/Docs authority, generated surfaces, scripts, branch/CI conventions, auditability | [x] |
| FS-01 | Deployment/server/runtime boundary | process model, HTTP/TLS/proxy assumptions, runtime config, startup/shutdown, secrets, environment, fail-closed behavior, observability, resource limits | [x] |
| FS-02 | `apps/server` composition root | routing, middleware, request context, auth/session, tenant/channel/locale resolution, error mapping, GraphQL/REST/server functions, host composition | [x] |
| FS-03 | Stable foundation/API crates | `rustok-api`, runtime/web/context contracts, dependency direction, shared types, transport/error contracts, accidental domain leakage | [x] |
| FS-04 | Workers, jobs, queue, outbox | worker ownership, retries/idempotency, leases, concurrency, delivery guarantees, dead-letter paths, shutdown/recovery, telemetry | [x] |
| FS-05 | Core platform modules | modules/control-plane, tenant, auth, RBAC, channel, cache, email, index/search/outbox/events, lifecycle/settings | [x] |
| FS-06 | Commerce domain | cart, customer, product, relations, pricing, inventory, order, payment, fulfillment, commerce orchestration, marketplace family | [x] |
| FS-07 | Content/social domain | content, taxonomy, translation, profiles, social graph, reactions, groups, moderation, comments | [x] |
| FS-08 | Publishing/community domain | blog, pages, forum, navigation, page-builder, SEO, notifications and cross-module projections | [x] |
| FS-09 | Capability/extension modules | AI, MCP, Iggy/connectors, Alloy, Flex, repository connectors and external/provider seams | [x] |
| FS-10 | Module-owned UI packages | all module `admin/`, `storefront/`, `next-admin/` packages; transport ownership, auth, locale, tenant and UI/data parity | [x] |
| FS-11 | Leptos applications | `apps/admin`, `apps/storefront`; SSR/hydration, routing, server functions, browser trust, caching, i18n, forms and operator paths | [x] |
| FS-12 | Next.js applications | `apps/next-admin`, `apps/next-frontend`; server/client boundaries, proxying, auth, GraphQL, SEO, caching, browser security and tenant propagation | [x] |
| FS-13 | Shared frontend/browser packages | `packages/*`, UI cores, richtext, generated clients, shared state, URL/security helpers, duplicated semantics | [x] |
| FS-14 | Storage/schema/migrations | all module migrations, entity/schema parity, cross-backend behavior, constraints, indexes, rollback/down paths, data-loss hazards | [x] |
| FS-15 | Utilities/installer/build/release tooling | `crates/utils/*`, installer, source/publication/signing, CLI tooling, build scripts, deployment tooling and operator safety | [x] |
| FS-16 | Shared libraries | `crates/libs/*`, UI foundations, common infrastructure and reusable abstractions; ownership, API stability, hidden coupling, dependency direction | [x] |
| FS-17 | Dependency & supply-chain closure | Cargo/npm lockfiles, duplicate/unused dependencies, feature flags, unsafe/advisory surfaces, generated code provenance, licenses/policies where repository contracts require them | [x] |
| FS-18 | Runtime/server application | server runtime beyond composition: request lifecycle, controllers, server functions, body limits, file/WS surfaces, error mapping, blocking I/O, panic/resource hazards, auth/tenant context propagation | [x] |
| FS-19 | Final architecture reconciliation | dependency graph, boundary violations, dead/duplicate paths, stale docs/ADRs, generated artifacts, canonical vocabulary, remaining TODO/placeholder risk | [x] |
| FS-20 | Release-readiness handoff | final ledger reconciliation, unresolved findings, maintainer test matrix, verification commands/evidence gaps, clean main baseline | [x] |

### Definition of Done for Every Phase

- [ ] Every relevant production path and boundary has been inspected, not only obvious entrypoints.
- [ ] Business invariants and failure states are documented in the audit notes.
- [ ] Cross-tenant / cross-principal / cross-channel / cross-locale leakage risks are checked.
- [ ] Concurrency, retry, idempotency and transaction boundaries are checked where applicable.
- [ ] Persistence, migrations and rollback implications are checked where applicable.
- [ ] All repository-owned defects found in scope are implemented in the phase branch or explicitly blocked by an owner decision/ADR.
- [ ] Tests are inspected but left for maintainer execution unless the test-running rule is explicitly changed.
- [ ] Ledger status and evidence are updated before integration.

---

# Continuous Code Review & Remediation Ledger (ACRE)

Tracking persistent progress across cyclical review rounds for all modules in RusToK.

## Current Cycle Status
- **Active Round:** Round 1
- **Cycle Started:** `2026-09-18T18:10:03Z`
- **Progress:** `212 / 212` components audited (**100%**)
- **Total Workspace Codebase:** `1,853,842` LOC across `212` modules/apps

---


## 2026-09-25 Commerce Store shared HTTP error-envelope hardening

The mounted Store shared HTTP mapper exposed dynamic StoreContext validation/currency
details and logged full owner errors plus tenant/user/channel identifiers. It now emits
stable public messages and bounded diagnostics: typed error kind, code/status, identity
presence/shape, and channel presence/length only. The shared storefront safety verifier
now requires the bounded contract and rejects the former raw payload patterns.

Maintainer runtime evidence, Cargo build, tests, and verifier execution remain unrun.

## 2026-09-25 Commerce Mounted Store Product runtime capability integrity

`products.rs` is intentionally retained as the compiled legacy Product compatibility source;
the mounted `/store/products` handler lives in `products_owner_list.rs` and already called
`runtime.product_storefront_http_read_port()`. The Commerce runtime was missing that delegation,
leaving an existing owner-read path without its host accessor. The missing delegation is now
present, and the owner-read verifier explicitly requires it. The legacy source was restored
exactly to its pre-audit state; no compatibility behavior was changed.

Maintainer runtime evidence, Cargo build, tests, and verifier execution remain unrun.

## 2026-09-25 Commerce Mounted Admin Order Detail owner-port cutover

The mounted `show_order` endpoint was still constructing `PaymentService` and
`FulfillmentService` directly even though `CommerceHttpRuntime` already mounted the
canonical `PaymentOrderReadPort` and `FulfillmentReadPort` capabilities. The route now
uses those owner ports, preserving optional collection/fulfillment reads, and the old
domain-error mappers were removed after becoming orphaned. The dedicated source verifier
now requires the typed owner-port handoffs and rejects direct foreign service construction.

Maintainer runtime evidence, Cargo build, tests, and verifier execution remain unrun.

## 2026-09-25 Commerce Order owner-port diagnostic hardening

The Order owner port still emitted complete database/core `Debug` payloads, raw tenant IDs,
validation text, and lifecycle transition values from checkout identity/completion paths.
Those diagnostics are now centralized as bounded context and error-shape facts; the public
PortError envelopes remain stable. The broad ecommerce verifier and canonical fixture now
require the bounded Order contract and explicitly reject the removed raw patterns.

Maintainer runtime evidence, Cargo build, tests, and verifier execution remain unrun.

## 2026-09-25 Commerce Admin shared HTTP error-envelope hardening

The shared Commerce admin HTTP helper used to accept a generic Debug error and log
`error = ?error`, and the post-order owner-port branch repeated the same full PortError
payload. Both paths now retain only stable owner/error-kind/public-code/status facts while
the public HTTP envelope remains unchanged. The existing admin order/fulfillment verifier
now requires the bounded helper contract and explicitly rejects the removed generic raw
error logging.

Maintainer runtime evidence, Cargo build, tests, and verifier execution remain unrun.

## 2026-09-25 Commerce Tax validation detail hardening

Tax request validation contained one dynamic public detail (`duplicate tax country rule for
{country_code}`), and its helper accepted arbitrary Display values, making future validation
message leakage easy to introduce. The helper now takes only static detail; the duplicate
country-rule path emits a stable message. The public-port verifier and regression fixture
explicitly reject both the old format and the Display-based helper signature.

Maintainer runtime evidence, Cargo build, tests, and verifier execution remain unrun.

## 2026-09-25 Commerce Tax port error-envelope hardening

The legacy TaxCalculationPort mapper could copy TaxError::Validation text into the
public PortError message, while the canonical in-process adapter logged the full
PortError debug payload. Validation now maps to a stable public message, and the
canonical adapter logs only bounded context/error-shape facts. The ecommerce public
port verifier and its regression fixture now explicitly cover this Tax boundary.

Maintainer runtime evidence, Cargo build, tests, and verifier execution remain unrun.

## 2026-09-25 Commerce Fulfillment lifecycle status typing

The Fulfillment owner command adapter had critical lifecycle checks expressed as raw
status strings (ship admission, reship replay/transition, cancel replay/transition).
Commerce fulfillment orchestration and its compatibility facade also compared response
statuses directly. These checks now use the canonical FulfillmentStatusKind view, whose
unknown value remains fail-closed. The ecommerce lifecycle verifier now covers these
surfaces and rejects their former raw comparisons.

Maintainer runtime evidence, Cargo build, tests, and verifier execution remain unrun.

## 2026-09-25 Commerce Cart Checkout diagnostic hardening

Cart Checkout owner-boundary logging previously retained full PortError debug payloads and
raw request context such as tenant ID, actor, channel, locale, traceparent, and idempotency
key. The checkout snapshot port now emits only bounded correlation/context facts plus typed
error-shape metadata; the service mapper similarly records variant/field-shape facts without
raw values. Tenant parse and validation/database diagnostics follow the same contract. The
broad ecommerce verifier now isolates these mapper sections and forbids the raw diagnostic
patterns so this boundary cannot regress silently.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun in this continuation.

## 2026-09-25 Commerce GraphQL validation envelope hardening

The Commerce GraphQL checkout compatibility boundary previously copied raw validation details from `CommerceError` and owner `PortError` into public GraphQL messages. Both validation branches now emit stable public text while retaining only bounded diagnostic facts internally, and the existing checkout source verifier explicitly forbids the raw-message forwarding patterns.

## 2026-09-25 Commerce source-integrity blockers

A fresh Commerce continuity audit found hard source blockers on `main`: the mounted GraphQL runtime initializer was missing field separators in `CommerceGraphqlRuntimeData`, and the Order post-order static verifier contained adjacent string literals without commas. Both were corrected atomically. The fulfillment typed-identity migration also contained unsafe PostgreSQL legacy-index casts: malformed or oversized numeric metadata could be evaluated as a direct integer cast instead of failing closed. The migration now length-bounds the backfill before casting and uses the same guarded cast in the validation constraint. The same migration also removed checkout identity fields from `fulfillment_items.metadata` without restoring them in `down`; rollback now reconstructs those three fields from the typed fulfillment owner row on PostgreSQL, SQLite, and MySQL, so down/reapply does not silently discard the legacy item identity. The forward cleanup is now equally conservative: `fulfillment_items` identity metadata is removed only when operation/index/plan-hash all match the successfully migrated parent fulfillment, so orphaned or conflicting legacy item data is retained rather than destroyed. Rust compilation, static verifier execution, migrations, and tests remain unrun in this continuation; maintainer verification remains outstanding.

## 2026-09-24 Media public URL owner hardening

A fresh Forum→Media boundary audit found that `MediaItem.public_url` previously fell back to `blob.object_key` when no public storage base URL was configured. That made an internal storage key look like a consumer-visible delivery URL and was inconsistent with the Media public-image capability contract.

Media now centralizes `MediaItem` construction in one owner helper, never uses an object key as `public_url`, emits the checksum-bound Media public-image capability URL for ready image assets when no direct public base exists, and leaves non-image `public_url` empty until Media owns a matching delivery capability. Rendition public URLs also no longer fall back to object keys. Regression coverage and the Media public-image static verifier enforce the boundary.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.

## 2026-09-24 Forum topic-list metadata batch hardening

A read-path audit found a per-topic Flex attached-metadata lookup inside Forum topic-list hydration. The page itself was bounded, but custom-field resolution issued one database read per topic, creating an avoidable N+1 query pattern.

Forum topic hydration now resolves the page's topic metadata through Flex's existing bounded attached-translation storage batch (maximum 200 entities), while singleton attached-payload resolution delegates to the same canonical implementation. Locale precedence and shared-versus-localized metadata semantics therefore have one owner implementation. A regression test covers exact locale/fallback behavior, and the Forum read-model verifier fails if per-topic metadata resolution is reintroduced.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.

## 2026-09-24 Forum mention Profiles batch hardening

Forum mention resolution previously performed one Profiles owner lookup per resolved handle. With the 32-target mention bound, a single revision could therefore create an avoidable per-handle database burst.

Profiles now exposes a tenant-scoped bounded handle batch reader that reuses its existing batched translations and tag loading. Forum mention resolution calls that boundary once, then validates each returned ProfileRecord against the existing tenant, handle, active-status and visibility contract. Missing handles remain the established field-free mention-target failure. The mention integration verifier forbids the old per-handle reader call.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.

## 2026-09-24 Forum Reply owner consolidation

The Forum Reply owner still exposed a raw persistence service through `Deref`, while `bounded_compat.rs` added read methods outside the owner. That split the canonical Reply contract across an implicit dereference boundary and a compatibility module.

Reply read operations are now explicit methods on `reply_owner::ReplyService`, including the bounded pagination contract. The owner no longer implements `Deref`; the obsolete `bounded_compat.rs` module is removed. The public `reply_facade::ReplyService` surface remains unchanged.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.
Topic owner now also exposes its read/list compatibility methods explicitly, removing its `Deref` boundary without changing the public Topic facade. Raw `topic` and `reply` modules remain private persistence implementations and are accessed only explicitly by their owner modules.

## 2026-09-24 Forum canonical owner consolidation

A continuous-review audit found two Forum service owners still embedding explicit legacy implementations: the read-model owner delegated topic/reply projections through `read_model_legacy`, while the moderation owner delegated topic pin/status operations through `moderation_legacy` via `Deref`. That left two competing implementation authorities inside the module and made the canonical owner boundary misleading.

Forum now contains the canonical category/topic/reply read-model implementation directly in `read_model_owner.rs`, and the canonical moderation owner directly owns pin/unpin plus close/reopen/archive lifecycle operations. The legacy source files, redundant public moderation forwarding layer, and stale module bindings were removed; the public `read_model` and `moderation` module paths remain unchanged.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.

## 2026-09-24 Profiles handle batch bound hardening

The Forum mention batch reader initially delegated its safety to the Forum's 32-target mention limit. That was too weak as a public ProfilesReader contract because other callers could submit an unbounded handle array.

Profiles now enforces its own 64-handle batch limit before any database work. The owner still performs one tenant-scoped profile read plus the existing batched translations/tag resolution, and Forum remains responsible for completeness and mention visibility validation. Integration verification now requires the owner-level limit.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.

## 2026-09-25 Commerce post-order mutation idempotency

The Order owner now durably admits post-order change/return mutations through `order_command_receipts`, commits receipts atomically with the mutation, replays stored responses for the same caller identity, and classifies payload conflicts/corrupt receipts as typed failures. Mounted admin REST write routes now require a caller-owned `Idempotency-Key`; generated per-request keys were removed from the covered routes. Repository Rust compilation, targeted tests, migration replay, contention, and static verification remain unrun in this continuation.

## 2026-09-25 Commerce mounted owner-runtime composition

A mounted Commerce GraphQL composition audit found that mandatory Payment owner wrappers could be silently synthesized inside the schema factory when host composition was incomplete. Commerce now requires the host-composed Payment provider registry, read runtime, and command runtime; the server composition layer explicitly assembles and preserves those wrapper runtimes. Fulfillment remains an optional Commerce capability and retains its separate owner-owned fallback path. Obsolete duplicate order-change and order-return REST implementations were removed so each route has one canonical adapter path.

Repository Rust compilation, the Commerce/server test suites, and the static verifier remain unrun in this environment; no successful build or test result is claimed.

## Components Review Status

| Status | Component | Category | Files | LOC | Last Audited | Notes |
|:---:|---|---|---:|---:|---|---|
| [x] | [admin](../../apps/admin) | `apps` | 134 | 22,234 | 2026-09-21 07:56 | Audited SSR/native GraphQL proxy boundaries, auth snapshot/bootstrap, tenant header handling, Page Builder verification, ProtectedRoute coverage, and standalone security middleware/CSP/HSTS; no additional production authorization or tenant-isolation finding. |
| [x] | [next-admin](../../apps/next-admin) | `apps` | 148 | 13,845 | 2026-09-22 19:15 | Continued cross-module settings audit: fixed Forum module settings UI to persist the canonical `use_reactions` key consumed by the Forum manifest and backend, so the admin switch now controls the documented setting instead of writing an ignored camelCase field. |
| [x] | [blog](../../apps/next-admin/packages/blog) | `apps` | 17 | 1,777 | 2026-09-21 09:06 | Audited blog admin API/UI, tenant/token forwarding, server-side GraphQL authorization, public-channel visibility, locale handling, rich-text rendering, and backend post version CAS for update/delete/publish transitions; no additional production finding. | Also fixed an unbounded Blog Category settings payload: settings are now object-only and capped at 64 KiB, with persisted-state validation on reads.
| [x] | [commerce](../../apps/next-admin/packages/commerce) | `apps` | 7 | 2,309 | 2026-09-21 09:13 | Audited commerce admin GraphQL API and all four templates: shipping profiles, cart promotions, order changes, and return decisions. Verified token/tenant fail-closed checks, backend permission/tenant boundaries, order/cart ownership guards, and stateful mutation transport. No additional production finding. |
| [x] | [email](../../apps/next-admin/packages/email) | `apps` | 5 | 289 | 2026-09-21 09:20 | Audited email settings API/form/page plus server platform-settings authorization and tenant scoping. Found and fixed plaintext SMTP credential exposure through generic settings GraphQL; email secrets are now redacted, while blank password updates preserve the existing secret. |
| [x] | [rbac](../../apps/next-admin/packages/rbac) | `apps` | 5 | 211 | 2026-09-21 09:24 | Audited role list/assignment API and UI plus Rust RBAC GraphQL/owner writer: direct control-plane principal, tenant-bound actor/target, users:manage permission, hierarchy ceiling, last-active-SuperAdmin continuity, and transactional persistence are enforced server-side. |
| [x] | [rustok-ai](../../apps/next-admin/packages/rustok-ai) | `apps` | 1 | 4,055 | 2026-09-21 09:30 | Audited AI control-plane UI GraphQL/WebSocket flows, provider/task/tool/session/approval wiring, tenant forwarding, credential references, and browser-side action controls; backend remains authoritative for tenant scope, permissions, and SecretRef policy. No additional production finding. |
| [x] | [rustok-mcp](../../apps/next-admin/packages/rustok-mcp) | `apps` | 1 | 1,041 | 2026-09-21 09:42 | Audited MCP control-plane UI and backend token/client management, tool authorization, delegated-user authority, tenant scoping, audit events, one-time token exposure, and scaffold workspace confinement; fixed concurrent token rotation/deactivation by locking the tenant-scoped client row inside the transaction. |
| [x] | [rustok-product](../../apps/next-admin/packages/rustok-product) | `apps` | 1 | 340 | 2026-09-21 09:48 | Audited the read-only product admin GraphQL adapter: tenantId/token/tenantSlug fail-closed checks, product/variant/price/inventory projections, locale/category/attribute queries, and executor forwarding. No additional production finding. |
| [x] | [search](../../apps/next-admin/packages/search) | `apps` | 1 | 2,913 | 2026-09-21 09:56 | Audited search control-plane UI plus Rust GraphQL reads/writes: tenant scope resolution, settings/dictionary/rebuild permissions, storefront/admin rate limits, search preview, analytics and projection diagnostics. No additional production security finding. |
| [x] | [translation](../../apps/next-admin/packages/translation) | `apps` | 4 | 5,421 | 2026-09-21 10:14 | Audited translation admin dispatcher/types and Rust GraphQL/domain boundaries for policy, glossary, memory, workflow, interchange and machine operations; verified tenant-bound PortContext authorization, permission scopes, idempotency receipts, actor binding and revision CAS/transactional writes. No additional production finding. |
| [x] | [workflow](../../apps/next-admin/packages/workflow) | `apps` | 10 | 1,393 | 2026-09-21 11:02 | Audited workflow admin API/UI and Rust GraphQL/service/engine paths: tenant+permission boundaries, workflow/step CRUD, execution history, manual triggers, templates, webhook routing, version snapshots and restore. Fixed concurrent update/restore/step races by locking the parent workflow row and keeping version/restore writes transactional. |
| [x] | [next-frontend](../../apps/next-frontend) | `apps` | 42 | 3,668 | 2026-09-21 11:15 | Audited root storefront SSR/i18n/module wiring, GraphQL transport and tenant propagation, SEO REST/GraphQL fallback, runtime robots/sitemap fetching, rich-text routes, and Next proxy/middleware. Fixed SSRF risk by restricting runtime SEO document fetches to the configured API origin and rejecting redirects. |
| [x] | [rustok-blog](../../apps/next-frontend/packages/rustok-blog) | `apps` | 5 | 368 | 2026-09-21 11:22 | Audited public blog queries/comments UI, tenantId+tenantSlug forwarding, published/channel visibility, auth-gated comment mutation, public-comment degradation/cache indicators, locale fallback and rich-text rendering. Backend confirmed current-tenant enforcement and comment permission checks. No additional production finding. |
| [x] | [rustok-comments](../../apps/next-frontend/packages/rustok-comments) | `apps` | 2 | 111 | 2026-09-21 11:28 | Audited comment composer/index: presentation-only, no direct API/token/tenant access; submission delegates to the parent callback, whose blog mutation path is tenant/auth/permission-bound. No additional production finding. |
| [x] | [rustok-product](../../apps/next-frontend/packages/rustok-product) | `apps` | 1 | 70 | 2026-09-21 11:31 | Audited storefront product catalog option query: read-only GraphQL transport, locale validation, tenant slug forwarding and optional auth token; no direct mutation or secret access. |
| [x] | [search](../../apps/next-frontend/packages/search) | `apps` | 1 | 681 | 2026-09-21 11:37 | Audited storefront search UI transport, suggestions, presets, catalog filters and tenant/token forwarding; no direct API or mutation boundary, with server-side search remaining authoritative for tenant/channel filtering. |
| [x] | [server](../../apps/server) | `apps` | 477 | 177,052 | 2026-09-24 15:01 | Continued host-composition review with Blog/Comments: synchronized the server Blog dependency contract with the active Taxonomy/Outbox/Channel boundary and preserved Profiles as optional presentation enrichment through `rustok-profiles-api`; corrected a stale `Blog -> Profiles` hard-dependency assertion. Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent. |
| [x] | [storefront](../../apps/storefront) | `apps` | 34 | 5,059 | 2026-09-21 09:42 | Audited Leptos SSR/CSR routing, native server functions, GraphQL transport forwarding, tenant/channel/locale context, module composition and authenticated Pages authoring surface; fixed arbitrary tenant selection in public canonical-route and SEO server functions by binding them to the configured host tenant. |
| [x] | [rustok-api](../../crates/libs/rustok-api) | `libs` | 35 | 6,627 | 2026-09-22 16:35 | Added the typed `StaticModuleSettingsReader` contract plus a runtime-only transaction-aware companion, both lifecycle-aware and owner-composed; consumer modules receive settings without reading `tenant_modules` persistence directly. Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent. |
| [x] | [rustok-core](../../crates/libs/rustok-core) | `libs` | 64 | 16,913 | 2026-09-19 05:57 | Eliminated Tier 0 unwraps/panics with documented invariants, converted DatabaseHealthCheck to typed std::error::Error trait, fixed bulkhead doc print |
| [x] | [rustok-events](../../crates/libs/rustok-events) | `libs` | 27 | 10,971 | 2026-09-19 06:10 | Audited schema.rs: documented structural JSON serialization invariants for root events, envelopes, contracts, and digests |
| [x] | [rustok-fba](../../crates/libs/rustok-fba) | `libs` | 1 | 82 | 2026-09-18 18:10 | Verified clean, 1 file, 82 LOC |
| [x] | [rustok-runtime](../../crates/libs/rustok-runtime) | `libs` | 4 | 2,424 | 2026-09-19 06:20 | Audited deployment.rs and layout.rs: replaced expect calls with typed error propagation through Receipt and InvalidMarker variants |
| [x] | [rustok-telemetry](../../crates/libs/rustok-telemetry) | `libs` | 15 | 3,121 | 2026-09-19 06:40 | Centralized 100+ Prometheus metric declarations through typed factory helpers with documented compile-time invariants |
| [x] | [rustok-web](../../crates/libs/rustok-web) | `libs` | 2 | 742 | 2026-09-19 06:50 | Audited lib.rs and browser_assets.rs: eliminated panics and expects in response builders and hex formatting |
| [x] | [alloy](../../crates/modules/alloy) | `modules` | 70 | 24,622 | 2026-09-19 16:15 | Audited memory.rs, sea_orm.rs, runner.rs: eliminated Tier 0 bare expects with typed error propagation and documented concurrency invariants, replaced untyped String cron errors with ScriptError::InvalidTrigger, verified multi-tenancy scoping |
| [x] | [flex](../../crates/modules/flex) | `modules` | 51 | 16,885 | 2026-09-19 16:25 | Audited translation targets and mutation.rs: replaced expects with typed PortErrors in identity builders, documented static contract invariants in descriptor values, enriched GraphQL doc comments |
| [x] | [rustok-ai](../../crates/modules/rustok-ai) | `modules` | 72 | 42,257 | 2026-09-20 07:10 | Audited inference.rs, direct_order_tasks.rs, mcp.rs, policy.rs, metrics.rs, service/types.rs, graphql/types.rs: eliminated Tier 0 bare expects and panics with typed AiError propagation and graceful fallback, removed dead-code suppressions and made lineage public, verified multi-tenancy scoping |
| [x] | [rustok-ai-alloy](../../crates/modules/rustok-ai-alloy) | `modules` | 1 | 384 | 2026-09-20 07:33 | Verified clean, 1 file, 384 LOC, all 7 tests passed |
| [x] | [rustok-ai-athanor](../../crates/modules/rustok-ai-athanor) | `modules` | 2 | 546 | 2026-09-20 07:33 | Verified clean adapter architecture under athanor feature gate, all integration tests passed |
| [x] | [rustok-ai-content](../../crates/modules/rustok-ai-content) | `modules` | 1 | 326 | 2026-09-20 07:33 | Verified clean, 1 file, 326 LOC, all 10 tests passed |
| [x] | [rustok-ai-media](../../crates/modules/rustok-ai-media) | `modules` | 1 | 71 | 2026-09-20 07:33 | Verified clean, 1 file, 71 LOC, all tests passed |
| [x] | [rustok-ai-order](../../crates/modules/rustok-ai-order) | `modules` | 1 | 255 | 2026-09-20 07:33 | Verified clean, 1 file, 255 LOC, all 9 tests passed |
| [x] | [rustok-ai-product](../../crates/modules/rustok-ai-product) | `modules` | 1 | 307 | 2026-09-20 07:33 | Verified clean, 1 file, 307 LOC, all 6 tests passed |
| [x] | [rustok-ai-translation](../../crates/modules/rustok-ai-translation) | `modules` | 1 | 1,657 | 2026-09-20 07:33 | Audited lib.rs: eliminated Tier 0 bare expects on schema serialization and manifest hashing with safe fallbacks, all 18 tests passed |
| [x] | [admin](../../crates/modules/rustok-ai/admin) | `modules` | 17 | 8,440 | 2026-09-20 07:10 | Audited native_server_adapter.rs, transport/mod.rs, ui/leptos.rs: removed dead_code suppression on cancel_run and re-exported in transport facade, safely handled optional browser window/location in WebSocket URL builder |
| [x] | [rustok-auth](../../crates/modules/rustok-auth) | `modules` | 50 | 11,107 | 2026-09-20 07:44 | Audited config.rs: eliminated Tier 0 unwrap in RS256 key pair validation via pattern matching, all 36 tests passed |
| [x] | [admin](../../crates/modules/rustok-auth/admin) | `modules` | 18 | 5,550 | 2026-09-20 07:44 | Audited native_server_adapter.rs: removed allow(dead_code) suppression on change-password REST response status and validated payload, all 16 tests passed |
| [x] | [cli](../../crates/modules/rustok-auth/cli) | `modules` | 1 | 333 | 2026-09-20 07:44 | Verified clean CLI module, 1 file, 333 LOC, all 4 tests passed |
| [x] | [rustok-blog](../../crates/modules/rustok-blog) | `modules` | 126 | 29,074 | 2026-09-24 18:42 | Continued fresh Blog/Comments saga audit: the previous generic post-create compensation was itself caller-authorization dependent. Storefront customers can create comments while delete remains owner-scoped, so the compensating delete could be rejected. Blog compensation now uses a trusted `SecurityContext::system()` through the existing Comments owner port with a fresh idempotency key; the original create retains the user audit/event identity. Added Rust regression coverage plus source verifier/evidence self-test enforcement. Runtime, gatekeeper, build, and tests remain unrun by the agent. |
| [x] | [admin](../../crates/modules/rustok-blog/admin) | `modules` | 16 | 4,914 | 2026-09-23 03:08 | Continued Blog admin surface review: removed the runtime SEO target `expect`, made invalid built-in SEO configuration fail closed with localized copy, aligned GraphQL/native archive semantics, preserved missing-post moderation errors, and removed all production `allow`/`expect`/`unwrap`/`panic!`/`todo!` findings in the package; remaining `expect` uses are test-only. Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent. |
| [x] | [storefront](../../crates/modules/rustok-blog/storefront) | `modules` | 12 | 2,601 | 2026-09-24 18:45 | Fresh Blog storefront tenant-boundary audit found that the native `blog/storefront-data` endpoint could resolve an arbitrary public tenant when `TenantContext` was absent. The fallback now must exactly match the configured host tenant, matching the canonical Pages/canonical-route/SEO host-selection contract; the existing storefront verifier and self-test enforce the guard. Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent. |
| [x] | [rustok-brand](../../crates/modules/rustok-brand) | `modules` | 22 | 3,111 | 2026-09-20 09:12 | Audited brand domain, schema, translations, zero Tier 0 findings, all 5 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-brand/admin) | `modules` | 9 | 1,806 | 2026-09-20 09:12 | Audited brand admin UI & transport, zero suppressions/unwraps, all 3 tests passed, clippy clean |
| [x] | [rustok-cache](../../crates/modules/rustok-cache) | `modules` | 37 | 14,548 | 2026-09-20 09:23 | Audited cache service, dedupe, lease & status: eliminated bare expect in key canonicalization, documented dedupe invariant, removed dead_code suppressions via cfg gates, all 189 tests passed, clippy clean |
| [x] | [rustok-cart](../../crates/modules/rustok-cart) | `modules` | 72 | 16,524 | 2026-09-20 09:45 | Audited guarded_ports: enforced fail-closed guest token claim verification for guest carts, zero suppressions/unwraps, all 57 tests passed, clippy clean |
| [x] | [storefront](../../crates/modules/rustok-cart/storefront) | `modules` | 18 | 3,643 | 2026-09-20 09:45 | Audited storefront transport & core: zero suppressions/unwraps, all 16 tests passed, clippy clean |
| [x] | [rustok-channel](../../crates/modules/rustok-channel) | `modules` | 47 | 11,238 | 2026-09-20 10:05 | Added SQLite schema support in migrations 10-12, zero suppressions/unwraps, all 33 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-channel/admin) | `modules` | 14 | 5,226 | 2026-09-20 10:05 | Eliminated allow(too_many_arguments) with typed PolicyRuleFormSignals struct, all 7 tests passed, clippy clean |
| [x] | [rustok-comments](../../crates/modules/rustok-comments) | `modules` | 38 | 8,889 | 2026-09-24 15:25 | Hardened the neutral Comments contract and owner provider: all seven port operations remain mandatory, durable write idempotency now binds receipts to the authenticated PortActor as well as tenant/owner/operation/key/request, and replay cannot cross principals inside one tenant. Host capability absence remains the only degradation boundary. Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent. |
| [x] | [rustok-comments-storefront-support](../../crates/modules/rustok-comments-storefront-support) | `modules` | 3 | 171 | 2026-09-20 10:28 | Zero suppressions/unwraps, all 3 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-comments/admin) | `modules` | 8 | 1,379 | 2026-09-20 10:28 | Updated native server adapter callers to typed filters, English boundary test markers, all 13 tests passed, boundary script verified, clippy clean |
| [x] | [rustok-commerce](../../crates/modules/rustok-commerce) | `modules` | 295 | 123,544 | 2026-09-20 13:25 | Deleted dead legacy complete_storefront_checkout with allow(dead_code), documented invariants in collection_translation and fulfillment_orchestration, all 148 unit + 80 parity + integration tests passed, clippy clean |
| [x] | [rustok-commerce-foundation](../../crates/modules/rustok-commerce-foundation) | `modules` | 20 | 1,044 | 2026-09-20 10:35 | Zero suppressions/unwraps/panics, all 3 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-commerce/admin) | `modules` | 21 | 5,825 | 2026-09-20 10:38 | Eliminated allow(dead_code), removed dead request_context_fields, all 15 tests passed, boundary script verified, clippy clean |
| [x] | [storefront](../../crates/modules/rustok-commerce/storefront) | `modules` | 16 | 2,023 | 2026-09-20 10:38 | Eliminated allow(dead_code) in shared_adapter, transport, and requests, all 7 tests passed, error-safety & handoff scripts verified, clippy clean |
| [x] | [rustok-content](../../crates/modules/rustok-content) | `modules` | 48 | 7,003 | 2026-09-20 13:45 | Added test-transport-fallback to dev-dependencies, documented richtext AST render and profile invariants, all 75 tests passed, clippy clean |
| [x] | [rustok-content-orchestration](../../crates/modules/rustok-content-orchestration) | `modules` | 2 | 2,467 | 2026-09-20 13:45 | Verified bridge wiring and invariants, zero suppressions/unwraps, all tests passed, clippy clean |
| [x] | [rustok-customer](../../crates/modules/rustok-customer) | `modules` | 26 | 5,022 | 2026-09-20 13:55 | Normalized input email before validation in create/update, typed customer.tenant_id_invalid port error code, all 16 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-customer/admin) | `modules` | 9 | 2,476 | 2026-09-20 13:55 | Replaced allow(clippy::too_many_arguments) with typed CustomerFormSignals, all 8 tests passed, clippy clean |
| [x] | [rustok-distribution](../../crates/modules/rustok-distribution) | `modules` | 34 | 15,734 | 2026-09-20 14:12 | Eliminated allow(unused_mut) via immutable shadowing in build_registry, replaced allow(too_many_arguments) with StorefrontBudgetedExecutionParams, all tests passed, clippy clean |
| [x] | [rustok-email](../../crates/modules/rustok-email) | `modules` | 6 | 702 | 2026-09-20 14:16 | Verified email module & ports, zero suppressions/unwraps, all 8 tests passed, clippy clean |
| [x] | [rustok-events-module](../../crates/modules/rustok-events-module) | `modules` | 14 | 847 | 2026-09-20 14:21 | Verified events runtime module adapter, zero suppressions/unwraps, test passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-events-module/admin) | `modules` | 7 | 506 | 2026-09-20 14:21 | Verified leptos events admin surface, zero suppressions/unwraps, clippy clean |
| [x] | [next-admin](../../crates/modules/rustok-events-module/next-admin) | `modules` | 6 | 278 | 2026-09-20 14:22 | Verified events next-admin UI package, typed status query and delivery configuration API |
| [x] | [rustok-forum](../../crates/modules/rustok-forum) | `modules` | 590 | 152,064 | 2026-09-24 22:40 | Continued Forum attachment boundary audit: implemented bidirectional Media-hold reconciliation with bounded Media owner-reference pagination and exact bulk lookup, tenant/owner/identity response validation, operator GraphQL exposure, host-composed Media provider precedence, and read-only/no-auto-repair semantics. Runtime PostgreSQL/SQLite evidence, gatekeeper, build, and tests remain unrun by the agent. | Canonical read-model and moderation legacy implementations remain consolidated into their owner modules.
| [x] | [admin](../../crates/modules/rustok-forum/admin) | `modules` | 35 | 12,236 | 2026-09-20 16:30 | Verified forum admin package, zero suppressions/unwraps, all 87 tests passed, clippy clean |
| [x] | [storefront](../../crates/modules/rustok-forum/storefront) | `modules` | 19 | 4,164 | 2026-09-20 16:30 | Re-exported public transport API in lib.rs, eliminated 6 allow(dead_code) suppressions, all tests passed, clippy clean |
| [x] | [rustok-fulfillment](../../crates/modules/rustok-fulfillment) | `modules` | 71 | 17,271 | 2026-09-25 12:54 | Moved checkout fulfillment identity from JSON metadata into typed owner columns with tenant-scoped uniqueness and immutable identity guards; checkout recovery now reads through owner APIs. Runtime/build/test evidence remains unrun by the agent. |
| [x] | [admin](../../crates/modules/rustok-fulfillment/admin) | `modules` | 8 | 1,454 | 2026-09-20 16:54 | Replaced 2 allow(too_many_arguments) suppressions with typed ShippingOptionFormSignals struct and methods, all 3 tests passed, clippy clean |
| [x] | [storefront](../../crates/modules/rustok-fulfillment/storefront) | `modules` | 12 | 1,206 | 2026-09-20 16:54 | Verified storefront fulfillment package, zero suppressions/unwraps, all 8 tests passed, clippy clean |
| [x] | [rustok-groups](../../crates/modules/rustok-groups) | `modules` | 112 | 31,916 | 2026-09-20 17:10 | Verified groups module, domain invariants, moderation & governance rules, zero suppressions/unwraps, all 20 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-groups/admin) | `modules` | 34 | 8,520 | 2026-09-20 17:10 | Gated SSR-only imports across 6 native transport adapters, zero suppressions/unwraps, all 4 tests passed, clippy clean |
| [x] | [storefront](../../crates/modules/rustok-groups/storefront) | `modules` | 17 | 3,123 | 2026-09-20 17:10 | Replaced 2 allow(too_many_arguments) suppressions with typed ApplicationFormSignals & ApplicationCopy, cleaned up unused imports across native adapters, all 4 tests passed, clippy clean |
| [x] | [rustok-iggy](../../crates/modules/rustok-iggy) | `modules` | 32 | 8,279 | 2026-09-20 18:48 | Eliminated allow(clippy::too_many_arguments) in DlqDuplicateAlertPolicy, verified DLQ deduplication & fair window scan tests, clippy clean |
| [x] | [rustok-iggy-connector](../../crates/modules/rustok-iggy-connector) | `modules` | 21 | 4,972 | 2026-09-20 18:48 | Eliminated allow(clippy::too_many_arguments) in ConsumerPoisonIdentity, removed dead create_and_connect stub, replaced allow(dead_code) with cfg(any(feature = iggy, test)), all 40 migration + unit tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-iggy-connector/admin) | `modules` | 7 | 612 | 2026-09-20 18:48 | Verified leptos iggy connector admin UI package, zero suppressions/unwraps, all tests passed, clippy clean |
| [x] | [next-admin](../../crates/modules/rustok-iggy-connector/next-admin) | `modules` | 5 | 367 | 2026-09-20 18:48 | Verified iggy connector next-admin components and typed API contract, zero suppressions |
| [x] | [rustok-index](../../crates/modules/rustok-index) | `modules` | 138 | 57,313 | 2026-09-20 19:50 | Eliminated 6 allow suppressions via typed IndexReconciliationBudget, ProductTestSchemas, PartitionAdmissionPolicyParams, PartitionShadowEvidenceParams, and IndexDriftRepairRecoveryCommandParams, resolved dead_code on ReconciliationLease.schema, all 320+ tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-index/admin) | `modules` | 9 | 2,180 | 2026-09-20 19:50 | Verified index admin package, zero suppressions/unwraps, all 7 tests passed, clippy clean |
| [x] | [rustok-inventory](../../crates/modules/rustok-inventory) | `modules` | 35 | 12,002 | 2026-09-20 20:09 | Eliminated 5 allow suppressions (removed deprecated attributes on trait and introduced LocalReservationOutcomeParams), fixed whitespace padding in inventory_policy_allows_backorder, all 28 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-inventory/admin) | `modules` | 10 | 4,267 | 2026-09-20 20:09 | Synchronized boundary tests and documentation with canonical Product catalog service owner reads, all 57 tests passed, zero suppressions, clippy clean |
| [x] | [rustok-marketplace](../../crates/modules/rustok-marketplace) | `modules` | 10 | 1,485 | 2026-09-20 20:55 | Verified marketplace orchestration & reversal protocols, zero suppressions/unwraps, all 5 unit + orchestration tests passed, clippy clean |
| [x] | [rustok-marketplace-allocation](../../crates/modules/rustok-marketplace-allocation) | `modules` | 12 | 1,476 | 2026-09-20 20:55 | Verified multi-vendor allocation batching, zero suppressions/unwraps, all tests passed, clippy clean |
| [x] | [rustok-marketplace-commission](../../crates/modules/rustok-marketplace-commission) | `modules` | 16 | 2,586 | 2026-09-20 20:55 | Verified commission calculation engine and rule precedence, zero suppressions/unwraps, all 3 tests passed, clippy clean |
| [x] | [rustok-marketplace-ledger](../../crates/modules/rustok-marketplace-ledger) | `modules` | 28 | 6,426 | 2026-09-20 20:55 | Replaced 3 allow(too_many_arguments) suppressions with typed LedgerTransactionContext, ReversalOperationContext, and BalanceTransferOperationContext, explicitly bound DatabaseBackend::MySql in immutability migration, all 7 tests passed, clippy clean |
| [x] | [rustok-marketplace-listing](../../crates/modules/rustok-marketplace-listing) | `modules` | 31 | 6,385 | 2026-09-20 20:55 | Replaced 6 allow(too_many_arguments) with AppendListingEventParams, CreateListingTransactionParams, UpdateTermsTransactionParams, TransitionTransactionParams, ReviewTransactionParams, and LegacySnapshotInput, all 6 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-marketplace-listing/admin) | `modules` | 9 | 1,903 | 2026-09-20 20:55 | Replaced 2 allow(too_many_arguments) with CreateListingFormSignals and DetailFormSignals, test passed, clippy clean |
| [x] | [rustok-marketplace-payout](../../crates/modules/rustok-marketplace-payout) | `modules` | 13 | 1,641 | 2026-09-20 20:55 | Fixed SeaORM table name mapping in m20260719_000001_create_marketplace_payouts migration, all tests passed, clippy clean |
| [x] | [rustok-marketplace-seller](../../crates/modules/rustok-marketplace-seller) | `modules` | 48 | 10,637 | 2026-09-20 20:55 | Eliminated allow(dead_code) by removing duplicate dead prose projection, replaced allow(too_many_arguments) with CommandEventParams, LegacySnapshotInput, and TestEventParams, un-nested collapsible if, all 14 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-marketplace-seller/admin) | `modules` | 13 | 2,378 | 2026-09-20 20:55 | Replaced 2 allow(too_many_arguments) with CreateSellerFormSignals and SellerDetailFormSignals, removed unused event import, test passed, clippy clean |
| [x] | [rustok-mcp](../../crates/modules/rustok-mcp) | `modules` | 26 | 6,856 | 2026-09-20 21:15 | Verified MCP runtime, tool execution, session isolation & permission model, zero suppressions/unwraps, all 33 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-mcp/admin) | `modules` | 7 | 1,829 | 2026-09-20 21:15 | Verified MCP admin transport & GraphQL integration, zero suppressions/unwraps, all tests passed, clippy clean |
| [x] | [rustok-media](../../crates/modules/rustok-media) | `modules` | 42 | 12,022 | 2026-09-24 22:40 | Extended Media owner contract with bounded durable-reference listing and exact bulk lookup for consumer reconciliation; synchronized FBA registry/evidence and transport conformance surface. Runtime tests/gatekeeper/build remain unrun by the agent in this continuation. |
| [x] | [rustok-media-transport](../../crates/modules/rustok-media-transport) | `modules` | 6 | 1,532 | 2026-09-24 22:40 | Extended Media gRPC contract with owner-reference list and exact bulk lookup, including explicit trusted-operation authorization and conformance coverage. Runtime tests/gatekeeper/build remain unrun by the agent in this continuation. |
| [x] | [admin](../../crates/modules/rustok-media/admin) | `modules` | 10 | 2,124 | 2026-09-20 21:24 | Verified media admin leptos UI, upload & translation forms, zero suppressions/unwraps, all 11 tests passed, clippy clean |
| [x] | [cli](../../crates/modules/rustok-media/cli) | `modules` | 1 | 175 | 2026-09-20 21:24 | Verified media CLI reconciliation commands & runtime checks, zero suppressions/unwraps, all 3 tests passed, clippy clean |
| [x] | [rustok-moderation](../../crates/modules/rustok-moderation) | `modules` | 36 | 9,113 | 2026-09-20 22:15 | Verified moderation service, transactional outbox & leases, zero suppressions/unwraps, fixed SQLite migration contract UUID blob literal formatting, all 24 tests passed, clippy clean |
| [x] | [rustok-moderation-api](../../crates/modules/rustok-moderation-api) | `modules` | 3 | 766 | 2026-09-20 22:15 | Verified moderation boundary ports, typed capabilities, scope claims, zero suppressions/unwraps, all 4 tests passed, clippy clean |
| [x] | [rustok-modules](../../crates/modules/rustok-modules) | `modules` | 175 | 129,930 | 2026-09-22 16:35 | Added the owner-composed static tenant settings read port and its transaction-aware counterpart: normalized settings plus lifecycle enabled state, exact tenant/module isolation, fail-closed corrupt-state handling, shared server runtime publication, and transaction-path coverage. Existing lifecycle/read-side tests remain unrun by the agent in this pass. |
| [x] | [rustok-modules-translation](../../crates/modules/rustok-modules-translation) | `modules` | 1 | 822 | 2026-09-21 03:45 | Verified translation descriptor mappings, revision coordination & plan application, zero suppressions/unwraps, all 9 tests passed, clippy clean |
| [x] | [cli](../../crates/modules/rustok-modules/cli) | `modules` | 1 | 1,813 | 2026-09-21 03:45 | Verified module CLI inspect, template render, validate & verification commands, zero suppressions/unwraps, all 6 tests passed, clippy clean |
| [x] | [rustok-navigation](../../crates/modules/rustok-navigation) | `modules` | 39 | 5,274 | 2026-09-21 04:35 | Verified menu service, channel bindings & translation target, replaced expect with typed error propagation in menu service, removed anyhow dependency, added INVARIANT comments for descriptors/cursors, zero suppressions/unwraps, all 13 tests passed, clippy clean |
| [x] | [storefront](../../crates/modules/rustok-navigation/storefront) | `modules` | 8 | 494 | 2026-09-21 04:35 | Verified leptos storefront navigation slot & native server adapter, removed unused imports, zero suppressions/unwraps, all tests passed, clippy clean |
| [x] | [rustok-notifications](../../crates/modules/rustok-notifications) | `modules` | 75 | 21,825 | 2026-09-21 05:40 | Formatted SQLite binary UUID BLOBs in group-key trigger/backfill migration, fixed SQLite test fixtures to use BLOB primary keys and UUIDs, zero suppressions, all 23 test suites passed, clippy clean |
| [x] | [rustok-notifications-api](../../crates/modules/rustok-notifications-api) | `modules` | 4 | 1,119 | 2026-09-21 05:40 | Verified API contracts, typed errors and audience page limits, zero suppressions, all 6 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-notifications/admin) | `modules` | 6 | 127 | 2026-09-21 05:40 | Verified admin routes and i18n copy, all tests passed, clippy clean |
| [x] | [storefront](../../crates/modules/rustok-notifications/storefront) | `modules` | 16 | 3,139 | 2026-09-21 05:40 | Replaced string errors in navigate_to_route with typed NotificationNavigationError, fixed literal count placeholder in ftl, removed unused imports, all 30 tests passed, clippy clean |
| [x] | [rustok-order](../../crates/modules/rustok-order) | `modules` | 80 | 18,288 | 2026-09-21 06:20 | Made tenant locale query backend-portable via sea-query, wired ensure_order_schema in tests, fixed order_tax_lines SeaORM entity default_value and test SQL fixtures, verified all 35 tests, clippy clean |
| [x] | [admin](../../crates/modules/rustok-order/admin) | `modules` | 13 | 1,998 | 2026-09-21 06:20 | Made core and ui modules public in lib.rs, introduced typed OrderCommandError enum for command validation, verified all 12 tests, clippy clean |
| [x] | [storefront](../../crates/modules/rustok-order/storefront) | `modules` | 11 | 930 | 2026-09-21 06:20 | Verified storefront contracts, error handling and locale resolution, zero suppressions/unwraps, all 5 tests passed, clippy clean |
| [x] | [rustok-outbox](../../crates/modules/rustok-outbox) | `modules` | 25 | 4,285 | 2026-09-21 06:28 | Verified transactional outbox bus, write-once deduplication, DLQ error routing, SQLite/Postgres relay dispatch, all 37 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-outbox/admin) | `modules` | 8 | 493 | 2026-09-21 06:28 | Made core module public in lib.rs, verified typed OutboxTransportError and tenant scope contracts, all tests passed, clippy clean |
| [x] | [rustok-page-builder](../../crates/modules/rustok-page-builder) | `modules` | 100 | 27,600 | 2026-09-21 06:38 | Replaced expects with pattern matching in static publish policy URL and style validators, verified document contracts and landing service, all 56 tests passed, clippy clean |
| [x] | [rustok-page-builder-storefront](../../crates/modules/rustok-page-builder-storefront) | `modules` | 3 | 1,192 | 2026-09-21 06:38 | Verified storefront render helper, localized route contracts, and SSR head/body integration, all 5 tests passed, clippy clean |
| [x] | [admin](../../crates/modules/rustok-page-builder/admin) | `modules` | 69 | 18,637 | 2026-09-21 06:38 | Documented validated exact field set invariant in consumer property editor, verified authoring, canvas document, SSR drop/assets/forms, all 95 tests passed, clippy clean |
| [x] | [rustok-pages](../../crates/modules/rustok-pages) | `modules` | 170 | 46,835 | 2026-09-21 07:13 | Audited core Pages service writes: tenant-scoped queries, page/version and translation CAS, published-document immutability, route ownership/history uniqueness, immutable artifact publish/rebuild/rollback receipts; closed builder-capability TOCTOU by enforcing the tenant feature gate inside create/document transactions. | Fresh post-merge audit found three route-history N+1 query patterns; Pages now batches published-route candidate page reads, existing publication snapshots, and delete-tombstone alias reads, with duplicate/conflict detection preserved. Source verifier guards the query shape; runtime/build/test evidence remains maintainer-owned.
| [x] | [admin](../../crates/modules/rustok-pages/admin) | `modules` | 24 | 5,904 | 2026-09-21 07:19 | Audited admin access policy, GraphQL transports, SSR Page Builder facade, browser intent preflight, rollback retry identity, scenario baseline CAS, metadata revision CAS, and same-origin inline editor launch; introduced typed ProjectDataParseError and documented compile-time physical invariants, zero suppressions, all 44 tests passed, clippy clean |
| [x] | [storefront](../../crates/modules/rustok-pages/storefront) | `modules` | 17 | 4,717 | 2026-09-21 07:24 | Audited native/GraphQL storefront transport, host route resolution, published artifact rendering, locale/channel-aware caching, authenticated inline editing, and public visibility; closed arbitrary tenant fallback in server functions by binding fallback resolution to the host-configured tenant. |
| [x] | [rustok-payment](../../crates/modules/rustok-payment) | `modules` | 85 | 20,644 | 2026-09-21 07:27 | Audited payment collection/refund lifecycles, provider-operation journal CAS, webhook inbox/lease/idempotency/tenant binding, provider request/result validation, admin and checkout command ports; fixed cross-backend lifecycle races with transactional row locks and CAS, serialized refund capacity reservations, and rejected terminalized collection reuse. |
| [x] | [storefront](../../crates/modules/rustok-payment/storefront) | `modules` | 11 | 1,316 | 2026-09-21 07:31 | Audited storefront native server functions, GraphQL transport, request/error safety, customer/cart ownership, tenant context, guest-cart behavior, and create/reuse flow; no additional production correctness or tenant-isolation finding after the payment lifecycle fixes. |
| [x] | [rustok-pricing](../../crates/modules/rustok-pricing) | `modules` | 47 | 18,461 | 2026-09-21 07:38 | Audited price-list CRUD/translation CAS, active-list resolution, canonical price upserts, rule/scope writes, read/write ports, channel/tenant binding, and money-integrity constraints; fixed rule/scope TOCTOU, duplicate canonical price races via variant serialization, and MySQL channel cross-tenant integrity gaps. |
| [x] | [rustok-pricing-persistence](../../crates/modules/rustok-pricing-persistence) | `modules` | 5 | 190 | 2026-09-21 07:41 | Audited pricing entities and transaction-aware bootstrap persistence; verified decimal/legacy columns, tenant/list/variant linkage, translation uniqueness, cascade FKs, and thin persistence boundaries. No additional production finding. |
| [x] | [admin](../../crates/modules/rustok-pricing/admin) | `modules` | 13 | 7,537 | 2026-09-21 07:47 | Audited admin server functions, GraphQL transport, permission gates, tenant context, price-list rule/scope writes, channel selection, request validation, and error safety; fixed a hardcoded SQLite backend in active price-list validation. |
| [x] | [storefront](../../crates/modules/rustok-pricing/storefront) | `modules` | 10 | 2,465 | 2026-09-21 07:51 | Audited public pricing server function, GraphQL transport, tenant/channel/price-list resolution, published-product visibility, client query sanitization, and error safety; no additional production tenant-isolation or correctness finding. |
| [x] | [rustok-product](../../crates/modules/rustok-product) | `modules` | 168 | 62,712 | 2026-09-21 09:59 | Audited Product lifecycle writes, Variant create/update/delete, Product image mutations, tenant/FK invariants, schema attribute value writes, publish requirements, translation CAS targets, and Product-SalesChannel index relation/convergence/relay paths. Fixed parent-before-child lifecycle locking, serialized only-variant deletion, transactional publish validation, and Product-scoped attribute-value writes. |
| [x] | [rustok-product-bundles](../../crates/modules/rustok-product-bundles) | `modules` | 22 | 3,848 | 2026-09-21 10:05 | Audited bundle CRUD, translations, item lifecycle, position allocation, product/variant references, tenant filtering, and bundle schema constraints; fixed bundle-parent lifecycle races, serialized item add/remove, and enforced tenant ownership of referenced Products/Variants. |
| [x] | [admin](../../crates/modules/rustok-product-bundles/admin) | `modules` | 9 | 2,291 | 2026-09-21 10:12 | Audited GraphQL/native admin transports, tenant guards, bundle CRUD forwarding, and read/write mutation boundaries; fixed native server-function authorization to require effective `PRODUCTS_READ`/`PRODUCTS_UPDATE` permissions in addition to tenant matching. |
| [x] | [rustok-product-catalog-service](../../crates/modules/rustok-product-catalog-service) | `modules` | 1 | 508 | 2026-09-21 10:20 | Audited standalone gRPC launcher, deployment secret/TLS/loopback configuration, bearer interceptor, trusted service actor, tenant metadata binding, Product read-port enforcement, and client transport validation; no additional production security/correctness finding. |
| [x] | [rustok-product-relations](../../crates/modules/rustok-product-relations) | `modules` | 20 | 2,100 | 2026-09-21 10:34 | Audited relation CRUD/reorder, tenant scoping, Product/RelatedProduct ownership, position allocation, concurrency ordering, and lifecycle cleanup. Fixed transactional parent locking, tenant-validated product refs, deterministic two-product lock ordering, and added fail-fast Postgres composite FKs with ON DELETE CASCADE to prevent dangling relations. |
| [x] | [admin](../../crates/modules/rustok-product-relations/admin) | `modules` | 9 | 1,232 | 2026-09-21 10:42 | Audited GraphQL/native relation admin transports, tenant binding, list/add/remove/reorder flows, and server-side auth; fixed native transport to require effective `PRODUCTS_READ`/`PRODUCTS_UPDATE` permissions in addition to tenant matching. |
| [x] | [rustok-product-transport](../../crates/modules/rustok-product-transport) | `modules` | 8 | 1,699 | 2026-09-21 10:55 | Audited gRPC bearer auth, trusted service actor and tenant binding, PortContext replacement, client metadata propagation, TLS/loopback endpoint validation, and transport error mapping; no additional production security/correctness finding. |
| [x] | [admin](../../crates/modules/rustok-product/admin) | `modules` | 20 | 12,023 | 2026-09-21 11:08 | Audited Product admin native/GraphQL transports, catalog/schema/value mutations, lifecycle forwarding, error safety, permission gates, tenant context, idempotency/retry identity and revision-bearing mutations; all native server functions enforce effective product permissions and trusted tenant context. No additional production finding. |
| [x] | [storefront](../../crates/modules/rustok-product/storefront) | `modules` | 12 | 3,098 | 2026-09-21 10:16 | Audited native/GraphQL storefront transports, published-product visibility, tenant/channel context, locale fallback, public inventory projection, pricing context, and client-side catalog controls; no additional production correctness or tenant-isolation finding. |
| [x] | [rustok-profiles](../../crates/modules/rustok-profiles) | `modules` | 52 | 8,990 | 2026-09-21 10:24 | Audited profile GraphQL read/write boundaries, privacy and follower checks, tenant/self ownership, media validation, handle/locale/visibility writes, profile upsert/event transactions, translation/tag integrity, and unique `(tenant, handle)`/`(profile, locale)` constraints; no additional production finding. |
| [x] | [cli](../../crates/modules/rustok-profiles/cli) | `modules` | 1 | 338 | 2026-09-21 10:28 | Audited profiles backfill command boundary: required tenant ID, tenant/user/enrichment reads, tenant-scoped profile mutations, dry-run/event behavior, and operational visibility options; no additional production authorization or cross-tenant finding. |
| [x] | [storefront](../../crates/modules/rustok-profiles/storefront) | `modules` | 9 | 1,464 | 2026-09-21 10:34 | Audited native/GraphQL profile reads, tenant and privacy boundaries, public media owner validation, authenticated follow mutations, idempotency/revision handling, locale fallback, and client transport selection; no additional production security/correctness finding. |
| [x] | [rustok-rbac](../../crates/modules/rustok-rbac) | `modules` | 55 | 11,022 | 2026-09-21 10:41 | Audited role mutation authority/continuity, tenant-integrity triggers, control-plane admission, durable permission invalidation generation, relation-based permission resolution/cache fencing, artifact permission catalog/assignment idempotency, and system-role repair; no additional production correctness or authorization finding. |
| [x] | [admin](../../crates/modules/rustok-rbac/admin) | `modules` | 8 | 503 | 2026-09-21 10:45 | Audited native server bootstrap boundary, direct-user control-plane admission, tenant match, SETTINGS_READ permission gate, permission catalog rendering, and UI transport context; no additional production authorization finding. |
| [x] | [cli](../../crates/modules/rustok-rbac/cli) | `modules` | 1 | 243 | 2026-09-21 10:48 | Audited consistency/repair command boundaries, tenant-id/all-tenants safeguards, dry-run/apply semantics, transactional system-role repair, and durable permission invalidation generation; no additional production finding. |
| [x] | [rustok-reactions](../../crates/modules/rustok-reactions) | `modules` | 7 | 2,836 | 2026-09-21 10:53 | Audited reaction write/read ports, tenant/actor authorization, subject serialization, subject/catalog revision CAS, idempotency receipts, aggregate counters, transactional events, reconciliation bounds/claims, and persistence uniqueness; no additional production correctness or authorization finding. |
| [x] | [rustok-reactions-api](../../crates/modules/rustok-reactions-api) | `modules` | 3 | 1,200 | 2026-09-21 10:58 | Audited reaction contract models/provider registry: non-nil tenant/subject/actor identity, positive revisions, bounded/unique catalogs and actor state, canonical subject matching, fail-closed deserialization, and provider registry validation. No additional production finding. |
| [x] | [rustok-reactions-storefront](../../crates/modules/rustok-reactions-storefront) | `modules` | 6 | 616 | 2026-09-21 11:03 | Audited public reaction transport/UI: auth-owned tenant/actor context, canonical subject revision validation, GraphQL inputs excluding tenant/actor identity, generated command IDs, stale-write refresh behavior, and safe reaction snapshot rendering. |
| [x] | [rustok-region](../../crates/modules/rustok-region) | `modules` | 39 | 8,559 | 2026-09-21 11:10 | Audited Region CRUD, tenant-scoped reads, exclusive-row serialization, translation/tax-policy replacement transactions, exact-locale translation CAS, revision high-water/change cursors, permission floors, and canonical Region translation-target provider; no additional production finding. |
| [x] | [admin](../../crates/modules/rustok-region/admin) | `modules` | 8 | 2,944 | 2026-09-21 11:17 | Audited Region admin server functions/UI: trusted AuthContext/TenantContext usage, permission gates for bootstrap/list/read/create/update, tenant-scoped RegionService calls, input normalization, and error boundaries; no additional production finding. |
| [x] | [storefront](../../crates/modules/rustok-region/storefront) | `modules` | 9 | 1,739 | 2026-09-21 11:24 | Audited native/GraphQL public region data, tenant context, locale/default-locale fallback, public currency/tax presentation, region selection routing, error fallback semantics, and UI rendering; no additional production tenant-isolation or privacy finding. |
| [x] | [rustok-search](../../crates/modules/rustok-search) | `modules` | 95 | 30,084 | 2026-09-24 17:35 | Audited tenant-scoped Search queries, transport-neutral PortContext authority, GraphQL/admin permissions, public storefront/channel trust, forum result-owner boundaries, analytics and dictionary cross-tenant checks, and PostgreSQL projection lifecycle; fixed authoritative tenant binding in Search ports, prevented stale projector overwrites with monotonic updated_at guards, and added PostgreSQL uniqueness for one settings row per tenant/global scope with duplicate cleanup; later hardened click tracking so the referenced search document must also belong to the same tenant.  Added fresh GraphQL error-boundary hardening: query, mutation, and Forum reconciliation adapters now use one centralized mapping that redacts database/external/internal details at the GraphQL boundary while preserving typed caller-safe codes. Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent. | Also found and fixed a real tenant-boundary gap in the Blog Search projector: author enrichment now requires both `users.id = blog_posts.author_id` and `users.tenant_id = blog_posts.tenant_id`, with a PostgreSQL negative regression for malformed cross-tenant author references and a source verifier guard. Also normalized the Search projector source naming: the current canonical implementation moved from the retired `projector_legacy.rs` name to `projector_core.rs` atomically, and the facade no longer carries legacy naming. Found two source-quality defects in the canonical Search projector: retired `projector_legacy.rs` naming has been removed in favor of `projector_core.rs`, and bootstrap count decoding now fails closed instead of silently converting database decode errors to zero; the central Search FBA verifier enforces both invariants. Bootstrap hardening is now enforced in both canonical projector layers: database count decode failures propagate instead of being coerced to zero, preventing schema/corruption failures from triggering false bootstrap rebuilds. The Blog projector also now validates source-table availability before destructive full/targeted replacement, with missing schemas and availability-row decode failures propagated so an unavailable source cannot erase the last committed Blog Search projection. The article-body refresh loop now likewise propagates `document_key` decode failures instead of treating them as an end-of-stream condition, preventing partial canonical-body indexing from being committed as success.
| [x] | [admin](../../crates/modules/rustok-search/admin) | `modules` | 20 | 6,725 | 2026-09-21 11:55 | Audited native Leptos server functions, GraphQL transport, auth/tenant extraction, settings/dictionary/rebuild permissions, target ID handling, and tenant-scoped read/write delegation; no additional production security or concurrency finding remained. |
| [x] | [storefront](../../crates/modules/rustok-search/storefront) | `modules` | 11 | 3,453 | 2026-09-21 12:00 | Audited public native/GraphQL transport, trusted tenant/channel resolution, published-only scope, suggestions/presets, click tracking, and forum-owner search path; no additional production security or concurrency finding remained after backend click tenant binding. |
| [x] | [rustok-seo](../../crates/modules/rustok-seo) | `modules` | 67 | 30,112 | 2026-09-22 17:30 | Continued settings-ownership review: removed SEO runtime reads of the tenant persistence entity and routed enabled/lifecycle settings through the static module settings owner port; preserved persisted settings readability while disabled and fail-closed reader failures. Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent. |
| [x] | [rustok-seo-admin-support](../../crates/modules/rustok-seo-admin-support) | `modules` | 6 | 2,627 | 2026-09-21 12:16 | Audited Leptos SEO entity panel and reusable widgets, client target/locale validation, GraphQL query/mutation transport, token and tenant forwarding, and rendered HTML surface; no direct DB/privileged backend path or additional production security, tenant-scope, or concurrency finding remained. Authoritative authorization stays in the shared SEO GraphQL backend. |
| [x] | [rustok-seo-targets](../../crates/modules/rustok-seo-targets) | `modules` | 1 | 1,252 | 2026-09-23 03:04 | Continued SEO target boundary review: removed redundant runtime panics from `SeoTargetSlug` validation while preserving the typed empty/boundary/character error contract; remaining `expect` uses are test-only fixtures/assertions. |
| [x] | [admin](../../crates/modules/rustok-seo/admin) | `modules` | 15 | 4,462 | 2026-09-21 12:25 | Audited native SEO admin server functions, auth/tenant extraction, per-operation permission checks, settings persistence, bulk/index/sitemap mutation boundaries, operator UI confirmations, and tenant-scoped service delegation; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [render](../../crates/modules/rustok-seo/render) | `modules` | 1 | 699 | 2026-09-21 12:33 | Audited the server-independent SEO head renderer for HTML injection, attribute escaping, structured-data emission, deterministic ordering, and public metadata rendering; fixed JSON-LD script breakout by escaping <, >, and & before embedding serialized JSON in script content. |
| [x] | [rustok-social-graph](../../crates/modules/rustok-social-graph) | `modules` | 36 | 8,796 | 2026-09-21 12:45 | Audited relation storage/FKs and uniqueness, GraphQL auth and tenant binding, command-port actor ownership and CAS/idempotency, privacy read/index scopes, monotonic index revisions, DLQ receipt claims, and maintenance replay/cleanup authorization; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-social-graph-cli](../../crates/modules/rustok-social-graph-cli) | `modules` | 1 | 324 | 2026-09-21 12:52 | Audited the owner-local receipt-cleanup command, explicit tenant/retention/limit parsing, system-actor PortContext, idempotency scope, runtime DB boundary, and delegation to the backend maintenance port; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-tax](../../crates/modules/rustok-tax) | `modules` | 5 | 1,586 | 2026-09-21 13:00 | Audited the transport-neutral tax calculation port, policy admission, tenant/channel context handling, provider selection, currency/rate validation, taxable-target binding, result invariants, and decimal tax calculation; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-taxonomy](../../crates/modules/rustok-taxonomy) | `modules` | 51 | 13,035 | 2026-09-24 14:45 | Continued owner-read audit from Blog tag projection: invalid persisted Taxonomy term locales are now classified as storage invariants and fail closed instead of being silently retained in the localized-name map; canonical/localized route-key mutations now reject post-normalization values beyond the 120-character storage boundary instead of leaking a database error; module scope normalization now enforces the 64-character storage boundary across mutation and owner-read paths. | Fresh cross-owner audit found stale Search projections after shared global Tag updates/deletes and Translation-target applies because Search projects live Taxonomy names. Taxonomy now emits the existing tenant-scoped `index.reindex_requested` event inside the same transaction for global Tags only; module-owned terms are excluded, and PostgreSQL translation evidence now requires the canonical `sys_events` table.
| [x] | [rustok-tenant](../../crates/modules/rustok-tenant) | `modules` | 28 | 3,836 | 2026-09-21 13:34 | Audited tenant lifecycle, bootstrap idempotency, slug/domain uniqueness, host-owned tenant resolution, tenant cache keys/invalidation boundaries, locale-policy CAS/idempotency, module read projections, settings schema limits, and admin auth/tenant/permission checks; no additional production security, tenant-scope, or concurrency finding remained. `TenantReadPort` arbitrary selector semantics are intentional for the host-owned pre-context resolver. |
| [x] | [admin](../../crates/modules/rustok-tenant/admin) | `modules` | 8 | 726 | 2026-09-21 13:42 | Audited the native tenant admin server function, auth/tenant scope matching, tenant/module RBAC gates, effective module policy projection, token/tenant UI forwarding, and rendered UI surface; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-translation](../../crates/modules/rustok-translation) | `modules` | 76 | 45,087 | 2026-09-21 14:05 | Audited translation GraphQL auth/context, workflow/proposal/apply CAS and idempotency, machine-operation leases/recovery, translation memory/glossary tenant scope, inventory and provider boundaries, interchange storage/processing leases, scheduler service/system contexts, collaboration permissions, and PostgreSQL/SQLite tenant-keyed schema constraints; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-translation-targets](../../crates/modules/rustok-translation-targets) | `modules` | 3 | 1,729 | 2026-09-21 15:10 | Audited provider contract/registry, capability combinations, opaque revisions and CAS helpers, protected-token/data-classification rules, apply/read context policy, and real Pages/Product owner adapters. Provider descriptors expose read/apply permission floors as metadata, but every registered owner adapter independently enforces authoritative Resource/Action permissions and tenant-scoped owner reads/writes; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [admin](../../crates/modules/rustok-translation/admin) | `modules` | 9 | 13,412 | 2026-09-21 15:28 | Audited native server-function and GraphQL transports, auth/request/tenant context binding, translation operation dispatch, permission/idempotency delegation, machine/import/export controls, tenant/locale forwarding, and UI rendering; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-workflow](../../crates/modules/rustok-workflow) | `modules` | 52 | 6,794 | 2026-09-21 16:05 | Audited workflow CRUD/versioning, tenant-scoped execution reads, trigger/webhook paths, event idempotency, transactional workflow/step mutation locks, execution state transitions, retry behavior, HTTP SSRF controls, script/notification abstractions, and GraphQL/HTTP permissions. Fixed unauthenticated webhook execution: webhook secrets are now explicitly configured, HMAC-SHA256 signatures are verified over the raw request body with constant-time comparison, missing/invalid signatures fail closed, and webhook signatures are no longer logged. |
| [x] | [admin](../../crates/modules/rustok-workflow/admin) | `modules` | 14 | 1,120 | 2026-09-21 16:28 | Audited native/GraphQL admin transport, auth and tenant forwarding, workflow permission gates, template creation path, UI tenant/token context, and rendered admin surface. Fixed a real source-integrity defect: `lib.rs`/UI/transport referenced a missing `core.rs`; restored the module-compatible core view-model and transport-context layer. No additional production security or tenant-scope finding remained. |
| [x] | [fly](../../crates/ui/fly) | `ui` | 65 | 23,474 | 2026-09-21 16:45 | Audited the framework-neutral Fly editor core: bundle/hash integrity, codec round-trip boundaries, runtime context contracts/dependencies/gates, internal links, safe URL and metadata handling, action/form capability gates, page rendering, CSS/HTML escaping, asset/style validation, and interaction materialization. No production auth/tenant boundary exists in this crate by design, and no additional XSS, URL, capability, or concurrency finding remained. |
| [x] | [fly-browser](../../crates/ui/fly-browser) | `ui` | 9 | 816 | 2026-09-21 16:58 | Audited the SSR-first browser adapter and bundled JavaScript bridge: message origin/source validation, protocol/instance/sequence checks, browser resource limits, pending-intent leases/timeouts, CSRF and same-origin fetch credentials, draft/revision/project-hash propagation, drag/drop intents, and consumer-owned endpoint delegation. No additional production security or tenant-scope finding remained. |
| [x] | [fly-leptos](../../crates/ui/fly-leptos) | `ui` | 5 | 1,691 | 2026-09-21 17:12 | Audited Leptos SSR/wasm feature separation, iframe/browser runtime, exact origin/source message validation, monotonic sequence handling, explicit outbound postMessage origins, pointer/resize lifecycle cleanup, and authenticated real-DOM inline-edit grants with session/page/revision/project-hash binding, expiry, sequence, field allow-list, and bounded plain-text input; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [fly-ui](../../crates/ui/fly-ui) | `ui` | 20 | 5,225 | 2026-09-21 17:40 | Audited capability intersection/normalization, command-to-capability mapping, contribution assembly filters, tenant/permission/provider-health policy inputs, palette access, drag/drop legality, keyboard/resize behavior, state-machine mutation gates, save/publish diagnostics, and cleanup/history state. Browser-supplied drop candidates are rechecked for legality before command generation and `command_for_drop` is crate-private; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [leptos-auth](../../crates/ui/leptos-auth) | `ui` | 9 | 1,494 | 2026-09-21 18:10 | Audited SSR/browser auth state, LocalStorage compatibility mode, request-scoped SSR snapshots, first-party Leptos server functions, GraphQL/native auth transport, token refresh/logout/current-user flow, tenant metadata forwarding, protected/guest route guards, and auth error mapping. Client token/tenant values remain transport metadata; authoritative tenant/session identity is established by the auth backend/JWT rather than this UI crate. No additional production security or tenant-scope finding remained. |
| [x] | [leptos-ui](../../crates/ui/leptos-ui) | `ui` | 7 | 503 | 2026-09-21 19:20 | Audited shared Leptos UI primitives and richtext sinks. RichTextHtml consumes only server-derived RichTextView; the canonical rustok-content renderer enforces allowlisted schema/nodes/marks, bounded size/depth, escaped text/attributes, and safe link schemes. RichText editor iframe is same-origin framed, sandboxed with `allow-scripts` but no `allow-same-origin`, and assets are embedded/server-owned. No additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [leptos-ui-routing](../../crates/ui/leptos-ui-routing) | `ui` | 1 | 233 | 2026-09-21 19:35 | Audited SSR/browser query reads and writes, host-provided route query sanitization, current-path navigation construction, and locale context; no arbitrary destination URL sink, auth/tenant boundary, network call, or concurrency state exists in this crate. |
| [x] | [rustok-graphql](../../crates/ui/rustok-graphql) | `ui` | 1 | 338 | 2026-09-21 20:10 | Audited the framework-agnostic GraphQL HTTP client for endpoint derivation, SSR/WASM defaults, auth/tenant/locale header forwarding, response/error mapping, persisted-query extensions, timeout/client lifecycle, and raw-client delegation. Endpoint remains an adapter/configuration input rather than request-derived authority; client tenant slug is transport metadata and backend auth context remains authoritative. No confirmed production security, tenant-scope, URL, or concurrency finding remained. |
| [x] | [rustok-ui-auth](../../crates/ui/rustok-ui-auth) | `ui` | 1 | 54 | 2026-09-21 20:48 | Audited shared client auth/session DTOs, expiry policy, error mapping, and serialization/debug boundaries. Fixed a real secret-leak risk by removing derived Debug from AuthSession and redacting both access and refresh tokens in its custom Debug implementation; backend Auth/RBAC remains authoritative. |
| [x] | [rustok-ui-core](../../crates/ui/rustok-ui-core) | `ui` | 5 | 1,280 | 2026-09-21 21:10 | Audited shared UI DTOs and helpers for CSS/HTML safety, route/query construction, navigation paths, selection/filter state, money formatting, badge/status mapping, and busy-key semantics. CSS inputs are reduced to finite class palettes, query keys use typed/static intents, navigation paths are URL-encoded through the URL API, and no auth/tenant/network authority exists in this crate. No additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-ui-forms](../../crates/ui/rustok-ui-forms) | `ui` | 1 | 75 | 2026-09-21 21:35 | Audited shared form submission state, field-error mapping, status transitions, serialization/debug surfaces, and validation issue conversion; no server/auth/tenant/network/secret boundary exists in this crate and no production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-ui-i18n](../../crates/ui/rustok-ui-i18n) | `ui` | 26 | 3,489 | 2026-09-21 22:05 | Audited locale normalization/fallback candidates, Fluent bundle construction, duplicate/default-locale handling, strict vs lenient startup semantics, interpolation/bidi isolation, key normalization macros, and diagnostic/error surfaces. Inputs are bounded and catalog ownership is static/module-local; no auth/tenant/network authority or production security/concurrency finding remained. |
| [x] | [rustok-ui-transport](../../crates/ui/rustok-ui-transport) | `ui` | 1 | 266 | 2026-09-21 22:30 | Audited native/GraphQL transport selection and fallback semantics. Found a reusable side-effect retry hazard: generic fallback could execute a secondary transport after a primary mutation response was lost. Added explicit `UiTransportRetrySafety` and made fallback conditional on `SafeToRetry`; `AtMostOnce` operations never fallback even when enabled, with regression coverage. |
| [x] | [rustok-build](../../crates/utils/rustok-build) | `utils` | 12 | 2,033 | 2026-09-21 23:10 | Audited platform build persistence, immutable execution-plan hashing, global build-event scope, command construction, manifest snapshot materialization, history bounds, status lifecycle, and queued worker execution. Fixed a real concurrency race: queued builds are now atomically claimed with a status CAS before execution, both worker entry points use the claim path, and only the winning claimant emits BuildStarted, preventing duplicate execution of the same build under concurrent workers. |
| [x] | [rustok-build-publication](../../crates/utils/rustok-build-publication) | `utils` | 4 | 862 | 2026-09-21 23:35 | Audited registry credential brokering, fixed executable identity, bounded subprocess I/O/timeouts, OCI target/artifact validation, Cosign KMS references, signed attestation normalization, digest-pinned publication, and private temporary credential/predicate cleanup. Fixed a real secret-exposure risk by creating registry credential directories as `0700` and Docker config files as `0600` on Unix. |
| [x] | [rustok-build-source](../../crates/utils/rustok-build-source) | `utils` | 2 | 1,570 | 2026-09-22 00:05 | Audited fixed-root CAS source identity, digest verification, strict USTAR parsing/materialization, deterministic source archive creation, path/symlink/type rejection, bounded archive/extraction/entry limits, atomic no-replace CAS publication, and partial-destination cleanup. The crate explicitly operates on trusted control-plane source media rather than Git/HTTP/arbitrary filesystem references; no additional production integrity, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-cli](../../crates/utils/rustok-cli) | `utils` | 3 | 1,302 | 2026-09-22 00:25 | Audited CLI command registration/duplicate detection, argument normalization, dry-run propagation, distribution-provider dispatch, runtime initialization, exit/error rendering, and CLI core provider contracts. The runner performs no shell execution or secret handling itself; command providers own privileged operations, while the core provider trait fails closed on unimplemented execution. No additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-cli-core](../../crates/utils/rustok-cli-core) | `utils` | 1 | 118 | 2026-09-22 00:50 | Audited CLI command contracts, provider fail-closed behavior, dry-run capability metadata, and dispatch semantics. Found and fixed a real safety gap: supports_dry_run was previously advisory, so `migrate up --dry-run` could still execute migrations; typed DryRunNotSupported enforcement now runs before provider execution, while the rebuild command remains explicitly dry-run capable. |
| [x] | [rustok-cli-platform](../../crates/utils/rustok-cli-platform) | `utils` | 3 | 466 | 2026-09-22 01:15 | Audited platform CLI provider commands, runtime host DB binding, PostgreSQL/SQLite baseline SQL, tenant-scoped report queries, build/rebuild delegation, migration/status paths, argument parsing, and output handling. `core rebuild` is explicitly dry-run capable and migration commands are protected by the CLI core dry-run gate; no additional production security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-cli-registry](../../crates/utils/rustok-cli-registry) | `utils` | 2 | 131 | 2026-09-22 01:35 | Audited generated provider composition, distribution selection, provider reference exposure, command inventory ordering, and duplicate-command responsibility. Registry is a static owner-provider composition; authoritative duplicate rejection remains at CLI dispatch, and no additional security, tenant-scope, or concurrency finding remained. |
| [x] | [rustok-installer](../../crates/utils/rustok-installer) | `utils` | 11 | 4,102 | 2026-09-22 01:55 | Audited installer preflight, topology/distribution admission, state-machine/recovery transitions, secret resolution/redaction, seed/admin transaction boundaries, deployment receipt validation, and durable session locking. Found and fixed a real global-lock race in rustok-installer-persistence: concurrent sessions could both observe no active lock and acquire independently; lock acquisition is now serialized at the authoritative persistence write point (PostgreSQL SERIALIZABLE, transactional write path for SQLite), with empty owners rejected. No additional production tenant-scope or integrity bypass remained. |
| [x] | [rustok-installer-cli](../../crates/utils/rustok-installer-cli) | `utils` | 1 | 443 | 2026-09-22 02:20 | Audited installer/seed CLI dispatch, dry-run semantics, secret/reference parsing and redaction, signed base-distribution receipt admission, instance-root binding, tenant/bootstrap inputs, lock options, and durable status rendering. Fixed an integration regression after the CLI dry-run safety gate: seed apply and install apply explicitly declare with_dry_run(), matching their non-mutating implementation paths. |
| [x] | [rustok-installer-persistence](../../crates/utils/rustok-installer-persistence) | `utils` | 7 | 1,380 | 2026-09-22 02:55 | Audited durable installer sessions, receipts, state persistence, global install lock acquisition, tenant assignment, seed/module lifecycle writes, bootstrap idempotency, admin/role transactions, and PostgreSQL/SQLite persistence boundaries. Fixed two real production safety issues: serialized global lock acquisition at the DB boundary (PostgreSQL SERIALIZABLE, transactional SQLite path) and fail-closed rejection of the Dev seed profile for production, including the deterministic demo-customer password path; preflight now rejects the same configuration before DB access. |
| [x] | [rustok-migrations](../../crates/utils/rustok-migrations) | `utils` | 65 | 14,997 | 2026-09-22 03:20 | Audited the canonical migration registry/order/dependency validation, installer owner/remaining schema split, SQLite compatibility boundary, high-risk registry identity/artifact backfills, Flex localization backfills, lease/receipt migrations, and destructive down paths. Fixed a real privileged-filesystem issue in the registry identity/artifact migration: legacy artifact sources are now confined to the managed storage root, destination keys reject traversal/absolute/control components, symlink destinations are rejected, and regression coverage was added. |
| [x] | [rustok-module-sdk](../../crates/utils/rustok-module-sdk) | `utils` | 2 | 60 | 2026-09-22 03:40 | Audited the frozen WIT guest contract and generated bindings for capability invocation, package/world identity, input/output serialization boundaries, and export macros. No tenant/auth/secret authority exists in the guest SDK; capability authorization remains host-owned and no additional production security or concurrency finding remained. |
| [x] | [rustok-module-template](../../crates/utils/rustok-module-template) | `utils` | 2 | 329 | 2026-09-22 04:00 | Audited generated module source/manifest rendering, identity validation, sandbox contract, brokered capability declarations, build-policy defaults, Index boundary documentation, and host-owned artifact descriptor generation. Template policy is fail-closed (no git dependencies, build scripts, or native links), and no additional production security or tenant-scope finding remained. |
| [x] | [rustok-secrets](../../crates/utils/rustok-secrets) | `utils` | 3 | 1,306 | 2026-09-21 22:45 | Audited secret reference registry, access policy, short-lived cache, redacted Debug formatting, and Env/MountedFile/Vault/Kubernetes/Cloud resolvers; added non-blocking in_cluster_async with tokio::fs, refactored synchronous in_cluster stream read, and documented registry registration invariants. Zero suppressions, all 13 tests passed, clippy clean |
| [x] | [rustok-storage](../../crates/utils/rustok-storage) | `utils` | 4 | 774 | 2026-09-21 22:56 | Audited storage runtime abstraction, local filesystem and S3 drivers, in-memory test fallback, key partitioning, and object-store conformance; added non-blocking local_async with tokio::fs::create_dir_all for async runtime initialization and refactored local directory creation via DirBuilder. Zero suppressions, all 6 tests passed, clippy clean |
| [x] | [rustok-test-utils](../../crates/utils/rustok-test-utils) | `utils` | 7 | 1,917 | 2026-09-21 23:10 | Audited test database setup, fixture generators, mock event bus, transaction wrappers, and assertion helpers; extracted in_memory_sqlite_options helper to decompose DB setup function length. Zero suppressions, all 15 tests passed, clippy clean |
| [x] | [utoipa-swagger-ui-vendored](../../crates/utils/utoipa-swagger-ui-vendored) | `utils` | 2 | 34 | 2026-09-21 23:15 | Audited vendored Swagger UI static assets and module export; verified no unwrap/panic, no blocking I/O, clippy clean, zero findings. |
| [x] | [rustok-artifact-node-agent](../../crates/workers/rustok-artifact-node-agent) | `workers` | 8 | 2,426 | 2026-09-21 23:40 | Audited side-by-side slot supervisor, materializer, watchdog, and assignment heartbeat loop; decomposed process_assignment, prepare_local, slot transitions, and watchdog recovery to comply with metrics limits. Zero suppressions, all 18 tests passed, clippy clean |
| [x] | [rustok-artifact-node-controller](../../crates/workers/rustok-artifact-node-controller) | `workers` | 3 | 137 | 2026-09-21 23:55 | Audited mTLS controller listener, database connection setup, agent authenticator parsing, and admission wiring; verified fail-closed fingerprint matching, zero unwrap/panic in runtime code, clippy clean, all tests passed. |
| [x] | [rustok-artifact-node-reconciler](../../crates/workers/rustok-artifact-node-reconciler) | `workers` | 3 | 158 | 2026-09-21 23:57 | Audited mTLS operator listener, scoped operator principal authenticator, and service delegation to database; verified fail-closed node scoping, zero unwrap/panic in runtime code, clippy clean, all tests passed. |
| [x] | [rustok-artifact-node-transport](../../crates/workers/rustok-artifact-node-transport) | `workers` | 6 | 1,016 | 2026-09-22 00:00 | Audited gRPC protobuf protocol definition, node agent and operator reconciliation services, client mTLS wrappers, and admission guards; verified fail-closed certificate extraction, zero unwrap/panic in runtime code, clippy clean, all 10 tests passed. |
| [x] | [rustok-module-build-dispatcher](../../crates/workers/rustok-module-build-dispatcher) | `workers` | 3 | 478 | 2026-09-22 00:20 | Audited broker queue consumer, mTLS build worker client connection, result-first execution flow, and shutdown coordinator; decomposed host from_env, run_dispatcher loop, and receive delivery parser to comply with function metrics. Zero suppressions, all tests passed, clippy clean |
| [x] | [rustok-module-build-transport](../../crates/workers/rustok-module-build-transport) | `workers` | 6 | 325 | 2026-09-22 00:25 | Audited gRPC protobuf contracts and client/server adapters for module-build and distribution workers; verified fail-closed admission and error propagation, zero unwrap/panic, clippy clean. |
| [x] | [rustok-module-build-worker](../../crates/workers/rustok-module-build-worker) | `workers` | 8 | 4,121 | 2026-09-22 00:50 | Audited OCI build worker runner, policy engine, artifact publication bundle collector, and SLSA provenance inspection; eliminated blocking fs::read in isolation attestation, fixed DirBuilder unused_mut in build publication, verified zero unwrap/panic in runtime code, all 10 tests passed, clippy clean. |
| [x] | [rustok-sandbox](../../crates/workers/rustok-sandbox) | `workers` | 26 | 8,092 | 2026-09-22 02:00 | Audited sandbox runtime, Wasm Component Model & Rhai executors, capability constraints broker, scenario harness, and admission control; decomposed oversized capability.rs into typed submodules under src/capability/, extracted helpers in runtime.rs, wasm.rs, rhai.rs, rhai_workspace.rs, and harness.rs. Zero suppressions, all 59 tests passed, clippy clean. |
| [x] | [rustok-sandbox-transport](../../crates/workers/rustok-sandbox-transport) | `workers` | 5 | 1,220 | 2026-09-22 02:14 | Audited gRPC protobuf protocol definition, client session loop, worker service adapter, capability callback broker, and frame stream processing; decomposed execute, execute_session, invoke, and read_host_frames to comply with metrics limits. Zero suppressions, all 7 tests passed, clippy clean. |
| [x] | [rustok-sandbox-worker](../../crates/workers/rustok-sandbox-worker) | `workers` | 3 | 692 | 2026-09-22 02:25 | Audited Rhai execution isolation wrapper, cgroup v2 memory observer probe, and hardened deployment attestation parser; decomposed execute and load_attestation, eliminated blocking fs::read and fs::read_to_string, verified zero unwrap/panic in runtime code, all 6 tests passed, clippy clean. |
| [x] | [rustok-static-distribution-worker](../../crates/workers/rustok-static-distribution-worker) | `workers` | 7 | 2,730 | 2026-09-22 03:30 | Audited distribution job runner, workspace materializer, fixed cargo pipeline runner, Cosign/SLSA publisher, and OCI referrer evidence; decomposed job execution, workspace source extraction, and publisher helpers into dedicated modules, resolved all metric findings and lifetime elisions, verified zero unwrap/panic in runtime code, all tests passed, clippy clean. |
| [x] | [rustok-verification-transport](../../crates/workers/rustok-verification-transport) | `workers` | 4 | 174 | 2026-09-25 06:05 | Audited gRPC protobuf protocol definition, verification client and service adapters, and mTLS readiness probe; verified WorkerAdmission permit acquisition, fail-closed error propagation, zero unwrap/panic in runtime code, clippy clean, all 3 tests passed. |
| [x] | [rustok-verification-worker](../../crates/workers/rustok-verification-worker) | `workers` | 5 | 1,080 | 2026-09-25 08:40 | Audited Cosign OCI trust verification adapter, SLSA provenance and CycloneDX SBOM validation, and policy evaluation; decomposed verify_with_flags, verify_trust_root, and verify to comply with function metrics limits, zero suppressions, zero unwrap/panic in runtime code, all 13 tests passed, clippy clean. |
| [x] | [rustok-worker-transport](../../crates/workers/rustok-worker-transport) | `workers` | 1 | 472 | 2026-09-25 09:00 | Audited mutually authenticated TLS listener/client configurations, leaf certificate SHA-256 fingerprint parsing, process-wide WorkerAdmission semaphore permits, and graceful shutdown signal handler; verified zero unwrap/panic in runtime code, all 10 tests passed, clippy clean. |

---

## 2026-09-24 Forum attachment relation CAS contract

A Forum-14 boundary review separated attachment-set concurrency from the existing
immutable mention/quote relation revision stream. `ForumAttachmentRelationRevision` now
provides a bounded monotonic CAS token: `0` means no committed set, the initial write
uses `0 -> 1`, every later replace requires an exact current token, and clearing all
attachments still commits a new revision rather than deleting the concurrency identity.
The pre-existing `source_revision` remains content provenance only and attachment-only
changes do not consume Forum content revisions. The batch-size constant is now named
`MAX_FORUM_ATTACHMENTS_PER_SET` to reflect the actual invariant.

Attachment persistence is intentionally not added in this slice. Media's
`MediaAssetReferenceAdmission` proves lifecycle admissibility at read time but does not
reserve an asset against deletion, so persisting a Forum reference on that fact alone
would retain a cross-owner time-of-check/time-of-use race. A Media reference-retention/
control contract is required before Forum attachment rows become authoritative.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.

## 2026-09-24 Forum attachment relation persistence

FORUM-14 now has a concrete owner-owned persistence path after the Media reference-retention control landed. Forum stores a per-tenant/target/locale CAS head plus bounded ordered relation rows; the mutable attachment head is deliberately separate from Forum content revisions and the immutable mention/quote relation stream.

New Media references are retained before Forum relation commit using stable consumer-owned identities derived from tenant, target, locale, position and media identity. Removed holds are released only after a successful Forum commit. Ambiguous commits and post-commit release failures preserve conservative holds instead of unsafe compensation. Database constraints enforce target-kind validity, tenant scope, positive revisions, bounded positions and unique ordering.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.
## 2026-09-24 Media durable reference-retention control

A Forum-to-Media boundary review identified the remaining deletion time-of-check/time-of-use gap in lifecycle admission. Media now owns durable `media_asset_reference_holds` keyed by a consumer-owned stable `reference_id`, with tenant cascading plus composite asset foreign-key integrity and database lifecycle guards: only active assets with ready active blobs can accept a hold, while lifecycle transitions into `delete_pending` or `deleted` fail closed whenever a hold exists. The Media write-port acquire/release operations use the existing trusted deadline/idempotency path; same-reference replay is idempotent and retargeting is rejected. Media deletion and finalization recheck reference holds.

The control intentionally does not claim distributed transactionality with consumer-owned databases. An owner crash after acquiring a hold before its own relation commit can leave a conservative orphan hold; consumer-side bounded reconciliation must release only provably stale identities. This is the required safety primitive before Forum attachment relations become authoritative.

Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.

## 2026-09-25 Product owner-port diagnostic hardening

The Product catalog read port still emitted raw owner error details and request identity
values from its shared mapper path. The port now records bounded context facts
(correlation/tenant/actor/channel/locale/causation/traceparent/idempotency presence and
lengths) and bounded error-shape facts (variant, text/UUID counts, and opaque payload
presence). Storage failures and variant-not-found diagnostics no longer expose backend
error text or resource identifiers, and every Product `CommerceError` variant maps to a
stable `PortError` envelope.

The ecommerce public-port verifier now reads the Product owner port directly, rejects the
former raw diagnostic patterns, requires the bounded fact helpers, and requires the stable
error envelopes. The active Commerce `/store/products` list was already on the Product-owned
HTTP capability before this slice and was not regressed or reintroduced through the legacy
controller.

Maintainer compiler, runtime, gatekeeper, build, and test evidence remain unrun by the agent.

## 2026-09-25 Storefront auxiliary HTTP error safety

The mounted Commerce storefront controller still contained a shared auxiliary HTTP error
boundary that serialized raw `PortError` and generic `Debug` error values while serving
region and shipping-option requests. The helper now records only bounded owner/error-kind,
retryability, tenant/cart presence, code-length, status, and stable operation facts. The
generic public-error helper no longer requires `Debug` and no longer serializes its error
value. The Product compatibility mapper in the same file was hardened to the same rule.

The ecommerce public-port safety verifier now reads the mounted storefront controller and
rejects the former raw diagnostic patterns while requiring the bounded helper contract.

Maintainer compiler, runtime, gatekeeper, build, and test evidence remain unrun by the agent.

## 2026-09-25 Storefront Cart and Order HTTP error safety

Mounted Commerce storefront Cart and Order transports still had raw owner diagnostics in
shared HTTP mappers. Cart now logs only bounded owner kind/code-length, retryability,
tenant/cart identity presence, and public status. Order Customer, Order read, return-command,
and Payment refund mappers now retain bounded correlation/context facts and owner error shape
without serializing raw `PortError`, tenant/user/customer/order identifiers, actor/channel
values, internal codes, or internal messages.

The ecommerce public-port verifier now includes these mounted Cart and Order controller files
and rejects the former raw diagnostic patterns.

Maintainer compiler, runtime, gatekeeper, build, and test evidence remain unrun by the agent.

## 2026-09-25 Admin Order audit correction

The repository mounts `controllers/admin/orders_owner_ports.rs` for the Admin Order routes;
`controllers/admin/orders.rs` is legacy and unmounted. An accidental edit to the legacy file
was reverted, so no legacy command migration is claimed.

The active controller already used `OrderAdminCommandPort` for mark-paid, ship, deliver, and
cancel. The real active defect was its generated per-request idempotency key. The command
context now requires the caller-provided `Idempotency-Key` header, while read contexts carry
no synthetic idempotency key. The order/fulfillment and order-detail verifiers were aligned
to the mounted controller.

Maintainer compiler, runtime, gatekeeper, build, and test evidence remain unrun by the agent.

## 2026-09-25 Storefront line-item resolution error safety

Mounted storefront line-item resolution still serialized raw Product/Pricing `PortError`
and Inventory `CommerceError` values at its HTTP boundary. The Product and Pricing mappers
now retain bounded owner kind/code-length/retryability and safe request/resource shape facts;
Inventory now records only bounded error-kind and identity/channel/locale presence facts.
Public HTTP envelopes remain stable.

The ecommerce public-port verifier now guards the line-item resolution controller against
the former raw owner error/context patterns.

Maintainer compiler, runtime, gatekeeper, build, and test evidence remain unrun by the agent.

## 2026-09-25 Admin checkout-operation diagnostic safety

Mounted Admin checkout-operation HTTP mapping used a redacted `Debug` wrapper and
serialized shape-labeled identity fields. The shared mapper now takes no error value at all
and emits only explicit tenant/actor/operation/payment/order/reservation state facts plus
stable public policy data. The verifier now rejects the former redacted/raw patterns.

Maintainer compiler, runtime, gatekeeper, build, and test evidence remain unrun by the agent.

## 2026-09-25 Active Admin Order idempotency and cart lifecycle typing

The mounted Admin Order controller was already using `OrderAdminCommandPort`, but its
shared context helper generated a new UUID per request and therefore could not preserve
caller-owned replay identity. Read contexts now carry no idempotency key; all four write
handlers require and propagate the caller's `Idempotency-Key` header.

Active Admin Order HTTP diagnostics also now retain only bounded error-code length rather
than serializing the internal code value. Separately, the storefront payment-collection
guard now parses `CartResponse::lifecycle_status()` and checks `CartStatus::Completed`,
failing closed when an unknown persisted status is encountered.

The Admin Order and order-detail verifier scripts were corrected to inspect the mounted
`orders_owner_ports.rs` controller rather than the unmounted legacy `orders.rs` source.

Maintainer compiler, runtime, gatekeeper, build, and test evidence remain unrun by the agent.

## 2026-09-25 Caller-owned Payment and Fulfillment idempotency

Mounted Admin Fulfillment write endpoints previously generated resource/operation or
payload-hash-derived idempotency keys. All six writes now require a caller-owned
`Idempotency-Key` and propagate it into the fulfillment owner command context.

Mounted Admin Payment collection transitions and refund transitions had the same
resource/operation-derived replay identity. They now require caller-owned keys; refund
creation already used a validated caller key and remains on that path. Internal payment and
fulfillment error codes are also represented only by bounded length facts at the HTTP boundary.

The ecommerce public-port verifier now guards both controllers against synthetic replay
identity and raw internal error-code diagnostics.

Maintainer compiler, runtime, gatekeeper, build, and test evidence remain unrun by the agent.

## 2026-09-25 Storefront return idempotency contract

Storefront order-return creation previously generated an idempotency UUID inside its
command context. It now requires a caller-owned `Idempotency-Key`, validates the same 191
byte contract used by the other commerce writes, and propagates the key to the Order
post-order command port. The endpoint's OpenAPI contract now declares the required header.

Admin Order, Fulfillment, and Payment write endpoints updated in this review similarly
declare their caller-owned `Idempotency-Key` in OpenAPI, keeping the documented transport
contract aligned with runtime enforcement.

Maintainer compiler, runtime, gatekeeper, build, and test evidence remain unrun by the agent.
## Completed Rounds Archive
_No completed rounds yet. Round 1 is currently in progress._


### FS-01 Result

**Implemented:** `resolve_database_uri` now logs only the fallback source, never the URI; `ServerRuntimeContext` now atomically initializes `ModuleEffectivePolicyCache` through a typed get-or-insert operation.

**Verification state:** Tests/runtime verification were not run by the agent, per the maintainer-owned test rule. The phase is ready for PR integration; after merge, `main` must be refreshed before FS-02 begins.


### FS-02 Pre-Implementation Audit Findings

- [x] **SERVER-COMP-01 — registry v2 routing requires tenant resolution for global registry operations.** `tenant_route_scope` currently classifies `/v2/catalog/*` as `TenantBound`, but registry publish/governance and remote-runner handlers intentionally use platform-scoped command contexts; the remote runner token has no tenant context. Only `/v2/catalog/publish/{request_id}/platform-build-stage` explicitly binds a tenant-scoped platform build. In full runtime this can reject legitimate registry runner/global operations before registry authorization executes.
- [x] **SERVER-COMP-02 — duplicated route-policy condition.** `tenant_route_scope` contains the same `/catalog` descendant predicate twice. The second branch is unreachable and obscures the actual global-route contract.
- [x] **SERVER-COMP-03 — remote executor token uses non-constant-time comparison in the controller.** The registry middleware already performs constant-time runner-token authentication, but `require_remote_executor_access` rechecks the same secret with ordinary string inequality. Keep defense-in-depth while using the same constant-time comparison semantics at both boundaries.


### FS-02 Result

**Implemented:** registry route scope now separates platform-global registry operations from the single tenant-bound platform-build staging operation; the duplicate `/catalog` predicate was removed; controller-side remote-runner token validation now uses constant-time comparison to match the registry middleware.

**Verification state:** Tests were inspected but not executed by the agent. Maintainer verification remains required. FS-02 implementation is ready for integration.


### FS-03 Pre-Implementation Audit Findings

- [x] **FOUNDATION-01 — module registry contract accepts unmanifested runtime modules.** `validate_module_registry_contract` verifies manifest entries are present in the runtime registry, but never rejects registry entries absent from the manifest. Because the manifest is the declared deployment composition, an extra runtime module can therefore pass the contract and expose runtime capabilities not represented by the active manifest.


### FS-03 Result

**Implemented:** the stable module-registry contract now requires the runtime registry set to be a subset of the declared `modules.toml` set; unmanifested runtime entries fail validation with a typed contract error. A regression test covers the extra-runtime-entry case.

**Verification state:** Tests were not run by the agent. Maintainer execution remains required. FS-03 implementation is ready for integration.


### FS-04 Pre-Implementation Audit Findings

- [x] **WORKER-01 — EventDispatcher queue depth is unbounded despite a max_queue_depth contract.** `DispatcherConfig` exposes `max_queue_depth`, but `EventDispatcher::start` spawns one Tokio dispatch task per received event and only limits per-handler concurrency with a semaphore. Under sustained load, dispatch tasks can accumulate while waiting for handler permits, so memory/task pressure is not bounded by the configured queue depth. The fix must enforce queue admission before task creation and preserve explicit backpressure behavior rather than silently growing an in-process queue.


### FS-04 Result

**Implemented:** `EventDispatcher` now performs bounded admission before receiving from the broadcast stream, holds the queue permit through all matching handler completion, and treats zero configuration as a safe minimum capacity of one. A regression test covers the configured queue bound.

**Verification state:** Tests were not run by the agent. Maintainer execution remains required. FS-04 implementation is ready for integration.


### FS-05 Pre-Implementation Audit Findings

- [x] **CORE-01 — RBAC revocation-fence audit passed.** The repository documentation states that request permission snapshots do not establish a revocation fence. The next check is to verify every mutation-capable RBAC path reads current persisted grants/generation after transaction boundaries and cannot authorize from a stale cache snapshot.
- [x] **CORE-02 — cache generation/recovery audit passed.** Cache module exposes durable invalidation, generation and bounded dedupe facilities; verify replay, gap detection, stale-event rejection, and Redis reconnect behavior cannot move a namespace generation backwards or permanently suppress a newer invalidation.
- [x] **CORE-03 — tenant lifecycle transaction/idempotency audit passed.** Verify tenant activation/deactivation and related membership/settings changes cannot commit state while leaving authorization/cache invalidation or lifecycle events permanently stale.


- [x] **AUTH-01 — secret-bearing auth types exposed credentials through derived `Debug`.** `AuthConfig` contains the HS256 secret and optional RSA private key; `AuthSettingsOverrides` can contain the same key material; `PasswordResetEmail` carries a reset URL containing a bearer token; `OAuthAppSecretResult` carries a client secret. Their derived `Debug` implementations can emit credentials if an error, trace or diagnostic path logs these values.


- [x] **EMAIL-01 — SMTP password was exposed by derived `Debug`.** `SmtpConfig` carries the SMTP credential and derives `Debug`; `EmailConfig` derives `Debug` transitively through the nested SMTP config. Any diagnostic dump of configured email settings can therefore disclose the SMTP password.


### FS-05 Result

**Implemented:** redacted `Debug` output for `AuthConfig`, `AuthSettingsOverrides`, `OAuthAppSecretResult`, `PasswordResetEmail`, `EmailConfig`, and `SmtpConfig`. Added regression tests asserting that secret/key/token/password material is absent from the rendered debug representation.

**Audit passes:** RBAC current-permission resolution uses persisted tenant-scoped relations and generation-aware cache publication; durable cache generation/consumer state is monotonic and acknowledgement-gated; tenant locale policy replacement is revision-checked, idempotent and transactionally event-published; direct auth access tokens re-check active sessions; search/index/email remain owner-composed rather than server-owned domain paths.

**Verification state:** Tests were not run by the agent. Maintainer execution remains required. FS-05 implementation is ready for integration.


### FS-06 Pre-Implementation Audit Findings

- [x] **COMMERCE-01 — commerce cross-tenant mutation audit passed.** Verify every cart/customer/product/pricing/inventory/order/payment/fulfillment mutation predicates all persisted reads/writes by the trusted tenant and never by client-supplied entity ids alone.
- [x] **COMMERCE-02 — money/order lifecycle invariant audit passed, with COMMERCE-04 remediated.** Verify currency/amount arithmetic, status transitions, idempotency and capture/refund/fulfillment event ordering cannot produce duplicate or impossible financial state.
- [x] **COMMERCE-03 — channel visibility audit passed; inventory reservation race remediated as COMMERCE-05.** Verify concurrent cart/order operations cannot oversell or expose products outside the active tenant/channel policy.


- [x] **COMMERCE-04 — capture uses order amount after partial authorization.** `PaymentProviderRegistry` explicitly permits partial authorization, and `capture_collection` correctly limits local capture to `collection.authorized_amount`, but checkout `capture_provider.rs` constructs the external capture request with `request.identity.amount`. A partially authorized collection therefore asks the provider to capture more than the amount authorized by that provider operation. The external request must use the persisted authorized amount as its financial authority.


- [x] **COMMERCE-05 — exported legacy inventory reservation has a read-modify-write race.** `InventoryService::reserve` reads `reserved_quantity`, computes availability, then writes the stale value back. Concurrent reservations on the same inventory level can overwrite one another and return misleading availability. The legacy public path must use the same database-guarded increment semantics as the identity reservation port.


### FS-06 Result

**Implemented:** checkout capture now sends the persisted provider-authorized amount to the external provider, preserving the documented partial-authorization contract. The exported legacy `InventoryService::reserve` path now uses a database-guarded atomic increment instead of stale read-modify-write, and returns post-update availability.

**Audit passes:** commerce mutations use tenant-scoped owner queries; payment amounts/statuses/refunds are backed by service-level transactions and database lifecycle/capacity guards; payment provider operations are journaled/idempotent and reconcile uncertain outcomes; checkout identity is durable and cross-boundary validated; channel-specific storefront inventory visibility is separated from tenant inventory accounting.

**Verification state:** Tests were inspected/added but not executed by the agent. Maintainer execution remains required. FS-06 implementation is ready for integration.


### FS-07 Pre-Implementation Audit Findings

- [x] **CONTENT-01 — content ownership/soft-delete boundary audit passed.** Verify all content/taxonomy/profile/group/comment mutations and reads are tenant-scoped and do not expose soft-deleted or foreign-owner records through alternate lookup paths.
- [x] **CONTENT-02 — translation fallback/locale identity/revision audit passed.** Verify locale keys, fallback chains, revisioning and update/delete paths cannot return another tenant's translation or silently overwrite a concurrent revision.
- [x] **CONTENT-03 — social graph/reaction/moderation invariant audit passed.** Verify duplicate reactions/follows/memberships, authorization edges, moderation state transitions and idempotency remain atomic under retries/concurrency.


- [x] **CONTENT-04 — content state machine loses original creation timestamp across valid transitions.** `ContentNode<Draft>::publish` and `ContentNode<Published>::unpublish` reconstruct state without carrying the original `created_at`; `Archived::restore_to_draft` has the same issue. Because `ContentNode` is publicly exported, a valid lifecycle round-trip can silently rewrite a business/audit timestamp.


### FS-07 Result

**Implemented:** `ContentNode` now preserves the original `created_at` through Draft → Published → Draft and Published → Archived → Draft lifecycle transitions. Regression coverage was added for the full lifecycle round-trip.

**Audit passes:** content/comment/group reads and mutations consistently carry tenant ownership and soft-delete constraints; taxonomy category hierarchy mutations use tenant-scoped locks, scope checks and revision CAS; taxonomy translation writes use tenant filters and revision CAS; profile privacy evaluates recipient state under the trusted tenant and checks actor identity; social graph commands use tenant-scoped idempotency receipts and revision CAS; reaction commands enforce tenant/actor admission, catalog revision fencing and aggregate/state consistency; moderation application workers use tenant-scoped lease/revision CAS and transactional case/event transitions.

**Verification state:** Tests were inspected and regression coverage was added, but no test suite was executed by the agent. Maintainer execution remains required. FS-07 implementation is ready for integration.


### FS-08 Pre-Implementation Audit Findings

- [x] **PUBLISH-01 — publication visibility boundaries audit passed.** Verify public reads cannot expose draft/archived/restricted records through route aliases, slugs, search indexes, projections or locale fallback.
- [x] **PUBLISH-02 — canonical URLs/aliases/SEO projection audit passed.** Verify tenant/locale uniqueness, retirement semantics, redirect safety and cache invalidation cannot point a public route at another tenant or stale resource.
- [x] **PUBLISH-03 — page-builder/navigation/notifications audit passed.** Verify authored component payloads, navigation trees and notification targets are tenant-scoped, permission-checked, size-bounded and idempotent under retries.


### FS-08 Result

**Implementation:** no repository-owned production defect was confirmed in this phase, so no application-code change was made. The temporary audit ledger is the only phase artifact.

**Audit passes:** blog/page public reads enforce Published state, tenant ownership, locale resolution and channel gates; forum public discovery and search-result eligibility re-evaluate exact audience visibility for topic/reply candidates and retain tenant/revision/deletion boundaries; page canonical routes and aliases are tenant/locale scoped and fail closed on ambiguity, with published-route snapshots and tombstones transactionally maintained; immutable Page Builder artifacts verify tenant/page/locale identity plus payload/build/materialization hashes and bounded resource policy before activation or audit; navigation menu creation/translation/binding uses tenant/channel scope and exact locale coverage with revision CAS; SEO redirect caches are keyed by tenant and invalidate transactionally, target hosts are constrained, redirect chains reject immediate loops, and sitemap jobs/deliveries use tenant-scoped idempotency; notification source inbox/fanout jobs use tenant-scoped identities, leases, bounded pages, cursor-advance proofs and idempotent fanout item keys.

**Verification state:** tests were inspected but not executed by the agent. Maintainer execution remains required. FS-08 implementation is complete and ready for integration.


### FS-09 Pre-Implementation Audit Findings

- [x] **EXT-01 — external/provider URL trust boundaries audit passed.** Check all connector/provider/network-capable modules for SSRF, private-network access, DNS rebinding assumptions, redirect following, credential leakage, and unbounded response/resource use.
- [x] **EXT-02 — capability execution authorization audit passed.** Verify AI/MCP/connector/automation actions are tenant-scoped, permission-checked, bounded by explicit capability allowlists and cannot turn user-controlled metadata into arbitrary privileged tool execution.
- [x] **EXT-03 — external side effects idempotency/retry audit passed.** Verify webhook/connector/provider retries cannot duplicate writes or side effects and that ambiguous outcomes are reconciled without weakening authorization boundaries.


- [x] **EXT-04 — MCP session plaintext token leaks through derived Debug/serialization.** `McpSessionContext` carries `plaintext_token` and derives both `Debug` and `Serialize`/`Deserialize`. Session contexts can therefore expose the authentication bearer material through diagnostics or serialized runtime state. The token must remain in-memory-only and redact from Debug/schema surfaces.


### FS-09 Result

**Implemented:** `McpSessionContext` keeps `plaintext_token` in memory only, omits it from JSON/schema output, and redacts it from Debug output. Regression coverage was added.

**Audit passes:** MCP tool authorization is policy/permission based with unknown tools denied; Alloy authoring requires authenticated tenant-matching scripts.manage context and resolves the tenant server-side; imported Alloy drafts fail closed when parent policy is unavailable; AI tool inventory/context/tool-call bounds are enforced and untrusted provider/tool data cannot create authority; agent permissions are an initiator/agent/descriptor intersection; provider targets are deployment-owned and their endpoints pass deployment egress policy; structured AI accounting uses tenant-scoped budgets, provider concurrency, leases and classification gates; Iggy producers partition by tenant and consumer acknowledgement is bound to stream/topic/partition plus the outstanding delivery; connector retries/DLQ preserve raw payload without widening authorization.

**Additional observation for later phase:** Flex persisted-schema presentation currently maps malformed stored \`fields_config\` to an empty view in \`standalone_schema_view_from_source\`; this is a storage-corruption resilience concern and is deferred to FS-14 rather than silently changed here.

**Verification state:** tests were inspected and regression coverage was added but not executed by the agent. Maintainer execution remains required. FS-09 implementation is ready for integration.


### FS-10 Pre-Implementation Audit Findings

- [x] **UI-01 — auth admin transport DTOs expose bearer/password secrets through derived Debug.** `ApiRequestContext`/`ServerGraphqlRequest` contain the caller token and derive `Debug`; `CreateUserInput` contains a plaintext password and derives `Debug`. The UI transport contract must preserve serialization for requests but diagnostic formatting must redact credentials.


### FS-10 Result

**Implemented:** added root `AUDIT_PLAN.md` as the single human entry point to the canonical trigger/ledger; redacted bearer tokens and GraphQL variables from auth admin transport Debug output; redacted user passwords from `CreateUserInput`; redacted OAuth client secrets from `CreateOAuthAppResult` Debug output.

**Audit passes:** module-owned admin/storefront transport layers were checked for auth/tenant propagation and owner-port usage. Commerce admin server functions resolve authenticated `AuthContext`/`TenantContext` and compare request tenant ids where supplied; auth admin mutations construct server-owned mutation contexts from resolved auth/tenant state and delegate to the owner port; page-builder, forum, product, tenant, order, payment and related module UI seams were reviewed for direct persistence or trust-boundary bypasses. No additional repository-owned production authorization bypass was confirmed in this phase.

**Verification state:** no test suite/build was executed by the agent. Maintainer execution remains required. FS-10 implementation is ready for integration.


### FS-11 Findings — 2026-09-27

- [x] **LEPTOS-01 — SSR server-function trust-boundary audit passed.** Protected mutations derive auth/tenant from server context; public storefront server functions do not treat browser identity parameters as authorization authority.
- [x] **LEPTOS-02 — protected server mutation audit passed.** Authenticated blog comments and forum read-state mutations verify canonical `AuthContext` + `TenantContext`, permission/audience/revision boundaries and owner services; the full HttpOnly/CSRF migration is separately tracked in the accepted ADR for FS-13.
- [x] **LEPTOS-03 — SSR/hydration data ownership audit passed.** Public storefront rendering uses tenant/channel/locale scoped owner reads and bounded public projections; no authenticated operator state was found embedded into public SSR output.
- [x] **LEPTOS-04 — full HttpOnly browser-session migration captured in accepted ADR; implementation intentionally deferred to FS-13.**
- [x] **LEPTOS-05 — SSR auth snapshot no longer trusts client-controlled cookie identity/role data.** Middleware treats the cookie as an untrusted transport envelope, revalidates its bearer token through the canonical auth transport, and inserts only the verified user into request extensions; `request_auth_snapshot` consumes only that trusted extension.

**Implementation:** `apps/admin/src/app/auth_ssr.rs` and `apps/admin/src/main.rs` implement the verified SSR snapshot middleware and request-extension boundary. `DECISIONS/2026-09-27-leptos-httponly-session-migration.md` defines the complete later migration contract.

**Verification state:** tests/builds were not run by the agent. Maintainer execution remains required. FS-11 implementation is ready for integration.


### FS-12 Pre-Implementation Audit Findings

- [x] **NEXT-01 — Next.js server/client trust-boundary audit passed.** Verify browser-provided tenant, user, role and provider data cannot become server authority, and server actions route mutations through canonical backend owner boundaries.
- [x] **NEXT-02 — proxy/middleware/auth/caching audit passed; reusable bearer exposure recorded as NEXT-05.** Verify auth/session cookies, proxy rewrites, cache headers, route handlers and server-side fetches cannot cross tenant/session boundaries or cache authenticated data publicly.
- [x] **NEXT-03 — GraphQL/REST/SEO data-loading audit passed.** Verify server components, route handlers and metadata generation use tenant/locale context from trusted request state, avoid secret leakage in HTML, and preserve fail-closed authorization semantics.


- [x] **NEXT-04 — SEO JSON-LD serialization created an inline-script XSS sink.** `buildSeoStructuredDataScripts` uses `JSON.stringify` directly for backend-provided `structuredDataBlocks.payload` and renders the result through `dangerouslySetInnerHTML`. JSON permits the literal `<` character, so a payload containing `</script><script>…` can terminate the JSON-LD script element before the browser sees the data as JSON. The serializer must emit script-safe JSON (at minimum escape `<`, `>`, `&`, U+2028 and U+2029).


- [ ] **NEXT-05 — NextAuth exposes the RusToK bearer to client JavaScript.** auth.ts stores rustokToken in the NextAuth JWT and copies it into session.user.rustokToken; useSession() therefore exposes the reusable backend access token to browser code. The complete fix requires migrating all client transport consumers to a server-owned session/proxy contract and is tracked in the accepted browser-auth ADR for FS-13. No partial removal is applied in FS-12 because it would break current client transport or create a split trust model.


### FS-12 Result

**Implemented:** `apps/next-frontend/src/shared/seo/metadata.ts` now emits script-safe JSON-LD by escaping `<`, `>`, `&`, U+2028 and U+2029 before insertion into the inline `<script type="application/ld+json">` element.

**Deferred architecture:** Next.js admin still exposes `rustokToken` through the client-visible NextAuth session. This is explicitly tracked in the accepted browser-auth ADR and deferred to FS-13 so the entire client transport can migrate to a server-owned/HttpOnly model without a split trust architecture.

**Audit passes:** Next-admin proxy/route auth, backend bearer forwarding, tenant propagation, module-enabled navigation, server-owned module mutations, Next storefront fixed-tenant composition, SEO REST/GraphQL fallback error taxonomy, same-origin SEO document fetching, runtime robots/sitemap handling, and server/client component boundaries. Starter routes required by docs all use `notFound()`.

**Verification state:** no tests/builds were run by the agent. Maintainer execution remains required. FS-12 implementation is ready for integration.


### FS-13 Pre-Implementation Audit Findings

- [x] **BROWSER-01 — browser-readable bearer persistence confirmed; full removal deferred by accepted browser-auth ADR to coordinated transport migration.** Shared auth currently permits access/refresh tokens in LocalStorage and NextAuth exposes the backend access token through the client-visible session. This expands any XSS blast radius and duplicates credential authority across browser storage/session layers.
- [x] **BROWSER-02 — HttpOnly/Secure/SameSite/CSRF/rotation/revocation contract accepted in ADR; implementation deferred to coordinated migration.** Moving authority to HttpOnly cookies without a consistent SameSite/CSRF/rotation/revocation contract would create a second class of vulnerabilities. Verify native/GraphQL/browser adapters can share one server-issued session contract.
- [x] **BROWSER-03 — tenant header is metadata only; backend re-resolves/validates tenant authority.** Inspect token/tenant header construction, URL/query helpers and request contexts for client-controlled tenant values that can cross the canonical server tenant resolution boundary.


- [x] **BROWSER-04 — shared browser cookie parser now fails closed on malformed encoding and preserves `=` characters.** `getCookieValue` directly calls `decodeURIComponent` and splits on every `=`. A malformed cookie can throw during auth bootstrap, and values containing `=` are truncated. The parser should fail closed on invalid encoding and split only at the first delimiter.


### FS-13 Result

**Implemented:** `packages/rustok-ui-auth/browser/getCookieValue` now splits cookie pairs at the first structural delimiter only, decodes the complete remaining value, and returns undefined on malformed percent-encoding rather than throwing.

**Architecture boundary:** BROWSER-01/02 are not hidden as “done”. Browser bearer persistence in Leptos LocalStorage and NextAuth client session remains a known architectural finding. The accepted browser-auth ADR makes the required full migration contract explicit and assigns completion to the coordinated shared browser transport migration. No partial token-storage removal was introduced.

**Audit passes:** shared AuthSession Debug redaction, browser API tenant metadata semantics, route query sanitizer/writer, transport retry safety policy, shared AuthError mapping, and client/server separation. The backend remains the authority for tenant and authorization decisions.

**Verification state:** no browser tests/builds were run by the agent. Maintainer execution remains required. FS-13 implementation is ready for integration.


### FS-14 Pre-Implementation Audit Findings

- [x] **STORAGE-01 — destructive/irreversible migration paths audited; no unguarded repository-owned data-loss path confirmed.** Check every migration with Drop/Delete/Truncate/Rename/alter-removal for guarded preconditions, data-preserving rollback and explicit irreversibility where applicable.
- [x] **STORAGE-02 — tenant/owner uniqueness and FK scope audited on high-risk migration set; taxonomy/product/forum/payment/tenant constraints preserve owner scope.** Verify business uniqueness keys include tenant/channel/locale where required, and foreign keys prevent cross-tenant references instead of merely relying on application filters.
- [x] **STORAGE-03 — migration/entity/schema parity audited on high-risk and recent migrations; no confirmed backend/schema mismatch.** Verify entities, DTOs, indexes and runtime assumptions match the actual migrated schema across PostgreSQL, MySQL and SQLite where supported.


### FS-14 Result

**Audit coverage:** inventoried 606 migration blobs across 43 owner modules; performed focused source review of destructive, legacy-retirement, backfill, enforce, normalize and repair migrations, including PostgreSQL/SQLite/MySQL guards where present.

**Findings:** no new repository-owned root-cause defect was confirmed in this phase. The previously deferred Flex persisted-schema corruption behavior remains tracked for this storage phase and requires a separate runtime/schema policy decision; no lossy automatic fallback was introduced.

**Verification state:** migration tests/database upgrade-downgrade runs were not executed by the agent. Maintainer execution remains required. FS-14 is ready for integration.


### FS-15 Pre-Implementation Audit Findings

- [x] **TOOLING-01 — release/build trust boundary and reproducibility audit passed.** Check release packaging/finalization, workflow inputs, generated artifacts and publication/signing for mutable remote inputs, symlinks, unpinned tools, digest drift, secret publication and unsafe filesystem behavior.
- [x] **TOOLING-02 — utility CLI mutation authority/environment guard audit passed, with TOOLING-04 remediated.** Check installer, seed/import/repair commands for implicit production defaults, destructive mutation without explicit operator intent, and output that leaks credentials or raw persisted secrets.
- [x] **TOOLING-03 — generated/release artifact deterministic provenance audit passed.** Verify archive contents, manifest/checksum generation, source materialization, publication receipts and installer distribution receipts cannot silently diverge or package unreviewed local content.
- [x] **TOOLING-04 — standalone seed apply bypassed installer environment policy and reused the admin password for the development customer.** The command mutates through seed ports outside `InstallPlan` preflight, defaults to `Dev`, and passes the same password to both the SuperAdmin and `customer@demo.local`.


### FS-15 Result

**Implemented:** standalone seed apply now requires an explicit `--environment`, rejects production, and enforces that policy during dry runs. The `Dev` profile now requires an independent demo-customer password instead of reusing the administrator password. Regression coverage was added at the CLI command boundary.

**Audit passes:** release packaging/finalization is deterministic and rejects symlinks/unexpected files; release workflows pin action revisions and verify release ancestry, signing, immutability, exact assets, checksums, SBOM/provenance and image digests; source/publication materializers enforce safe paths, create-new semantics and executable identity; installer receipts are signature/digest bound and production preflight rejects plaintext/sample secrets; topology validates exact surface/role ownership; CLI plan/output paths redact secrets.

**Verification state:** tests/builds were not run by the agent. Maintainer execution remains required. FS-15 is ready for integration.


### FS-16 Pre-Implementation Audit Findings

- [x] **LIB-01 — shared error/diagnostic secret and PII audit passed.** Shared libraries are reusable by every module, so Debug/Display/serialization of credential-bearing or request-bearing types must never become a cross-module leakage primitive.
- [x] **LIB-02 — shared context/tenant/auth authority audit passed, with LIB-06 remediated.** Verify helpers distinguish trusted runtime authority from client metadata and do not allow downstream modules to reconstruct security context from transport values.
- [x] **LIB-03 — shared storage/event/web invariant audit passed.** Check shared repository/storage adapters, event envelopes, web helpers and cache primitives for generic behaviors that weaken tenant scope, error stability, transaction ownership or idempotency at call sites.
- [x] **LIB-04 — feature/optional dependency boundary audit passed.** Verify shared crates do not accidentally enable incompatible feature combinations or expose server-only dependencies to browser/transport targets.


### FS-16 Pre-Implementation Audit Finding — Rate Limiter

- [x] **LIB-05 — shared RateLimiter stored raw API keys/login identifiers in bucket keys and Debug output.** `check_api_key` uses `api_key:<raw secret>` as an in-memory bucket key and `check_login` uses `login:<raw identifier>`. `RateLimiter` also derives `Debug`, recursively exposing the bucket map. A diagnostic dump can therefore disclose API credentials and login identifiers. The limiter should use process-local opaque key identities and never render bucket contents.


### FS-16 Pre-Implementation Audit Finding — Request Tenant Authority

- [x] **LIB-06 — shared `RequestContext` bypassed the accepted canonical tenant-resolution boundary.** When `TenantContextExtension` is absent, `RequestContext::from_request_parts` accepts `X-Tenant-ID` directly. This contradicts the accepted strict tenant/request-trust ADR, under which tenant resolution is a server-owned middleware pipeline and downstream request contexts must consume the trusted resolved context rather than reconstruct tenant authority from transport metadata.


### FS-16 Pre-Implementation Audit Finding — Telemetry Cardinality

- [x] **LIB-07 — shared Prometheus metrics exposed unbounded raw `tenant_id` labels.** `rustok-telemetry` uses tenant UUID strings as labels for event publication, span creation, and media upload/delete counters. Tenant cardinality is deployment-scale and unbounded, so series count grows with every tenant and can become a memory/storage/query resource-exhaustion vector. The shared telemetry contract needs a bounded tenant dimension.


### FS-16 Result

**Implemented:** shared `RateLimiter` now stores opaque in-memory identifiers and redacts bucket state from Debug; `RequestContext` now requires the canonical trusted `TenantContextExtension` and no longer reconstructs tenant authority from raw `X-Tenant-ID`; shared tenant-aware Prometheus metrics now use deterministic 256-value `tenant_bucket` labels under the accepted ADR `2026-09-27-bounded-tenant-metric-cardinality.md`.

**Audit passes:** `rustok-api`, `rustok-core`, `rustok-events`, `rustok-runtime`, `rustok-web`, `rustok-telemetry`, and `rustok-fba` were reviewed for secret-bearing Debug/serialization surfaces, request/tenant authority, event envelope validation, storage/runtime path safety, transport error mapping, feature isolation, and dependency direction. Event envelopes do not dump payloads through Debug; AuthContext/TenantContext/ChannelContext consume trusted extensions; `rustok-api` runtime/server features remain directionally isolated; `rustok-core` `redis-cache` is an intentionally empty compatibility feature with no Redis references in cache implementation.

**Verification state:** Tests/builds were not run by the agent. Regression tests were added for the rate limiter, RequestContext and telemetry bucket contract. Maintainer execution remains required. FS-16 implementation is ready for integration.


### FS-17 Pre-Implementation Audit Findings

- [x] **SUPPLY-01 — Cargo lock/dependency graph audit passed.** The 1622-package lockfile has one immutable Athanor git revision; reviewed security-sensitive duplicate families did not reveal an unpinned git source or unsafe source override. Multiple package versions remain governed by the existing reviewed cargo-deny policy. Detect multiple versions of security/serialization/network primitives, git dependencies without immutable revs, path dependencies escaping the workspace, and lockfile entries whose provenance cannot be reconciled to manifests.
- [x] **SUPPLY-02 — JavaScript dependency/provenance audit passed.** All six lockfiles are npm lockfileVersion 3; local links occur only for expected workspace packages; root, admin, storefront and richtext packages have no install lifecycle hooks, while Next admin uses its existing Husky prepare hook. Verify package-lock integrity, workspace/package boundary, postinstall scripts, local file/link dependencies, and build-time downloads do not introduce mutable or unreviewed code execution.
- [x] **SUPPLY-03 — CI/CD action pinning completed.** Third-party GitHub Actions in the audited workflow tree were migrated to full commit SHAs, and scripts/verify/verify-workflow-action-pins.mjs now rejects mutable or unapproved action refs in CI. Every third-party GitHub Action used in release/build/security-sensitive workflows should be pinned to an immutable commit SHA where repository policy requires it; mutable tags/branches are not acceptable for privileged automation.
- [x] **SUPPLY-04 — cargo-deny/toolchain/license/source policy audit passed.** deny.toml explicitly denies unknown registries and git sources, allows only the crates.io registry plus the reviewed Athanor repository, uses the RustSec advisory DB, and rust-toolchain.toml declares the stable toolchain. Verify `deny.toml`, `rust-toolchain.toml`, advisory/license/source policies and repository scripts actually constrain the dependency graph they claim to govern.


### FS-17 Result

**Implemented:** all audited third-party GitHub Actions are pinned to immutable commit SHAs; a permanent CI verifier enforces the approved pin set; dependency-audit tooling uses exact versions (cargo-audit 0.22.2 and cargo-outdated 0.19.0) instead of floating latest installs; temporary migration workflow/script were removed after cutover.

**Dependency evidence:** Cargo.lock contains one immutable Athanor git revision and no known removed malicious crates tracing_checks or tracings; h2 0.4.16 is the locked version and is patched for the August 2026 RustSec advisory. JavaScript lockfiles are npm lockfileVersion 3, with workspace-only local links where expected.

**Verification state:** tests/builds were not run by the agent, per the maintainer-owned test policy. Static source/lock/workflow audits were completed and FS-17 is ready for integration.


### FS-18 Pre-Implementation Audit Findings

- [x] **RUNTIME-01 — request body/resource boundary audit passed.** Verify every externally reachable JSON/form/file/WebSocket endpoint has explicit bounded body/frame/time/resource controls, including endpoints bypassing the main GraphQL/REST router.
- [x] **RUNTIME-02 — production panic/fail-closed audit passed.** Audit `unwrap`/`expect`/assertions in handlers, extractors, deserializers and background request-adjacent services; unknown/malformed state must fail closed with stable errors.
- [x] **RUNTIME-03 — async executor/blocking-I/O audit passed.** Verify all synchronous heavy I/O has a bounded blocking boundary or dedicated worker ownership, and that request cancellation propagates to child work.
- [x] **RUNTIME-04 — file/static/WebSocket trust-boundary audit passed.** Verify path normalization/traversal, symlink handling, range/size limits, WebSocket origin/auth checks, and disconnect cleanup.
- [x] **RUNTIME-05 — server error/log/status audit passed, with RUNTIME-07 remediated.** No generic `Debug`/request metadata should cross public response/log boundaries; HTTP status must never be derived from untrusted numeric values.


- [x] **RUNTIME-06 — GraphQL WebSocket input queue was unbounded.** `handle_graphql_ws` uses `tokio::sync::mpsc::unbounded_channel` between the network read task and `async_graphql::http::WebSocket`. A peer can send valid WebSocket messages faster than the schema consumes them, causing unbounded queued `String` allocations. The transport must apply a bounded channel and explicit frame/message size limits so backpressure reaches the socket rather than accumulating memory.


- [x] **RUNTIME-07 — server rate-limit debug logging exposed raw rate-limit identity keys.** `rate_limit_base::rate_limit_for_paths` logs `rate_limit_key` verbatim. Depending on policy, the key includes client IP plus trusted tenant UUID and OAuth application UUID. These are privacy-sensitive identifiers and the debug path can leak them into application logs. The log must use only a stable non-reversible fingerprint and policy metadata.


- [x] **RUNTIME-08 — public email-verification request endpoint bypassed the dedicated auth rate-limit policy.** `/api/auth/verify/request` can enqueue a verification email for a target address but `init_rate_limit_layers` only assigns the stricter auth limiter to login/register/reset paths. The endpoint therefore falls back to the general `/api/` limiter, weakening anti-abuse protection for a direct email-sending side effect.


- [x] **RUNTIME-09 — `RUSTOK_DEMO_MODE` could expose password-reset/email-verification bearer tokens in production responses.** Auth controllers directly read `RUSTOK_DEMO_MODE` and return generated reset/verification tokens when set, but the shared production-environment validation does not constrain this flag. An accidental production environment setting therefore turns an otherwise out-of-band email flow into a credential-bearing API response.


### FS-18 Result

**Implemented:** GraphQL WebSocket transport now has explicit 256 KiB frame/message bounds and a bounded 32-message input queue with backpressure; rate-limit diagnostics use a short SHA-256 fingerprint instead of raw IP/tenant/OAuth identities; email verification requests share the dedicated auth rate-limit namespace; and demo reset/verification token exposure is centrally disabled whenever the environment is production.

**Audit passes:** framework body extractors remain bounded by their default/request-specific limits; request/persisted-data panic candidates are either test-only or protected by validated invariants, with public malformed data mapped to stable errors; server request handlers do not perform synchronous filesystem/process work; artifact/static paths are admission-bound and downloads use storage keys rather than client filesystem paths; GraphQL WebSocket auth is token-bound, tenant-bound and revalidated against the original RBAC scope; observability endpoints are protected by bearer authorization and bounded readiness payloads; public error mappings use typed statuses and safe messages.

**Verification state:** tests/builds were not run by the agent. Regression tests were added for WebSocket transport bounds, rate-limit log fingerprints, auth-rate-limit coverage, and production-safe demo token policy. Maintainer execution remains required. FS-18 implementation is ready for integration.


### FS-19 Pre-Implementation Audit Findings

- [ ] **ARCH-01 — audit-plan/source-of-truth duplication requires reconciliation.** Root `AUDIT_PLAN.md`, ACRE documentation, and the living full-stack ledger may contain overlapping or stale instructions. There must be one canonical trigger and one canonical phase-progress source without contradictory phase numbering or obsolete workflow text.
- [ ] **ARCH-02 — architecture/module dependency direction requires a fresh graph audit.** Verify shared libraries do not depend upward on app/server or specific feature modules, module manifests do not encode reciprocal runtime dependencies, and generated/source registries cannot create cycles.
- [ ] **ARCH-03 — generated registries and metadata require source-of-truth reconciliation.** Check `cli-registry.toml`, module manifests, generated route/registry artifacts, ADR indexes and other committed generated surfaces for drift against their generators/canonical owners.
- [ ] **ARCH-04 — stale compatibility/legacy paths require final cutover audit.** Search for public APIs, compatibility shims, deprecated names and old terminology left reachable after the repository’s documented cutovers.
- [ ] **ARCH-05 — unresolved TODO/placeholder/dead path risk requires closure.** Review production TODO/FIXME/panic placeholders and unreachable/dead compatibility code; remove or explicitly register anything that remains necessary.


### FS-19 Pre-Implementation Audit Finding — Source Layout

- [x] **ARCH-06 — production commerce service stitched a source file with `include!`.** `crates/modules/rustok-commerce/src/services/checkout_payment_stages.rs` creates a nested `legacy` module by `include!("checkout_payment_stages_legacy.rs")`. This is explicitly forbidden by the repository ACRE contract because it bypasses normal Rust module boundaries and hides ownership/dependency structure from tooling. The existing sibling file must be mounted as a normal `#[path] mod` without changing its API.


### FS-19 Pre-Implementation Reconciliation Finding

- [x] **ARCH-07 — phase table was stale for FS-00 through FS-13.** The live plan history shows those phases already integrated into `main`, but the phase-order table still marks many of them `[ ]`. The single living ledger therefore gives a false “unfinished” state and cannot reliably serve as the one command-driven continuation point.


### FS-19 Result

**Implemented:** the production commerce payment-stage compatibility implementation no longer uses `include!`; the legacy file declares its shim dependencies explicitly and is attached through a normal `#[path] mod` boundary. The living phase table was reconciled so FS-00 through FS-18 reflect their already-integrated state.

**Audit passes:** `modules.toml` contains 53 module entries with no missing or cyclic `depends_on` edges; Cargo.lock shows no shared-library dependency on `rustok-server`, `rustok-admin`, or `rustok-storefront`; `AUDIT_PLAN.md`, ACRE and the living ledger agree on the single trigger and plan authority; generated CLI registry output matches the declared root/module provider sources; intentional legacy guards remain explicitly wired where they serve active compatibility boundaries; superseded ADRs are registered as superseded rather than silently rewritten.

**Verification state:** tests/builds/generators were not executed by the agent. Static source and metadata checks only. FS-19 implementation is ready for integration.


### FS-20 Release-Readiness Handoff

**Audit cycle state:** all phases FS-00 through FS-19 are integrated and marked complete in this living ledger. The deep cycle has no remaining unchecked production finding entries. FS-20 is documentation/verification handoff only; no product-code change is required by the audit result. The handoff itself is now the final completion artifact for this cycle.

**Fresh main baseline:** `fa47d64d6faee9507891d0bca145b4672c39dfc9`.

**Maintainer verification matrix — not executed by the agent:**

| Area | Suggested verification | Agent status |
|---|---|:---:|
| Core/shared security fixes | targeted `cargo test` for rustok-core, rustok-api, rustok-telemetry, rustok-auth, rustok-email, rustok-mcp | not run |
| Commerce fixes | targeted `cargo test` for rustok-payment, rustok-inventory, rustok-content, rustok-commerce | not run |
| Server runtime fixes | targeted `cargo test` for rustok-server, including GraphQL WS, auth-rate-limit, settings/demo-policy cases | not run |
| CLI/installer fix | targeted `cargo test -p rustok-installer-cli` for seed environment/credential boundary | not run |
| Workspace compilation | maintainer `cargo check --workspace` / release-profile check appropriate to deployment | not run |
| Rust test suites | maintainer workspace/package test matrix | not run |
| ADR registry | `npm run verify:adrs` | not run |
| Generated CLI registry | `npm run verify:cli-registry` | not run |
| Workflow pinning | `node scripts/verify/verify-workflow-action-pins.mjs` | not run |
| Existing API/runtime verifiers | applicable `scripts/verify/*` contracts touched by the audited areas | not run |
| Dependency audits | repository-declared Cargo/npm advisory and unused-dependency checks | not run |

**Evidence limitations:** the agent performed source-level inspection, repository metadata analysis, dependency/manifest graph inspection, static diff review and targeted regression-test authoring. No test suite, compiler, formatter, cargo-deny/cargo-audit, npm verifier, generator or runtime environment was executed. Therefore the audit establishes code-level findings/remediations and architecture reasoning, not runtime pass/fail evidence.

**Post-merge baseline rule:** after this handoff is merged, the repository's `main` contains the completed audit cycle and the next invocation of `реализуй план аудита` must start a new audit round rather than re-entering FS-00..FS-20. Any new phase/round should create its own dated ledger section and preserve this cycle as historical evidence.

**Cycle completion criteria:**
- [x] Single canonical trigger documented in `AUDIT_PLAN.md`, ACRE and `AGENTS.md`.
- [x] Single living audit-progress ledger reconciled.
- [x] All FS-00..FS-19 phases integrated into `main` through dedicated branches/PRs.
- [x] No unchecked production finding remains in the completed phase blocks.
- [x] All known test/build/verification gaps explicitly handed to the maintainer.
- [x] Final FS-20 handoff commit is the release-readiness merge for this audit cycle.
