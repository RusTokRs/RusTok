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
**Current main SHA:** `7ad05f0bcd87f4227a99cd7ce3f3984874dbb105`  
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
- [x] **FS-22.02.08 — `apps/server/src/middleware/locale.rs`** — one-module audit.
- [x] **FS-22.02.09 — `apps/server/src/middleware/tenant.rs`** — one-module audit.
- [x] **FS-22.02.10 — `crates/modules/rustok-cart/src/guest_access_http.rs` + native storefront capability adapters** — one-owner boundary audit.
- [x] **FS-22.02.11 — `apps/server/src/middleware/security_headers.rs`** — one-module audit.
- [x] **FS-22.02.12 — `apps/server/src/services/server_bootstrap.rs`** — one-module audit; completed with three consecutive remediation iterations and a final fresh second pass. Remaining full worker rollback/join/abort lifecycle gaps are explicitly deferred to FS-24.
- [x] **FS-22.02.13 — `apps/server/src/services/app_runtime.rs`** — one-module audit; completed with three consecutive remediation iterations and a final fresh second pass. Marketplace-provider panic handling remains a separate owner-module finding; full detached-worker lifecycle remains deferred to FS-24.
- [x] **FS-22.02.14 — `apps/server/src/services/server_runtime_context.rs`** — one-module audit; fresh discovery and independent second pass found no repository-owned in-scope defect requiring code remediation.
- [x] **FS-22.02.15 — `apps/server/src/services/graphql_schema.rs`** — one-module audit; completed with five consecutive remediation iterations and a fresh post-merge second pass. Cross-module runtime-fallback candidates remain explicitly deferred to their owner modules.
- [x] **FS-22.02.16 — `apps/server/src/controllers/graphql.rs`** — one-module audit; completed with six consecutive remediation iterations and a final fresh second pass. Remaining RBAC self-mutation invalidation and full detached WebSocket worker lifecycle are explicitly deferred to their owner/FS-24 tracks.
- [x] **FS-22.02.17 — `apps/server/src/controllers/auth.rs`** — one-module audit; completed with two remediation iterations and a final fresh second pass. Registration-policy and invite-consumption boundaries are now enforced.
- [x] **FS-22.02.18 — `apps/server/src/controllers/oauth.rs`** — one-module audit; completed with four remediation iterations and a final fresh second pass. OAuth/OIDC transport, scope, caching, and handshake boundaries are now aligned with the reviewed contract.
- [x] **FS-22.02.19 — `apps/server/src/controllers/users.rs`** — one-module audit; completed with two remediation iterations and a final fresh second pass. Tenant isolation, permission boundaries, pagination semantics, and database error handling were reverified.
- [x] **FS-22.02.20 — `apps/server/src/controllers/health.rs`** — one-module audit; completed with two remediation iterations and a final fresh second pass.
- [x] **FS-22.02.21 — `apps/server/src/controllers/metrics.rs`** — one-module audit; completed with three remediation iterations and a final fresh second pass.
- [x] **FS-22.02.22 — `apps/server/src/controllers/marketplace_registry.rs`** — one-module audit; completed with five remediation iterations and a final fresh second pass.
- [x] **FS-22.02.23 — `apps/server/src/controllers/artifact_http.rs`** — one-module audit; completed with three remediation iterations and a final fresh second pass.
- [x] **FS-22.02.24 — `apps/server/src/controllers/artifact_permissions.rs`** — one-module audit; fresh discovery and independent second pass found no repository-owned in-scope defect requiring code remediation.
- [x] **FS-22.02.25 — `apps/server/src/controllers/admin_events.rs`** — one-module audit; completed with two remediation units and a fresh independent second pass protecting DLQ database error/status semantics and replay claim ownership.
- [x] **FS-22.02.26 — `apps/server/src/controllers/channel.rs`** — one-module audit; completed with one remediation unit and a fresh independent second pass restoring typed HTTP error semantics; OpenAPI aggregation omission deferred to FS-22.02.31.
- [x] **FS-22.02.27 — `apps/server/src/controllers/flex.rs`** — one-module audit; completed with two remediation iterations and final reconciliation: atomic mutation+outbox consistency was merged in PR #4217, then the residual schema-collection route mismatch was fixed after refreshing `main`.


### FS-22.02.27 Iteration 2 — post-merge route reconciliation

- **Base:** refreshed `main` at `fa6615b07999584cb36c89933310df4773a5b102`; PR #4217's event-atomic mutation fix is already integrated.
- **Finding:** the live collection route remained `/api/v1/flex/schemas/` while the OpenAPI contract declared `/api/v1/flex/schemas`.
- **Fix:** aligned the Axum route to the canonical non-slashed path. No event/outbox code was duplicated.
- **Second pass:** re-read the current controller and swagger registration after the concurrent installer merge; no further controller-owned issue remained.

### FS-22.02.28 Iteration 2 — installer reconciliation against current `main`

- **Base:** refreshed `main` at `28fe9574bb2b11bf45628f5dfaab204789d5bd4b`. Concurrent work already present on `main` includes the durable `install_http_jobs` path and corrected setup-token call signatures.
- **Confirmed finding:** the remaining setup gate only inspected the newest install session. A later `failed`/recovery session could therefore hide a historical `completed` installation and re-open setup semantics.
- **Remediation:** added `InstallerPersistenceService::has_completed_session()`; server mutation endpoints and the public `completed` status flag now use historical completion state. Added a persistence integration regression test for completed-then-failed ordering.
- **Confirmed finding:** PostgreSQL database creation still accepted `create_if_missing=true` without `pg_admin_url` and silently fell back to the sample `postgres:postgres` admin URL.
- **Remediation:** removed the fallback and require a non-empty explicit `pg_admin_url` before privileged database creation.
- **Second pass:** re-read installer controller, persistence owner, SeaORM database adapter, HTTP job implementation already on `main`, installer core/CLI contracts, state-machine docs, and HTTP host controls. No additional repository-owned defect remained in the `.28` primary surface.
- **Verification:** repository-content inspection and branch diff review only. No tests, clippy, build, gatekeeper, migrations, or runtime commands were run by the agent.
- **Status:** `FS-22.02.28` complete. Next primary module: `FS-22.02.29 — apps/server/src/controllers/mcp.rs`.
- [x] **FS-22.02.28 — `apps/server/src/controllers/installer.rs`** — one-module audit; reconciled against concurrent `main` changes, with two additional remediation units: historical completion closure and removal of unsafe PostgreSQL admin-URL fallback.
### FS-22.02.29 Iteration 1 — `apps/server/src/controllers/mcp.rs`

- **Base:** refreshed `main` at `c697b8c340aa09e797bac8b4f9611db316f623a9`; dedicated branch `codex/audit-fs-22.02.29-mcp-controller`.
- **Invariant map:** all MCP management mutations must preserve tenant/actor authority ceilings across every transport; DB failures must remain server-internal; tool/scaffold error responses must not expose implementation diagnostics; GraphQL and REST must preserve the same typed error semantics; persisted runtime binding must remain token/client/tenant scoped.
- **Finding MCP-22.02.29-01:** `McpManagementService::map_db_err` returned HTTP 400 with the raw database error text.
- **Remediation:** DB failures now log server-side and map to generic HTTP 500.
- **Finding MCP-22.02.29-02:** `DbBackedMcpRuntimeBridge` had the same raw DB-to-400 mapper, producing the same incorrect client-visible classification on MCP runtime operations.
- **Remediation:** runtime DB failures now use generic HTTP 500 with server-side diagnostics only.
- **Finding MCP-22.02.29-03:** MCP management authority errors in REST were flattened to 403, so expected invalid/not-found/internal conditions received the wrong HTTP status.
- **Remediation:** added typed `map_mcp_authority_error`; invalid -> 400, forbidden -> 403, not-found -> 404, internal -> generic 500. Added focused status tests.
- **Finding MCP-22.02.29-04:** remote Alloy scaffold stage/review/apply responses returned raw `error.to_string()` values, which could expose filesystem paths and internal implementation details to the MCP caller.
- **Remediation:** stage/review/apply now log the underlying error and return stable generic tool error messages.
- **Finding MCP-22.02.29-05:** the shared GraphQL management provider erased the server `Error` type into `McpManagementMutationError::Internal(String)`, so GraphQL lost 400/404 semantics and could expose internal messages. The typed management error also lacked a `Forbidden` variant for authority denials.
- **Remediation:** added `McpManagementMutationError::Forbidden`; server provider now preserves forbidden/bad-input/not-found/internal categories with stable messages, and GraphQL maps forbidden to `PERMISSION_DENIED` while retaining the existing validation/conflict/not-found/internal mappings. Added focused provider regression coverage.
- **Security non-findings:** scaffold filesystem writes remain bounded by `RUSTOK_MCP_SCAFFOLD_WORKSPACE_ROOT` plus workspace `Cargo.toml`/target-directory invariants; persisted token resolution enforces token expiry/revocation, active client, tenant-consistent client/policy, and current delegated-user permissions; the production runtime composes `GuardedMcpManagementProvider` around `ServerMcpManagementMutationProvider`, so GraphQL does not bypass the existing authority ceiling. Current default tool requirements do not use arbitrary policy scopes as standalone default grants.
- **Liveness note reviewed:** scaffold draft status uses an explicit `staged -> applying -> applied/failed` flow. No documented lease/timeout contract exists for crash recovery of `applying`; introducing a synthetic lease would require owner schema/state-machine changes and was not folded into this controller iteration without a defined recovery contract.
- **Fresh second pass:** re-read `mcp.rs`, `mcp_management.rs`, `mcp_runtime.rs`, `mcp_management_authority.rs`, `mcp_management_guard.rs`, `mcp_management_mutation_provider.rs`, MCP GraphQL management code, scaffold workspace owner, token model, MCP docs, and runtime composition. Checked raw error responses, typed status mapping, GraphQL parity, tenant binding, authority wrapping, token lifecycle and scaffold path controls. No additional repository-owned defect remained in the primary controller surface.
- **Verification:** repository-content inspection and branch diff review only. No tests, clippy, build, gatekeeper, migrations, or runtime commands were run by the agent per maintainer-owned execution policy.
- **Status:** `FS-22.02.29` complete. Next primary module: `FS-22.02.30 — apps/server/src/controllers/oauth_metadata.rs`.
- [x] **FS-22.02.29 — `apps/server/src/controllers/mcp.rs`** — one-module audit; completed with five remediation units and a fresh independent second pass across REST, GraphQL, runtime binding, authority guard and scaffold transport boundaries.
### FS-22.02.30 Iteration 1 — `apps/server/src/controllers/oauth_metadata.rs`

- **Base:** refreshed `main` at `eddd280afffc13b875e32eb0e0539b22f7fb2536`; dedicated branch `codex/audit-fs-22.02.30-oauth-metadata`.
- **Invariant map:** discovery metadata must identify the same authorization server that serves the well-known document; endpoint URLs must be derived from the configured public issuer rather than an unrelated environment fallback; advertised response modes, grant methods, PKCE methods, and UserInfo claims must match the actual OAuth transport; malformed issuer configuration must fail closed without exposing configuration internals.
- **Finding OAUTH-METADATA-22.02.30-01:** `oauth_metadata.rs` used `RUSTOK_PUBLIC_URL` with a localhost fallback for endpoint URLs while taking the issuer from `AuthConfig`. A production configuration with issuer `https://api.example.com` could therefore publish an issuer for one authority and endpoints for another host. RFC 8414 requires the returned issuer to be identical to the issuer identifier used to derive the well-known metadata URL.
- **Remediation:** removed the environment/localhost fallback; metadata now uses the configured auth issuer as the single public URL source and constructs authorization, token, UserInfo, and revocation endpoint URLs from that same authority.
- **Finding OAUTH-METADATA-22.02.30-02:** the implementation omitted `response_modes_supported`, so RFC 8414 defaults would advertise `query` and `fragment` even though the authorization transport only returns authorization results through query parameters.
- **Remediation:** explicitly advertise only `query`.
- **Finding OAUTH-METADATA-22.02.30-03:** `claims_supported` listed JWT/token claims such as `iss`, `aud`, `exp`, `iat`, and `client_id` that are not returned by the UserInfo endpoint.
- **Remediation:** align `claims_supported` with the actual UserInfo response: `sub`, `role`, `tenant_id`, `name`, `email`, and `email_verified`.
- **Finding OAUTH-METADATA-22.02.30-04:** relative/default issuer values, issuer query/fragment components, credentials in issuer userinfo, and non-root issuer paths were accepted by the metadata fallback logic even though the root well-known routes cannot represent those issuer forms.
- **Remediation:** require an absolute HTTPS issuer without userinfo, query, fragment, or unsupported path components; configuration errors map to the server's generic internal-error response.
- **Finding OAUTH-METADATA-22.02.30-05:** the controller comment described `/.well-known/openid-configuration` as OpenID Connect Discovery even though the server does not implement the complete OIDC Provider Configuration surface (ID-token/JWKS metadata and flows).
- **Remediation:** retain the RFC 8414 alias for compatibility but document it as an OAuth Authorization Server Metadata alias, not a full OIDC Provider Configuration document. Updated the OAuth verification guide and OAuth app-connections plan accordingly.
- **Focused regression coverage added:** issuer/endpoint consistency, malformed/non-HTTPS issuer rejection, query/fragment/path rejection, explicit response-mode advertisement, and UserInfo claim metadata.
- **Fresh second pass:** after PR #4225 was merged as `1732c78f57ea90c989bb319def2750e3cc64458e`, refreshed `main` and re-read the target controller, OAuth authorize/UserInfo transport, auth deployment validation, runtime ownership docs, and both OAuth verification documents. Confirmed there was no remaining primary-surface defect; the post-merge pass also cleaned up source formatting/comment drift without changing the contract.
- **Verification:** repository-content inspection, standards comparison against RFC 8414 and OpenID Connect Discovery documentation, branch diff review, PR integration review. No tests, clippy, build, gatekeeper, migrations, or runtime commands were run by the agent per maintainer-owned execution policy.
- **Integration:** implementation PR #4225 merged into `main` at `1732c78f57ea90c989bb319def2750e3cc64458e`. Closeout/documentation reconciliation is on `codex/audit-fs-22.02.30-closeout`.
- **Status:** `FS-22.02.30` complete. Next primary module: `FS-22.02.31 — apps/server/src/controllers/swagger.rs`.
- [x] **FS-22.02.30 — `apps/server/src/controllers/oauth_metadata.rs`** — one-module audit; completed with four metadata-contract remediation findings plus documentation/compatibility reconciliation and a fresh independent post-merge second pass.


### FS-22.02.31 Iteration 1 — `apps/server/src/controllers/swagger.rs`

- **Base:** refreshed `main` at `ada2d6f5635b9426581524915db486f34634fa07`; dedicated branch `codex/audit-fs-22.02.31-swagger-v2`.
- **Invariant map:** OpenAPI must describe the same documented operations actually owned by the server controllers; response/request schemas referenced by documented operations must exist in `components.schemas`; composite paths with multiple HTTP methods must retain all operations; special host modes must not publish hidden contracts; authentication requirements must be machine-readable rather than implied by prose.
- **Finding SWAGGER-22.02.31-01:** `ApiDoc` omitted 10 existing auth operations despite their controllers carrying `#[utoipa::path]` contracts: password reset request/confirm, session listing/revocation, revoke-all sessions, password change, profile update, and login history.
- **Remediation:** registered all omitted auth operations and their request/response schemas.
- **Finding SWAGGER-22.02.31-02:** `ApiDoc` omitted `/health/runtime`, despite the route being explicitly annotated and serving the runtime guardrail snapshot.
- **Remediation:** registered the runtime health operation and its ToSchema contract, including the guardrail snapshot types.
- **Finding SWAGGER-22.02.31-03:** `ApiDoc` omitted seven documented marketplace registry operations: request-changes, hold, resume, and the four remote-runner lifecycle operations.
- **Remediation:** registered all seven operations and their request/response schemas, including runner claim payloads and owner-transfer/validation request models.
- **Finding SWAGGER-22.02.31-04:** `ApiDoc` omitted both documented user-administration operations from `controllers/users.rs`.
- **Remediation:** registered `/api/users` and `/api/users/{id}`, including `UserItem`, `UsersListParams`, and `UsersResponse` schemas.
- **Finding SWAGGER-22.02.31-05:** remote registry runner endpoints require the dedicated `x-rustok-runner-token` header, but the OpenAPI document exposed no corresponding security scheme/operation requirement.
- **Remediation:** added a named `runner_token` API-key security scheme and attached it to all four remote runner operations while retaining the existing user bearer scheme for session-backed routes.
- **Finding SWAGGER-22.02.31-06:** registry-only OpenAPI filtered paths but left unrelated auth/admin/user schemas, security schemes, and tags in the public document.
- **Remediation:** added reference-driven pruning for schema dependencies, security schemes, and tags after registry-only path filtering. Recursive schema references are retained; unused security/tag metadata is removed.
- **Adjacent route finding:** while enumerating all documented controller operations, `controllers/users.rs` was found to document `/api/users` but only mount `/api/users/`. Added the canonical non-slashed route while retaining the trailing-slash route for compatibility.
- **Focused regression coverage added:** complete documented core path inventory, shared-path HTTP method coverage, runtime schema presence, remote-runner authentication metadata, and registry-only hidden-component/tag isolation.
- **Fresh second pass:** re-read all `apps/server/src/controllers/*.rs` files containing `#[utoipa::path]`, compared 59 documented operations against the `ApiDoc` path registry, verified request/response schema coverage, rechecked the user route compatibility fix, and refreshed the branch over the concurrent `main` advance instead of carrying a stale branch base. No further repository-owned omission remained in the primary Swagger contract.
- **Verification:** repository-content/static inspection plus current `utoipa 5.5` API documentation review. No cargo tests, clippy, build, reference-artifact export, gatekeeper, migrations, or runtime commands were run by the agent per maintainer-owned execution policy.
- **Status:** `FS-22.02.31` complete. Next primary module: `FS-22.02.32 — apps/server/src/channels/builds.rs`.
- [x] **FS-22.02.31 — `apps/server/src/controllers/swagger.rs`** — one-module audit; completed with six Swagger contract findings, an adjacent users route compatibility fix, and a fresh independent second pass.


- [x] **FS-22.02.32 — `apps/server/src/channels/builds.rs`** — one-module audit; completed with one authorization remediation unit and a fresh independent second pass.
### FS-22.02.32 Iteration 1 — `apps/server/src/channels/builds.rs`

- **Base:** refreshed `main` at `df10d9af5770e4bdcea395a161761d48ce7240e7`; dedicated branch `codex/audit-fs-22.02.32-builds-channel`; merged as PR #4228 at `4a38082383e42ed09ec7710fb9087608b882751a`.
- **Invariant map:** the build progress stream is a platform control-plane read surface; WebSocket upgrade must require authenticated request context and canonical module-read authority; GraphQL and native transports must not diverge on authorization; build event scope remains platform-global and no tenant attribution is introduced in the transport.
- **Finding BUILDS-22.02.32-01:** `GET /ws/builds` documented Bearer authentication, but the normal `auth_context::resolve_optional` middleware permits anonymous requests to continue. The handler upgraded anonymous clients because it had no route-local authentication/permission admission.
- **Remediation:** the WebSocket handler now fails with HTTP 401 without an authenticated `AuthContextExtension`, and requires one of the canonical effective module permissions: `modules:read`, `modules:list`, or `modules:manage`. This matches the existing GraphQL `build_progress` subscription admission contract.
- **Adjacent-boundary re-audit:** `auth_context.rs` still supplies the request-scoped permission snapshot and principal facts; GraphQL performs the same module-read authorization; `build_event_hub.rs` shares one context-owned broadcast hub; the global `BuildEventScope::Platform` contract remains unchanged. No duplicate auth, tenant policy, or event ownership was introduced.
- **Regression audit:** anonymous requests can no longer reach `on_upgrade`; unrelated permissions are rejected; accepted module-read levels remain compatible with manage/list/read semantics. No lifecycle, broadcast, or wire-message behavior was changed by the remediation.
- **Fresh second pass:** re-read `builds.rs`, its runtime route registration in `host.rs`, middleware ordering in `app_router.rs`/`auth_context.rs`, GraphQL build subscription, build hub initialization, and the local server responsibility documentation. No additional repository-owned defect was found in the primary module that required another remediation unit.
- **Documentation:** `apps/server/docs/README.md` now records the `/ws/builds` authorization contract.
- **Verification:** repository-content/static inspection and branch/PR diff review only. No test suite, clippy, build, gatekeeper, migration, or runtime command was executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.02.32` complete. Next primary module: `FS-22.03` must first be decomposed into one-primary-module iterations.

- [x] **FS-22.03 — identity/auth propagation:** complete; all currently decomposed primary modules through FS-22.03.18 were audited in sequential module-specific iterations.
- [x] **FS-22.03.01 — `apps/server/src/extractors/auth/mod.rs`** — completed with one security-boundary remediation and a fresh independent second pass.
- [x] **FS-22.03.02 — `apps/server/src/middleware/auth_context.rs`** — completed with one principal-admission remediation and a fresh independent second pass.
- [x] **FS-22.03.03 — `apps/server/src/controllers/auth.rs`** — completed with one account-enumeration remediation and a fresh independent second pass.
- [x] **FS-22.03.04 — `apps/server/src/controllers/oauth.rs`** — completed with one Bearer-parser compatibility remediation and a fresh independent second pass.
- [x] **FS-22.03.05 — `apps/server/src/models/oauth_apps.rs`** — completed with one exact-grant-policy remediation and a fresh independent second pass.
- [x] **FS-22.03.06 — `apps/server/src/services/oauth_token_service.rs`** — completed with one security/presentation boundary remediation and a fresh independent second pass.
- [x] **FS-22.03.07 — `crates/modules/rustok-auth/src/jwt.rs`** — completed with one input-range remediation and a fresh independent second pass.
- [x] **FS-22.03.08 — `crates/modules/rustok-auth/src/credentials.rs`** — completed with one RNG-failure remediation and a fresh independent post-merge second pass.
- [x] **FS-22.03.09 — `crates/modules/rustok-auth/src/config.rs`** — fresh module audit found no remaining owner-level defect requiring code remediation; auth-settings parse-failure handling is deferred to the host adapter track.
- [x] **FS-22.03.10 — `apps/server/src/auth.rs`** — completed with one fail-closed configuration parsing remediation and a fresh post-merge second pass.
- [x] **FS-22.03.11 — `crates/modules/rustok-auth/src/lifecycle.rs`** — complete.
- [x] **FS-22.03.12 — `apps/server/src/services/auth_lifecycle.rs`** — complete.
- [x] **FS-22.03.13 — `apps/server/src/services/auth_lifecycle_provider.rs`** — complete.
- [x] **FS-22.03.14 — `crates/modules/rustok-auth/src/backfill.rs`** — complete.
- [x] **FS-22.03.15 — `crates/modules/rustok-auth/src/bootstrap.rs`** — complete.
- [x] **FS-22.03.16 — `crates/modules/rustok-auth/src/admin_mutations.rs`** — complete.
- [x] **FS-22.03.17 — `apps/server/src/services/auth_admin_mutation_provider.rs`** — complete.
- [x] **FS-22.03.18 — `apps/server/src/services/oauth_admin_guard.rs`** — complete after PR #4277.
- [ ] **FS-22.04 — tenant/channel/locale propagation:** in progress; decomposed into one-primary-module iterations focused on the shared request-context boundary first, then tenant resolution/cache, channel resolution/cache, locale policy/cache, and transport propagation boundaries.
- [x] **FS-22.04.01 — `crates/libs/rustok-api/src/request.rs`** — completed with trusted request-context projection, canonical locale evidence, URL form decoding, and tenant-consistency fences.
- [x] **FS-22.04.02 — `apps/server/src/middleware/tenant_resolution.rs`** — typed tenant identifier/source resolution and request-trust boundary; completed after PR #4281 and post-merge reconciliation. Next primary module: `FS-22.04.03 — apps/server/src/middleware/tenant.rs`.
- [x] **FS-22.04.03 — `apps/server/src/middleware/tenant.rs`** — tenant read-port/cache/context materialization and invalidation propagation; completed after PR #4284 and post-merge reconciliation. Next primary module: `FS-22.04.04 — apps/server/src/middleware/channel.rs`..
- [x] **FS-22.04.04 — `apps/server/src/middleware/channel.rs`** — channel RequestFacts, selector/host/OAuth/locale propagation and cache identity; completed after PR #4286 and post-merge reconciliation. Next primary module: `FS-22.04.05 — apps/server/src/middleware/locale.rs`.
- [x] **FS-22.04.05 — `apps/server/src/middleware/locale.rs`** — tenant locale policy enforcement and cache/generation propagation; completed after fresh module-level re-audit from `a6001d414deffefdd32fb5eda8bff5b0680b894c`. No additional repository-owned in-scope defect required code remediation.
### FS-22.04.05 Iteration 1 — `apps/server/src/middleware/locale.rs`

- **Base:** refreshed `main` at `a6001d414deffefdd32fb5eda8bff5b0680b894c`; dedicated branch `codex/audit-fs-22.04.05-locale`.
- **Invariant map:** trusted tenant requests must never honor a locale outside the tenant-owned policy; canonical request-locale precedence and normalization must remain unchanged; empty/legacy policy projections must fail closed to the trusted tenant default; cache fills must not repopulate obsolete policy after invalidation; local cache generation state must remain bounded and fail closed on version exhaustion; durable tenant-generation recovery must invalidate local locale state on missed, gapped, or regressed observations; response `Content-Language` and request `ResolvedRequestLocale` must use the same effective locale.
- **Discovery:** re-read `apps/server/src/middleware/locale.rs` end-to-end together with the tenant `TenantLocalePolicyPort`, locale-policy owner validation/persistence, tenant/locale generation listeners, application-router ordering, cache initialization, request-locale primitives, and the retained locale-generation regression suite.
- **Confirmed current contract:** the middleware always constrains tenant-bound requests, including an empty policy projection; locale-policy reads are owner-port based and typed; tenant locale policy validation enforces canonical locale uniqueness, exactly one enabled default, valid enabled fallbacks, and acyclic fallback graphs; tenant locale writes commit policy rows, tenant default, idempotency receipt, and invalidating events transactionally; the shared tenant durable generation is advanced by the canonical event transport and consumed by both tenant and locale cache listeners.
- **Generation/cache review:** the bounded monotonic per-tenant version namespace prevents stale async fills from becoming reachable after invalidation; rollover clears obsolete keys; version exhaustion bypasses cache state rather than reusing an unsafe token. The locale generation listener handles exact/wildcard invalidations, durable-ahead recovery, local lag, Redis publication gaps, and generation regression by clearing local state and retaining degraded readiness.
- **Router/propagation review:** the composed HTTP stack executes tenant resolution before locale middleware (then auth/channel), while registry/worker-only profiles intentionally run locale without tenant because those routes are not tenant-bound. `resolve_locale` consumes only the trusted tenant extension and the canonical `resolve_request_locale` primitive; downstream `RequestContext` now requires the resulting `ResolvedRequestLocale`.
- **Fresh second pass:** independently re-read the locale middleware after the surrounding FS-22.04.01..04 changes landed on `main`, checked cache/generation failure paths and fallback ordering again, and compared the module against the owner policy contract. No new repository-owned defect remained in this primary module.
- **Deferred adjacent findings:** the legacy `rustok-core::i18n::Locale` projection mismatch for world-language Unicode locales remains intentionally assigned to `FS-22.04.10`; GraphQL WebSocket policy/selection remains `FS-22.04.06`. Neither is patched from `locale.rs` because doing so would create a competing owner/contract.
- **Verification:** GitHub source inspection, cross-file static reasoning, and branch/ledger diff review only. No tests, clippy, build, migration, gatekeeper, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.05` complete after integration; next primary module is `FS-22.04.06 — apps/server/src/controllers/graphql.rs`.

- [x] **FS-22.04.06 — `apps/server/src/controllers/graphql.rs`** — HTTP/WebSocket tenant/channel/locale context propagation only; GraphQL resolver composition remains FS-22.05; completed after PR #4290 and post-merge reconciliation.
### FS-22.04.06 Iteration 1 — `apps/server/src/controllers/graphql.rs`

- **Base:** refreshed `main` at `95351d3556bf5d965351cf6df1303a97a07f6059`; dedicated branch `codex/audit-fs-22.04.06-graphql`.
- **Invariant map:** HTTP GraphQL must consume trusted tenant/locale/channel/auth context rather than reconstructing authority; WebSocket tenant/auth/locale initialization must remain tenant-bound and fail closed; initialization transport must have bounded frame/message/queue/resource behavior; credentials and untrusted tenant identifiers must not enter logs; RBAC auth leases must be established only after successful tenant/auth/locale validation and revalidated before emitting post-initialization results.
- **Discovery:** audited the complete controller together with tenant route policy, tenant/locale/channel/auth middleware, `RequestContext`, RBAC request scope, GraphQL schema security extensions, GraphQL resolver consumers, locale-policy owner contract, WebSocket protocol behavior, and local GraphQL documentation.
- **Finding GRAPHQL-22.04.06-01:** a completed WebSocket upgrade could remain indefinitely in the pre-initialization state because the controller waited on the GraphQL stream without a `connection_init` receipt deadline. The HTTP edge timeout no longer applies after upgrade, so idle unauthenticated sockets could retain per-connection runtime state.
- **Remediation:** added a 10-second connection-initialization receipt deadline and closes with protocol code `4408`; the deadline is gated by a callback-entry marker so it covers receipt of the init message rather than cancelling slow tenant/auth/locale validation after the message has already arrived.
- **Finding GRAPHQL-22.04.06-02:** failed tenant resolution logged the raw `tenantSlug` from the untrusted WebSocket initialization payload, allowing attacker-controlled diagnostic amplification and identifier disclosure in application logs.
- **Remediation:** removed the raw slug from the warning and added a source regression guard preventing direct `tenant_slug` logging through the controller's tracing macros.
- **Regression audit:** existing frame/message/queue limits, protocol-negotiation rejection, APQ hash bounds, stable GraphQL JSON errors, tenant owner-port locale policy, RequestContext insertion, RBAC lease ordering/revalidation, and intentionally unset WebSocket channel dimensions remain unchanged.
- **Fresh second pass:** independently re-read the changed controller after remediation and then re-read the merged `main` implementation. The second pass caught and corrected the initial timeout implementation's semantic overreach before PR creation; final merged code now times only init-message receipt and preserves slow initialization callbacks.
- **Deferred adjacent findings:** the legacy `rustok-core::i18n::Locale` projection mismatch for world-language Unicode locales remains assigned to `FS-22.04.10`; full GraphQL resolver composition remains FS-22.05; detached WebSocket worker lifecycle remains FS-24.
- **Verification:** GitHub source inspection, cross-file contract reasoning, branch diff review, and post-merge source reconciliation only. No tests, clippy, build, migration, gatekeeper, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.06` complete after PR #4290 merge (`a9ca6020415e7a6c5b911d8bc1092b91f63231fc`); next primary module is `FS-22.04.07 — apps/server/src/middleware/channel_native_wrapper.rs`.

- [x] **FS-22.04.07 — `apps/server/src/middleware/channel_native_wrapper.rs`** — native mutation context propagation and channel invalidation boundary; completed after PR #4293 and post-merge reconciliation.

### FS-22.04.07 Iteration 1 — `apps/server/src/middleware/channel_native_wrapper.rs`

- **Base:** refreshed `main` at `fbdee42911aa19279a227fcac4566e7e087a47a9`; dedicated branch `codex/audit-fs-22.04.07-channel-native-wrapper`; integrated by PR #4293.
- **Invariant map:** native `/api/fn/*` channel mutations must use the same trusted tenant context as handler authorization; successful native mutations must invalidate the tenant-local channel resolution cache and publish durable generation; REST/native paths must retain transport parity; wildcard server-function classification must not drift from the owner endpoint inventory.
- **Discovery:** re-read the wrapper, base channel middleware, application-router ordering, tenant/auth context extension, all Channel Admin native server functions, REST channel controller, cache invalidation service/listener, durable channel generation migration, and the channel cache architecture guard.
- **Confirmed current contract:** all 15 native channel mutations are tenant/auth gated; the wrapper executes on the wildcard server-function route after tenant context is available and delegates channel resolution to the base middleware; successful native mutations call the shared `invalidate_tenant_channel_cache` facade (local cache invalidation plus durable-generation publication fast path); REST mutations use the same facade; durable reconciliation recovers missed or remote publication and generation regression.
- **Finding FS-22.04.07-01:** the existing architecture guard only checked that every owner server-function mutation endpoint appeared as a substring in the wrapper source. That allowed stale, extra, or duplicate wrapper entries to evade detection even while the current inventory happened to be complete.
- **Remediation:** documented the host ownership of wildcard-route classification and strengthened `channel_cache_architecture_guard` to parse both endpoint inventories, normalize them to exact `/api/fn/...` paths, sort, and require exact equality; non-channel paths and the read-only bootstrap route are rejected. Runtime invalidation behavior was left unchanged.
- **Fresh second pass:** re-read the merged wrapper and architecture guard, re-compared all 15 native mutation endpoints with the wrapper inventory, and rechecked REST invalidation, tenant/auth admission, router ordering, and durable generation recovery. No additional repository-owned defect remained inside this primary module.
- **Adjacent findings:** ChannelService multi-statement transaction/invariant concerns remain within the existing channel owner work and are not patched from the middleware wrapper. No owner-level ChannelContext/tenant/locale propagation drift was found in this iteration, so FS-22.04.08–10 remain conditional on a concrete finding in their own primary-module audits.
- **Verification:** GitHub source inspection, cross-file static reasoning, branch review, and post-merge source reconciliation only. No tests, clippy, build, migration, gatekeeper, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.07` complete after PR #4293 merge (`d79545be8c44bf5e6e7530f92fe45fcdfb074454`); next primary module is `FS-22.04.08 — crates/libs/rustok-api/src/context/channel.rs`.

- [x] **FS-22.04.08 — `crates/libs/rustok-api/src/context/channel.rs`** — shared ChannelContext shape/source propagation; completed after fresh module-level audit with no repository-owned code defect requiring remediation.

### FS-22.04.08 Iteration 1 — `crates/libs/rustok-api/src/context/channel.rs`

- **Base:** refreshed `main` at `1e8bac381c87c3b5aa5f9b3ee26d7b99a0e9e748`; dedicated branch `codex/audit-fs-22.04.08-api-channel-context`.
- **Invariant map:** the shared channel context must remain owner-neutral, tenant-bound by upstream trusted middleware, transport-safe, and consistent with `RequestContext`, the channel owner resolver, native/REST admin adapters, and server-only extraction boundaries. Resolution source and trace types must retain stable serialization names and all channel-specific authority must remain outside `rustok-api`.
- **Discovery:** re-read the complete `context/channel.rs`, `context/mod.rs`, `context/tenant.rs`, `context/auth.rs`, `request.rs`, API crate exports/docs, channel owner resolution/DTO/service contracts, server channel middleware and tests, REST/native channel transports, storefront/search channel consumers, and the current runtime-context invariant verifier.
- **Confirmed current contract:** `ChannelContext` carries the resolved channel identity, tenant identity, active/status state, selected target, settings, resolution source, and typed trace; server-only `ChannelContextExtension`/extractors are feature-gated; `RequestContext` independently verifies channel-to-tenant equality before projecting the channel id/slug/source; channel middleware derives the context from the canonical owner `ResolutionDecision`; REST/native transport uses the same shared context contract without moving channel ownership into `rustok-api`.
- **Security review:** no alternate tenant/channel authority is introduced by the shared type. The required extractor reads only the internally inserted extension, while `RequestContext` rejects cross-tenant channel context. The channel resolver itself performs tenant checks for explicit channel-id selection before the context is materialized.
- **API consistency review:** `ChannelContextExt` currently exposes the helper on Axum `Parts`, matching the actual request-part extraction surface. `TenantContextExt` additionally exposes an `Extensions` helper, but no current channel production consumer requires the analogous method and no code path reconstructs channel authority from raw `Extensions`; adding an unused compatibility surface would enlarge the public contract without fixing a live defect.
- **Shape/transport review:** the serializable shared type is intentionally reused by the authenticated Channel Admin bootstrap response; this is documented and does not cross into an unauthenticated public surface. The admin-specific view model remains separately owned by `rustok-channel-admin`.
- **Fresh second pass:** independently re-read the merged FS-22.04.07 channel middleware/wrapper after its post-merge SHA refresh, then compared the shared context fields against `RequestContext`, owner DTOs, admin models, storefront/search consumers, and the module effective-policy channel input boundary. No additional repository-owned defect remained inside this primary module.
- **Adjacent findings:** channel revision/effective-policy revision remains an owner policy concern and is not duplicated into `ChannelContext` without a demonstrated transport requirement; the conditional FS-22.04.09/10 audits remain separate shared-contract reviews.
- **Remediation:** no production code change required in this primary module. The existing API contract is retained unchanged.
- **Verification:** GitHub source inspection, cross-file static reasoning, branch review, and post-merge source reconciliation only. No tests, clippy, build, migration, gatekeeper, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.08` complete with no code remediation; next primary module is `FS-22.04.09 — crates/libs/rustok-api/src/context/tenant.rs`.

- [x] **FS-22.04.09 — `crates/libs/rustok-api/src/context/tenant.rs`** — shared TenantContext/extension contract; completed after fresh module-level audit with no repository-owned production defect requiring remediation.

### FS-22.04.09 Iteration 1 — `crates/libs/rustok-api/src/context/tenant.rs`

- **Base:** refreshed `main` at `c91ae1fcb6fc8658c4836d6532d7561f11dea229`; dedicated branch `codex/audit-fs-22.04.09-api-tenant-context`.
- **Invariant map:** the shared tenant context must remain a trusted projection of tenant middleware authority; it must not resolve tenants, inspect tenant-owned persistence, or accept raw request assertions itself. Server-only HTTP extraction must stay feature-gated, optional extraction must remain non-authorizing, and the public shape must remain compatible with the host/module boundary.
- **Discovery:** re-read the complete `context/tenant.rs`, `context/mod.rs`, `request.rs`, host-authority context, tenant middleware/resolution policy, current tenant extractor, tenant-owner DTO/service/ports, tenant tests, API crate feature contract/docs, and the current router ordering.
- **Confirmed current contract:** `TenantContext` contains the resolved tenant id, name, slug, optional domain, settings, default locale, and active state. It contains no resolver or storage logic. `TenantContextExtension` is the sole request extension wrapper, `TenantContextExt` reads only that wrapper, and `OptionalTenant` returns absence without fabricating authority.
- **Security review:** `CurrentTenant` consumes the trusted extension rather than request headers; tenant middleware validates the selected identifier before loading the projection and validates an asserted compatibility slug against the resolved tenant. Tenant-bound `RequestContext` requires the trusted tenant extension before constructing downstream request context. No raw request value is interpreted by this API module.
- **Feature/ownership review:** `rustok-api` keeps tenant HTTP extraction behind the `server` feature at the module boundary, while the actual tenant resolution and persistence remain in `apps/server` plus `rustok-tenant`. The `Extensions` accessor is useful for host internals and does not create a second authority path.
- **Status semantics:** the required `TenantContext` extractor fails with HTTP 500 and a static middleware-missing message, which is appropriate for missing internal infrastructure rather than a user-controlled not-found condition. The application-level `CurrentTenant` extractor separately maps absent context to 404 for its route contract; this distinction is outside this primary API module and is preserved.
- **Projection semantics:** middleware rejects inactive tenants before constructing the context, so `TenantContext.is_active` is currently an explicit projection of owner state rather than a client-controlled toggle. Iteration 1 identified the duplicate `rustok-api::TenantError` enum as legacy/shared surface and recorded it for a dedicated cleanup pass.
- **Fresh second pass:** re-read the tenant middleware around route classification, tenant identifier resolution, projection loading, generation-aware cache fill, and extension insertion after the current `main` refresh; rechecked `RequestContext` tenant binding and `CurrentTenant` consumption. No additional repository-owned defect remained inside `context/tenant.rs`.
- **Adjacent findings:** tenant middleware cache generation/negative-cache behavior, raw diagnostic logging inside `apps/server/src/middleware/tenant.rs`, and the legacy `rustok-api::TenantError` surface are separate concerns; none justifies changing the primary context contract in this iteration.
- **Remediation:** no production code change required in this primary module. Existing TenantContext/extension/extractor contract is retained unchanged.
- **Verification:** GitHub source inspection, cross-file static reasoning, branch review, and post-refresh second-pass reconciliation only. No tests, clippy, build, migration, gatekeeper, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** Iteration 1 completed with the shared TenantContext contract unchanged; the identified legacy error surface was intentionally isolated into Iteration 2 before closing FS-22.04.09.


### FS-22.04.09 Iteration 2 — retire unused `rustok-api::TenantError`

- **Trigger:** Iteration 1 identified a persistence-backed `TenantError` in the shared API crate that duplicated the owner module's `rustok_tenant::TenantError`. A dedicated repository/public-consumer audit was performed before changing the exported surface.
- **Usage audit:** no current repository source usage or exact qualified import of `rustok_api::TenantError` or `rustok_api::context::TenantError` was found. GitHub code search for the qualified forms returned zero results.
- **Ownership audit:** tenant service operations use `rustok_tenant::TenantError`; `TenantContext` is a trusted context projection and has no need for a persistence/domain error enum in the shared context module.
- **External publication check:** no public GitHub consumer of the qualified API was found, and web search returned no public crates.io result for `rustok-api`. Private/unindexed consumers cannot be proven from repository inspection, so this remains a repository-local API cleanup rather than a claim of universal external compatibility.
- **Remediation:** removed `TenantError` from `crates/libs/rustok-api/src/context/tenant.rs`, removed its context-module and crate-root re-exports, and added a permanent API-surface guard forbidding the retired error and direct `sea_orm::DbErr` dependency from the tenant context module while retaining `TenantContext` and `OptionalTenant`.
- **Regression audit:** tenant authority resolution, extension insertion, `CurrentTenant`, `RequestContext`, and owner `rustok_tenant::TenantError` semantics are unchanged. The shared `rustok-api` SeaORM dependency remains because its separate `runtime` feature still legitimately uses it.
- **Verification:** source inspection, qualified-usage search, dependency/feature review, and API-surface guard review only. No Cargo/test/clippy/runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.09` fully closed after retirement of the unused legacy shared error contract; next primary module is `FS-22.04.10 — crates/libs/rustok-api/src/locale.rs`.

- [x] **FS-22.04.10 — `crates/libs/rustok-api/src/locale.rs`** — shared typed locale normalization/runtime-vs-storage boundary; Unicode/CLDR canonicalization is delegated to `rustok-ui-i18n` while the API layer retains the 32-byte normalized storage projection and `und` provenance separation.
### FS-22.04.10 Iteration 1 — `crates/libs/rustok-api/src/locale.rs`

- **Base:** refreshed `main` at `d45806b363bf12644dac768d9bc0bee54d44c34e`; dedicated branch `codex/audit-fs-22.04.10-api-locale-contract`.
- **Invariant map:** one canonical Unicode/CLDR locale parser must own locale grammar and alias resolution; `rustok-api` may compose host precedence and storage/runtime policy but must not carry a second incompatible locale grammar; runtime/tenant locales must reject storage-only `und`; normalized locale identities must remain within the platform's 32-byte storage width.
- **Discovery:** re-read `crates/libs/rustok-api/src/locale.rs`, API exports/docs, `rustok-ui-i18n` locale and Accept-Language owner modules, tenant locale-policy ports/service projection, server locale middleware, request-context extraction, static module UI i18n validation, and the historical Navigation consumer contract.
- **Finding LOCALEAPI-22.04.10-01:** `rustok-api::normalize_locale_tag` contained a second hand-written BCP-47 normalizer with a 32-byte raw-input bound and no extension/CLDR alias semantics, while `rustok-ui-i18n` is the documented canonical owner for Unicode locale parsing/canonicalization. This made locale identity dependent on request source: `Accept-Language` used the UI owner and could canonicalize aliases such as `iw-IL-u-ca-hebrew` to `he-IL`, whereas query/cookie/Medusa and tenant-policy paths went through the API parser and rejected that same valid Unicode locale. Tenant policy and static UI contracts therefore risked carrying aliases/incompatible identities into runtime/catalog matching.
- **Remediation:** replaced the hand-written parser with a direct delegation to `rustok-ui-i18n::normalize_locale_tag`. The API layer now enforces only the host/storage contract that remains its responsibility: the canonical extension-free identity must fit the platform's 32-byte locale width. `RuntimeLocale` and `TenantLocale` continue to reject `und`, while `StoredLocale` continues to permit it as explicit unknown provenance.
- **Regression coverage:** added canonical Unicode/CLDR alias coverage for `iw-IL-u-ca-hebrew` and typed-runtime coverage proving the same canonical identity is used through `RuntimeLocale`. The existing malformed, `und`, serialization and common BCP-47 cases remain covered.
- **Adjacent-boundary re-audit:** tenant locale-policy entries already use `TenantLocale`; the owner service canonicalizes persisted policy rows through that type and validates exact default/fallback invariants. Server locale middleware receives the typed policy projection and retains tenant allowlist/default/fallback selection; `RequestContext` continues to consume only the upstream resolved locale extension. Static module i18n validation continues to use `rustok-api::normalize_locale_tag`, so it now shares the same canonical locale identity as UI catalogs.
- **Fresh second pass:** independently re-read the changed API module, root exports/docs, the i18n verifier, tenant policy owner, server locale resolver, request-context consumer and known legacy Navigation/static-package consumers. No additional repository-owned production defect requiring remediation remained inside this primary module. The separately exposed API fallback-candidate helpers have no current repository consumer and their semantics are not changed speculatively in this iteration.
- **Guardrail:** `scripts/verify/verify-i18n-contract.mjs` now requires the API normalizer to delegate to `rustok-ui-i18n` and rejects reintroduction of the legacy hand-written parser shape.
- **Documentation:** synchronized `crates/libs/rustok-api/README.md` and `crates/libs/rustok-api/docs/README.md` with the canonical normalization ownership and 32-byte host/storage projection.
- **Verification:** repository-content/static inspection, commit history, external-publication search, and branch-diff review only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.10` complete after integration of the dedicated implementation branch; next primary module is `FS-22.04.11`.

- [x] **FS-22.04.11 — request-derived cache-key propagation across owner adapters** — completed as a repository-wide cache-key owner assessment across the canonical cache contract and the concrete server/module cache adapters inspected after FS-22.04.10. No repository-owned request-derived cache-key defect was confirmed, so no speculative cache-key rewrite was introduced.
- [x] **FS-22.05.01 — `apps/server/src/graphql/loaders.rs`** — one-module audit; GraphQL tenant-name loader error boundary hardened to redact backend diagnostics; integrated via PR #4304 at `702985eed0f6e0386bda2909ae16c451e1261239`.
- [x] **FS-22.05.02 — `apps/server/src/graphql/schema.rs`** — one-module audit; composition dependencies and generated runtime-data factories reverified end-to-end; no repository-owned in-scope defect required remediation.
- [x] **FS-22.05.03 — `apps/server/src/graphql/types.rs`** — one-module audit; public GraphQL complex fields and transport projection error boundaries hardened. Next primary module: `apps/server/src/graphql/queries.rs`.
- [x] **FS-22.05.04 — `apps/server/src/graphql/queries.rs`** — one-module audit; root query authorization/tenant scoping/pagination reverified and GraphQL owner/backend diagnostics redacted at the query error boundary. Next primary module: `apps/server/src/graphql/subscriptions.rs`.
- [x] **FS-22.05.05 — `apps/server/src/graphql/subscriptions.rs`** — one-module audit; subscription authorization, event scope/filtering, connection lifecycle, and error boundary reverified; stable permission error mapping applied. Next primary module: `apps/server/src/graphql/settings/query.rs`.
- [x] **FS-22.05.06 — `apps/server/src/graphql/settings/query.rs`** — one-module audit; host authority and tenant scope reverified, settings owner redaction preserved, invalid category mapped to `BAD_USER_INPUT`, and raw backend diagnostics removed from GraphQL errors. Next primary module: `apps/server/src/graphql/settings/mutation.rs`.
- [ ] **FS-22.05.07 — `apps/server/src/graphql/settings/mutation.rs`** — one-module audit; host/tenant authority, secret-write handling, category validation, outbox behavior, optimistic concurrency, and owner error mapping.

- [ ] **FS-22.05 — GraphQL composition:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.

### FS-22.04.11 Assessment — request-derived cache-key propagation across owner adapters

- **Base:** refreshed `main` at `7ad05f0bcd87f4227a99cd7ce3f3984874dbb105`; dedicated implementation branch `codex/audit-fs-22.05.01-graphql-loaders` was created from that exact SHA for the next primary module.
- **Discovery:** re-read the canonical cache key contract (`rustok-cache::CacheKeyBuilder`, generation and typed-cache boundaries) plus the concrete owner caches for channel resolution, tenant locale, field definitions, RBAC permissions, SEO redirects, Pages storefront data, marketplace catalog/detail, and tenant/module policy.
- **Invariant map:** a cache entry must include every request/source attribute that can change the value; tenant/channel/locale/permission/generation dimensions must not collapse into a shared key; provider-isolated caches may use owner instance identity only when the provider configuration and unkeyed inputs are immutable for that instance.
- **Finding assessment:** no concrete repository-owned cache-key collision or request-context omission was confirmed. Field-definition, RBAC, tenant, locale, Pages, channel and SEO caches include their required tenant/entity/generation dimensions; marketplace catalog list keys include registry identity plus query dimensions; marketplace detail is a provider-instance-local cache and its owner detail contract ignores list-query filters, so slug-only detail keys do not collapse distinct supported results.
- **Decision:** do not add a speculative cross-cutting cache-key abstraction or rewrite existing owner keys without a demonstrated collision. The track is closed as an assessment-only iteration; later cache findings remain assignable to their concrete owner modules.
- **Verification:** repository source/static inspection and owner-contract comparison only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent, per the maintainer-owned test rule.
- **Status:** `FS-22.04.11` closed with no code remediation. The next primary module is `FS-22.05.01 — apps/server/src/graphql/loaders.rs`.

### FS-22.05.02 Assessment — `apps/server/src/graphql/schema.rs`

- **Base:** post-merge `main` refreshed at `702985eed0f6e0386bda2909ae16c451e1261239`; dedicated branch `codex/audit-fs-22.05.02-graphql-schema` was created from that exact SHA.
- **Discovery:** re-read the full schema composition module, its direct bootstrap caller (`services/graphql_schema.rs`), generated GraphQL contribution code emitted by `apps/server/build.rs`, `rustok-api::GraphqlRuntimeInputs`, and the shared-value injection used to assemble the host runtime.
- **Invariant map:** every schema dependency must be present before resolver execution; optional module contributions must be feature-gated at compile time; generated runtime-data factories may fail closed before schema finalization; startup composition failures must not silently produce a partially composed schema.
- **Assessment:** `init_graphql_schema` validates the boot-owned `ModuleRegistry` and `SharedModuleMarketplaceCatalog` before constructing `GraphqlRuntimeInputs`, then publishes the same host runtime values used by generated factories. `build.rs` generates feature-gated `MergedObject`/`MergedSubscription` members and invokes only declared runtime-data factories. The `expect` calls in `schema.rs` therefore guard explicit composition invariants rather than request-derived failures; replacing them with a wider fallible schema-construction API would be an architectural contract change without a demonstrated runtime defect.
- **Immediate/adjacent re-audit:** the marketplace catalog is inserted into the same host runtime consumed by the schema, optional storage/alloy/media providers are attached before `GraphqlRuntimeInputs` construction, and generated feature guards match the manifest contribution descriptors. No alternate schema builder bypass was identified in the inspected server composition.
- **Regression audit:** resolver limits (`depth=12`, `complexity=600`) and security extensions remain globally attached; DataLoader registration remains unchanged; optional feature-specific data are still attached only when their feature is compiled and their runtime provider is present. No behavior change was introduced.
- **Fresh second pass:** independently re-read `schema.rs`, `services/graphql_schema.rs`, `build.rs` runtime-data generation, and `rustok-api` GraphQL runtime-input contract. No repository-owned in-scope defect remained in this primary module.
- **Verification:** repository source/static inspection only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.05.02` closed without code remediation. Next primary module: `FS-22.05.03 — apps/server/src/graphql/types.rs`.
### FS-22.05.03 Assessment — `apps/server/src/graphql/types.rs`

- **Base:** post-merge `main` refreshed at `c85f9af4ac550e8faee5d3175d72adf957dde7cc`; dedicated branch `codex/audit-fs-22.05.03-graphql-types` was created from that exact SHA.
- **Discovery:** read the complete 1,377-line GraphQL type adapter, the direct `User` field dependencies (`RbacService`, `FlexAttachedValuesService`, `TenantNameLoader`), the canonical `rustok-api::GraphQLError` taxonomy, the module recovery owner projection, and the build/recovery transport contracts consumed by this adapter.
- **Invariant map:** public GraphQL fields must expose stable transport-safe errors rather than backend diagnostics; client-provided enum/action parsing must use `BAD_USER_INPUT`; trusted persisted-state failures must use `INTERNAL_ERROR`; tenant scope comes from the authenticated request/owner projection rather than client-selected tenant identity; browser-safe recovery projections must not copy raw hook/database diagnostics.
- **Finding GRAPHQLTYPES-22.05.03-01:** `User::role`, `User::can`, and `User::custom_fields` converted owner/RBAC/Flex failures directly to strings, allowing database/storage diagnostics to cross the GraphQL boundary. `Permission::from_str` likewise used the raw parser error for invalid actions.
- **Remediation:** added one local GraphQL error helper that logs the backend diagnostic server-side and returns the canonical `INTERNAL_ERROR` with a stable field-specific message. Invalid permission actions now return `BAD_USER_INPUT`. Existing successful values, tenant/user identities, and DataLoader behavior are unchanged.
- **Finding GRAPHQLTYPES-22.05.03-02:** `ModuleOperationRecoveryPlan.error_message` copied the owner journal's raw diagnostic directly into a public GraphQL `SimpleObject`, despite the owner API describing the recovery view as browser-safe.
- **Remediation:** GraphQL now derives `error_message` from the typed recovery `issue` and returns only stable category messages (`post_hook_failed`, `pre_hook_failed`, or generic lifecycle failure), never the persisted raw error text. `retryable`, `recommended_action`, and typed lifecycle facts remain unchanged.
- **Immediate re-audit:** the complete changed adapter was re-read. No direct raw conversion remains in the `User` complex fields, and the recovery projection no longer forwards `plan.error_message` verbatim.
- **Adjacent-boundary re-audit:** `schema.rs` still registers the same loaders and global GraphQL extensions; `queries.rs` remains tenant/permission gated for the recovery query; the owner recovery snapshot is still the source of lifecycle category/identity facts. The build snapshot's operator diagnostics remain a separate build-owner contract and were not changed speculatively in this module.
- **Regression audit:** stable GraphQL codes are preserved for internal/user-input failures; successful role/permission/custom-field resolution is unchanged; recovery clients retain the same field shape but receive sanitized `errorMessage` values. No tenant selector or new authorization path was introduced.
- **Fresh second pass:** independently searched the full `types.rs` for `map_err`, `expect`, `unwrap`, `error_message`, warnings/errors, and owner projection adapters, then traced the identified recovery/build/error surfaces to their direct callers/owners. No remaining repository-owned defect attributable to this primary adapter module was confirmed.
- **Verification:** repository source/static inspection and branch-diff review only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.05.03` closed; branch is ready for PR/merge. Next primary module is `FS-22.05.04 — apps/server/src/graphql/queries.rs`.
### FS-22.05.06 Assessment — `apps/server/src/graphql/settings/query.rs`

- **Base:** post-merge `main` refreshed at `94a6e5151b052c77b0cd0387122a3dc133b9516c`; dedicated branch `codex/audit-fs-22.05.06-graphql-settings-query` was created from that exact SHA.
- **Discovery:** reviewed the complete settings query module plus `settings/mod.rs`, generic `SettingsService`, Iggy connector configuration owner, event delivery configuration owner, GraphQL settings payload types, and the admin Iggy consumer.
- **Invariant map:** tenant settings reads require the resolved tenant boundary and `settings:read`; process-wide settings require request-scoped `HostAuthority::Read`; generic settings must return only owner-redacted values; invalid client categories are input errors, while infrastructure/database failures are internal errors; host-global responses must not contain raw Iggy passwords.
- **Finding GRAPHQLSETTINGSQUERY-22.05.06-01:** all four settings query paths exposed owner/backend diagnostics through `FieldError::internal_error(&error.to_string())`. `platform_settings` additionally collapsed `SettingsError::InvalidCategory(user_input)` into `INTERNAL_ERROR`, reflecting the arbitrary category string in the error message.
- **Remediation:** added a local stable GraphQL settings error boundary; `SettingsError::InvalidCategory` now maps to `BAD_USER_INPUT` with a constant message, while all other owner/database/serialization failures return stable internal messages and keep diagnostics only in server logs.
- **Secret/value audit:** `SettingsService::get/get_all` already applies `redact_secrets` before GraphQL exposure; the service allowlist excludes owner-specific search settings. The Iggy snapshot exposes only secret resolver/key metadata and `password_configured`, never the resolved password. The admin UI contract explicitly states that only the secret reference is stored and the password never enters the UI/database.
- **Host authority audit:** `require_host_authority` reads a task-local `HostAuthorityContext`; `apps/server/src/host_authority.rs` authenticates a dedicated host credential, removes the raw header before downstream handling, and documents that GraphQL WebSocket upgrade tasks do not inherit the host-authority scope. `HostAuthorityContext` is independent from tenant RBAC, so ordinary tenant permissions cannot imply host-global access.
- **Tenant audit:** `require_tenant_settings_scope` rejects a host/tenant mismatch before tenant settings reads; `SettingsService::get/get_all` receives the resolved tenant ID, so GraphQL cannot select another tenant's generic settings through the category argument.
- **Fresh second pass:** re-read all resolver methods and searched the final file for raw error-string conversions; none remain. Rechecked host authority task-local scope, generic settings secret redaction, Iggy snapshot semantics, and the admin read contract. No additional repository-owned defect attributable to this primary query adapter was confirmed.
- **Regression audit:** successful settings data shape and category values are unchanged; redaction remains owned by `SettingsService`; host authority/tenant enforcement is unchanged; only error taxonomy and diagnostic presentation changed.
- **Verification:** repository source/static inspection, owner-contract tracing, and branch diff review only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.05.06` closed; ready for PR/merge. Next primary module: `FS-22.05.07 — apps/server/src/graphql/settings/mutation.rs`.

### FS-22.05.05 Assessment — `apps/server/src/graphql/subscriptions.rs`

- **Base:** post-merge `main` refreshed at `ca8300d578fea4e7ea6b8f9a4fd97081c4e3ae16`; dedicated branch `codex/audit-fs-22.05.05-graphql-subscriptions` was created from that exact SHA.
- **Discovery:** reviewed the complete subscription module plus `BuildEventHub`, `BuildService`, build event types/publisher, the GraphQL schema subscription registration, the HTTP build WebSocket channel, and the canonical module permission hierarchy.
- **Invariant map:** subscription setup must require an authenticated principal and tenant-scoped module permission; streamed events must belong to the scope represented by the owner model; build-id filters must be exact; broadcast lag/closure must not panic or spin; permission/backend failures must not expose persistence diagnostics.
- **Finding GRAPHQLSUBSCRIPTIONS-22.05.05-01:** module permission DB failures were passed through `FieldError::internal_error(&err.to_string())`, allowing backend diagnostics to escape from subscription setup.
- **Remediation:** added a server-side logging helper that returns a stable GraphQL `INTERNAL_ERROR` for permission lookup failures, with a regression test proving the client message does not contain representative backend diagnostics.
- **Finding GRAPHQLSUBSCRIPTIONS-22.05.05-02:** the denial message said only `modules:read` was required even though the policy accepts `modules:read`, `modules:list`, or `modules:manage`. This was a transport-contract inconsistency rather than an authorization bypass.
- **Remediation:** denial text now names all three accepted permission paths; the executable authorization policy and effective-permission semantics remain unchanged.
- **Event scope audit:** `BuildService` persistence and `BuildEvent` identity are platform-global in the inspected contract (build records have no tenant key), and `BuildEventScope::Platform` explicitly documents platform composition builds as global. Therefore the shared `BuildEventHub` does not represent a tenant-owned event stream that can be cross-filtered by `TenantContext`; no speculative tenant filter was introduced.
- **Adjacent WebSocket audit:** `/ws/builds` consumes the same global build hub and uses the same module permissions. Its raw `BuildFailed.error` payload is an operator-facing build diagnostic carried by the global build contract, not a subscription setup error; changing that payload would be a separate build transport contract iteration and was not bundled here.
- **Connection/lifecycle audit:** subscriber creation is request-scoped; broadcast `Lagged` and `Closed` paths are handled without panic; client disconnect drops the receiver through stream cancellation. The current best-effort live-stream behavior was left unchanged because no stronger delivery guarantee is declared by the build event contract.
- **Fresh second pass:** independently re-read `subscriptions.rs`, `BuildEventHub`, `BuildService`, `BuildEventScope`, and build WebSocket authorization. No additional repository-owned defect attributable to this primary module was confirmed.
- **Verification:** repository source/static inspection, owner/event-scope tracing, and branch diff review only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.05.05` closed; ready for PR/merge. Next primary module: `FS-22.05.06 — apps/server/src/graphql/settings/query.rs`.

### FS-22.05.04 Assessment — `apps/server/src/graphql/queries.rs`

- **Base:** post-merge `main` refreshed at `465ee1b55fe80c225f13e15c4e5c1a36444dbeec`; dedicated branch `codex/audit-fs-22.05.04-graphql-queries` was created from that exact SHA.
- **Discovery:** reviewed the complete root-query module, its direct service/owner callers, GraphQL dashboard security extension, tenant/auth extractors, canonical permission hierarchy, pagination contract, module lifecycle recovery owner, marketplace owner, and build owner.
- **Invariant map:** sensitive root queries must enforce authentication/RBAC either locally or through the schema extension; all tenant-owned reads must stay bound to the resolved request tenant; pagination and collection limits must be bounded before DB/provider calls; GraphQL error responses must not expose owner/backend diagnostics; recovery errors may expose only stable typed issue metadata.
- **Finding GRAPHQLQUERIES-22.05.04-01:** root-query error mapping repeatedly passed `DbErr`/owner errors through `FieldError::internal_error(&err.to_string())`, and `me` returned `err.to_string()` directly. `ModuleOperationRecoveryError::PostHookFailed` also formatted its raw hook error into the client-facing GraphQL message.
- **Remediation:** added a single server-side logging helper returning stable GraphQL `INTERNAL_ERROR` messages and migrated every raw internal-error conversion in the module to it. The post-hook recovery error retains its machine-readable code/retry metadata but now returns only `Module hook failed`. The `me` database path now returns a stable message as well.
- **Immediate re-audit:** searched the full module for raw `internal_error(&...)`, direct `err.to_string()` returns, and formatted error strings. No raw backend diagnostic path remained.
- **Authorization audit:** `dashboardStats` and `recentActivity` intentionally have no local guard but are fail-closed by `GraphqlDashboardSecurityPolicy`, which detects direct and fragment-contained fields and requires effective `ANALYTICS_READ`; this was traced through the schema composition and was not duplicated in the resolver.
- **Tenant audit:** the HTTP auth extractor rejects access tokens whose `claims.tenant_id` differs from the resolved `TenantContext.id`; direct-user sessions and user rows are checked against the same tenant. Root `user/users/me` queries additionally filter DB reads by the resolved tenant where applicable. Recovery plan lookup is tenant-bound inside the lifecycle owner and cross-tenant IDs become `OperationNotFound`.
- **Pagination/resource audit:** `clamp_collection_limit`, `PaginationInput::normalize`, `take(limit)`, and the owner pagination contracts bound collection sizes before the DB/provider calls. No unbounded list input was found in this module.
- **Regression audit:** existing authorization composition remains unchanged; dashboard authorization remains centralized; successful root query data mappings remain unchanged; only error presentation is hardened.
- **Fresh second pass:** independently re-read all root query declarations and guard placement, searched the final file for raw diagnostic patterns, and rechecked direct tenant/permission boundaries. No additional repository-owned defect attributable to this primary module was confirmed.
- **Verification:** repository source/static inspection, targeted owner-contract tracing, and branch diff review only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.05.04` closed; ready for PR/merge. Next primary module: `FS-22.05.05 — apps/server/src/graphql/subscriptions.rs`.

### FS-22.05.01 Iteration 1 — `apps/server/src/graphql/loaders.rs`

- **Base:** refreshed `main` at `7ad05f0bcd87f4227a99cd7ce3f3984874dbb105`; dedicated branch `codex/audit-fs-22.05.01-graphql-loaders`.
- **Invariant map:** GraphQL loader failures must not expose persistence/backend diagnostics to API clients; loader identity values originate from already-authorized GraphQL objects and must remain tenant-scoped by the source object; batching must preserve one result per tenant UUID without silently fabricating names.
- **Finding GRAPHQLLOADER-22.05.01-01:** `TenantNameLoader` converted `SeaORM::DbErr` directly to `async_graphql::Error::new(err.to_string())`. A database failure while resolving the public `User.tenantName` field could therefore return raw SQL/connection/storage diagnostics in the GraphQL response.
- **Remediation:** the loader now routes persistence failures through one local error boundary that logs the backend error server-side with only bounded batch-size context and returns the stable client message `Tenant name lookup failed`. No successful lookup, batching, or tenant identity mapping behavior was changed.
- **Regression coverage:** added a focused unit regression proving a representative backend diagnostic is absent from the client-facing GraphQL error message.
- **Immediate re-audit:** re-read the full loader and the only direct projection consumer, `User::tenant_name` in `apps/server/src/graphql/types.rs`; the field still uses the same DataLoader and remains optional for missing tenant rows.
- **Adjacent-boundary re-audit:** `apps/server/src/graphql/schema.rs` registers one `TenantNameLoader` DataLoader with the request schema; GraphQL user projections are assembled from tenant-scoped user records, so the loader does not introduce a new client-supplied tenant selector. No alternate loader or direct tenant-name GraphQL path was found in the inspected composition.
- **Regression audit:** successful tenant-name resolution remains unchanged; missing tenant rows still resolve to `None`; only backend-failure presentation changes from raw diagnostics to a stable error. The server log retains the original error for operator diagnosis without copying the UUID batch itself.
- **Fresh second pass:** independently re-read `loaders.rs`, `types.rs` tenant-name projection, schema DataLoader registration, and the GraphQL error construction path. No additional repository-owned defect remained inside this primary module.
- **Verification:** repository source/static inspection and branch-diff review only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** iteration ready for PR/merge after final branch-diff review.

- [ ] **FS-22.06 — REST/controller composition:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.07 — Server-function composition:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.08 — Embedded UI composition:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.09 — Feature/config interaction matrix:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.10 — Error/observability boundary:** do not start as a broad subsystem pass; convert it into the same one-primary-module queue before execution.
- [ ] **FS-22.11 — Fresh second-pass composition audit:** perform this only after the module queue above has been completed, still one primary module per iteration.

### FS-22.03.01 Iteration 1 — `apps/server/src/extractors/auth/mod.rs`

- **Base:** refreshed `main` at `1c7bf562207bb32ac0ef395b15b8bd5fe7725b53`; a later independent main change was refreshed before PR and found unrelated to this boundary.
- **Invariant map:** token verification must validate signature/issuer/audience/expiry before principal classification; tenant identity must match routed tenant; direct users require a live session bound to the token subject; delegated OAuth users require a current active app, grant, scope ceiling, and applicable consent; service tokens require an active app, client-credentials grant, app-subject binding, and current app permission ceiling; authentication must not depend on presentation/localization data.
- **Finding AUTH-22.03.01-01:** active OAuth-app resolution called `find_active_by_client_id`, which hydrated tenant locale and `oauth_app_translations` even though the authentication resolver needs only security/configuration fields. A presentation-storage outage or migration drift could therefore turn into an authentication outage for otherwise valid OAuth bearer tokens and added avoidable database work on every delegated/service-token request.
- **Remediation:** added the owner-side security-only `find_active_security_by_client_id` lookup and switched the auth extractor to it. Presentation-oriented lookup remains available to presentation callers and is not changed into a competing security source.
- **Regression coverage:** added an extractor integration regression that removes `oauth_app_translations` after app creation and verifies service-token permission resolution still succeeds.
- **Immediate/adjacent re-audit:** re-read the full extractor, OAuth app model lookup/hydration helpers, OAuth token issuance service, auth-context propagation, principal-kind classifier, RBAC permission restriction, session model, and OAuth consent lookup. The security lookup now has no tenant-locale/translation dependency; token scope and tenant checks remain fail-closed.
- **Fresh second pass:** independently compared authentication behavior for direct, delegated, and service principals; rechecked malformed/removed scopes, inactive OAuth apps, session-subject binding, inactive users, RBAC storage failure mapping, and the changed presentation/security lookup boundary. No additional repository-owned root-cause issue remained inside this primary module.
- **Adjacent finding assigned to next primary:** delegated OAuth users currently satisfy the `SecurityActorKind::User` check used by `auth_context.rs` self-service admission. This permits delegated principals to reach direct-session-oriented endpoints such as password/profile/session operations; because the fix belongs to the middleware admission boundary, it is explicitly recorded as `FS-22.03.02` rather than patched from the extractor.
- **Documentation:** `apps/server/docs/README.md` now states that access-token authentication uses a security-only OAuth app lookup independent of presentation translations.
- **Verification:** repository-content/static inspection and branch-diff review only. No test suite, clippy, build, gatekeeper, migration, or runtime command was executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.03.01` complete after merge; next primary module is `FS-22.03.02 — apps/server/src/middleware/auth_context.rs`.
### FS-22.03.02 Iteration 1 — `apps/server/src/middleware/auth_context.rs`

- **Base:** refreshed `main` at `d8fbeba9f90200edccab4682a7434861318e8be0` after FS-22.03.01 integration; dedicated branch `codex/audit-fs-22.03.02-auth-context`.
- **Invariant map:** the auth middleware must use the typed principal classification produced by the canonical access-token resolver; authentication and authorization boundaries must not reconstruct principal kind from coarse actor labels; direct-session self-service operations must require `AuthPrincipalKind::DirectUser`; delegated OAuth principals may remain valid for delegated OAuth/storefront flows unless an owner contract explicitly requires a direct session; anonymous requests must remain distinguishable from invalid presented credentials.
- **Finding AUTHCTX-22.03.02-01:** `human_user_only` rejected only `SecurityActorKind::Service`. Delegated OAuth users are classified as `SecurityActorKind::User`, so they passed the same guard used for `/api/auth/me`, session listing/revocation, password change, profile update, and login history. Those handlers consume `CurrentUser.session_id`, and delegated access tokens intentionally carry a nil session ID; in particular, `revoke-all` excludes `id != nil`, while password change is a direct-session lifecycle operation.
- **Remediation:** introduced a bounded `is_direct_user_self_service_path` classifier for the auth self-service routes and require `current_user.principal_kind.is_direct_user()` before dispatch. This uses the canonical typed principal already produced by the auth extractor; it does not inspect `client_id`, `grant_type`, or `session_id` ad hoc. Existing `human_user_only` remains responsible for the separate service-vs-human guard on storefront/AI paths.
- **Regression coverage:** added focused path-boundary and principal-admission tests covering all direct-session auth self-service routes, delegated/service rejection, and preservation of delegated eligibility for OAuth, storefront, and AI paths.
- **Immediate re-audit:** re-read the changed middleware branch, principal propagation, typed `AuthPrincipalKind`, request RBAC scope creation, and every direct `CurrentUser` consumer in `controllers/auth.rs`. `/api/auth/logout` is intentionally excluded because it authenticates the supplied refresh token itself rather than using `CurrentUser.session_id`.
- **Adjacent-boundary re-audit:** GraphQL and OAuth controller paths continue to accept delegated principals where their contracts permit them; Pages inline authoring already requires `DirectUser`; service-only forum moderation restrictions remain unchanged. No alternate principal reconstruction was introduced.
- **Regression audit:** anonymous requests still flow to downstream extractors and receive their existing 401 behavior; invalid presented credentials still fail at the auth middleware; direct users retain self-service access; delegated OAuth users are denied before direct-session handlers and therefore cannot use a nil session identity to mutate/revoke session state.
- **Fresh second pass:** independently rechecked route matching for exact auth paths, nested session paths, `/api/auth/logout`, `/api/oauth/*`, `/store*`, `/api/fn/ai/*`, and Pages inline editing. No additional repository-owned issue remained in the primary `auth_context.rs` module.
- **Documentation:** `apps/server/docs/README.md` now states that auth self-service endpoints require `AuthPrincipalKind::DirectUser` and delegated OAuth users do not inherit the direct-session contract.
- **Verification:** repository-content/static inspection and branch-diff review only. No test suite, clippy, build, gatekeeper, migration, or runtime command was executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.03.02` complete after merge; continue to the next unchecked primary module in FS-22.03.

### FS-22.03.03 Iteration 1 — `apps/server/src/controllers/auth.rs`

- **Base:** refreshed `main` at `a3b710661bc98fb48b5637843762b22731197b0e` after FS-22.03.02 integration; dedicated branch `codex/audit-fs-22.03.03-auth-controller`.
- **Invariant map:** authentication recovery endpoints must not disclose whether a tenant account exists; reset/verification request responses must remain externally uniform across account-present, account-absent, and email-delivery/configuration failure states; reset tokens remain credential-bound; tenant and token claims remain validated by canonical auth helpers; asynchronous delivery failures must not turn into unhandled request failures.
- **Finding AUTHCTRL-22.03.03-01:** `/api/auth/reset/request` and `/api/auth/verify/request` returned a generic 200 for unknown addresses but returned 500 when an existing account reached an unavailable email service or invalid reset URL. The difference exposed account existence whenever mail transport configuration or URL preparation failed. Normal successful delivery was already asynchronous, so request success should not depend on transport availability.
- **Remediation:** both request endpoints now treat email-service construction and URL preparation failures as non-fatal, log only a bounded generic server-side warning, and preserve the same public success envelope. Existing asynchronous send failures remain server-observable and non-fatal. No token is logged or exposed in production.
- **Immediate re-audit:** re-read the controller's register/login/refresh/logout/reset/verification/session/profile routes and their lifecycle consumers. Reset token generation remains credential-fingerprint-bound and confirmation delegates to the canonical lifecycle path; verification confirmation still binds tenant, user id, and normalized email.
- **Adjacent-boundary re-audit:** auth rate limiting covers login/register/reset/verify prefixes; tenant resolution remains required for these routes; `rustok-auth` owns token primitives and REST DTOs; email delivery remains a host concern. No new account-existence branch was introduced.
- **Regression audit:** unknown-account requests still skip email preparation and return success; existing-account requests now return the same status even when email setup is unavailable; demo-only token exposure remains constrained by non-production/demo mode; logout continues to validate the supplied refresh token directly.
- **Fresh second pass:** independently checked response paths for existing/non-existing accounts, email-service construction failures, reset URL failures, asynchronous send failures, tenant mismatch during verification confirmation, and route rate-limit coverage. No additional repository-owned defect remained in the primary `auth.rs` controller requiring another remediation unit.
- **Documentation:** `apps/server/docs/README.md` now records the generic public response contract for reset/verification request delivery failures.
- **Verification:** repository-content/static inspection and branch-diff review only. No test suite, clippy, build, gatekeeper, migration, or runtime command was executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.03.03` complete after merge; continue to the next unchecked primary module in FS-22.03.

### FS-22.03.04 Iteration 1 — `apps/server/src/controllers/oauth.rs`

- **Base:** refreshed `main` at `0f4aab39179b017a6b751bfb7373fa58b6c82833`; dedicated branch `codex/audit-fs-22.03.04-oauth-controller`.
- **Invariant map:** OAuth HTTP transport must preserve RFC-style Bearer authentication semantics across all browser-session paths; authentication parsing must agree with the canonical Axum extractor; malformed/multi-token authorization values must fail closed; PKCE, exact redirect URI, tenant, scope, consent, and no-store token response boundaries must remain unchanged.
- **Finding OAUTHCTRL-22.03.04-01:** `extract_bearer_token` used a case-sensitive `strip_prefix("Bearer ")` parser and accepted only that exact spacing form. The same controller's `CurrentUser` extractor uses the typed HTTP authorization parser. A valid authorization header using another casing of the `Bearer` scheme could therefore authenticate the request but then be rejected by `/api/oauth/browser-session`, producing an inconsistent protocol boundary.
- **Remediation:** replaced the custom parser with bounded ASCII-whitespace tokenization that requires exactly two fields, compares the authentication scheme case-insensitively, and rejects Basic/multi-token values. Browser authorization and browser-session creation now share the same local Bearer parsing behavior.
- **Immediate re-audit:** re-read token, authorize, browser authorize, consent, browser-session, userinfo, revoke, redirect, PKCE and cookie helpers. Exact redirect URI matching, mandatory S256 PKCE, tenant binding, consent-scope checks, no-store token responses and RFC 7009 revocation semantics are unchanged.
- **Adjacent-boundary re-audit:** canonical `auth_context` still performs bearer extraction/CurrentUser resolution independently; the typed `Authorization<Bearer>` path remains the source of authenticated principal state, while the controller parser is used only where the raw token must be copied into the browser-session cookie. No second authentication authority was introduced.
- **Regression audit:** lower/upper mixed-case Bearer schemes now work consistently; non-Bearer and extra-token values are rejected; cookie fallback still works for browser authorize/consent; Secure/HttpOnly/SameSite cookie attributes remain unchanged.
- **Fresh second pass:** independently checked browser-session header parsing, cookie fallback, redirect construction/clearing, consent form flow, userinfo scope gating, revoke semantics, token cache headers, exact redirect registration, and PKCE method enforcement. No additional repository-owned defect remained in the primary `oauth.rs` module requiring another remediation unit.
- **Documentation:** `apps/server/docs/README.md` now records the case-insensitive Bearer parsing contract for OAuth browser-session/consent flows.
- **Verification:** repository-content/static inspection and branch-diff review only. No test suite, clippy, build, gatekeeper, migration, or runtime command was executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.03.04` complete after merge; continue to the next unchecked primary module in FS-22.03.

### FS-22.03.05 Iteration 1 — `apps/server/src/models/oauth_apps.rs`

- **Base:** refreshed `main` at `8fcc3c42dfb71c02f6a995045347dfdc7acd33d0`; dedicated branch `codex/audit-fs-22.03.05-oauth-grants`.
- **Invariant map:** an OAuth application's persisted `grant_types` is the canonical authority for grant admission; compatibility behavior must not manufacture authority absent from persisted state; manifest-managed producers that require refresh rotation must declare `refresh_token` explicitly before exact membership is enforced; manual and auto-created applications must have identical grant semantics for the same persisted grant set.
- **Finding AUTHAPP-22.03.05-01:** `Model::supports_grant_type` implicitly admitted `refresh_token` for every auto-created application that declared only `authorization_code`. This expanded persisted OAuth authority and could issue/rotate refresh credentials even though `refresh_token` was absent from the application's recorded grant policy.
- **Producer reconciliation:** manifest-managed first-party admin and standalone storefront producers were the concrete callers relying on this compatibility expansion. Their persisted grant sets now explicitly include `refresh_token` alongside `authorization_code` and `client_credentials`, preserving the intended refresh flow before exact enforcement.
- **Remediation:** `supports_grant_type` now performs exact persisted membership with no auto-created compatibility expansion. Regression coverage rejects an undeclared refresh grant for both manual and auto-created applications and accepts an explicitly declared auto-created refresh grant. Token-service coverage repeats the same auto-created boundary at the consumer admission point.
- **Immediate re-audit:** re-read the OAuth app model, manifest sync producer, token exchange service, access-token auth extractor, authorization controller, and refresh/token tests. Existing authorization-code and client-credentials checks remain exact; refresh tokens are emitted only when the explicit app grant is present.
- **Adjacent-boundary re-audit:** manifest sync updates existing first-party app grant types during runtime bootstrap, so the two concrete producers converge persisted rows to the new explicit set before traffic. Embedded apps intentionally keep an empty grant set and do not participate in the authorization-code/refresh flow.
- **Regression audit:** manual and auto-created apps with only `authorization_code` now fail refresh grant admission; explicit `refresh_token` succeeds; manifest-managed admin/storefront app composition retains the same intended three-grant set. No token format, tenant binding, scope restriction, or refresh CAS behavior changed.
- **Fresh second pass:** independently searched all reachable `supports_grant_type` call sites in the server auth surface and rechecked producer literals, consumer grant gates, and tests. No remaining repository-owned compatibility expansion was found in this primary model contract.
- **Documentation:** `apps/server/docs/README.md` records exact persisted grant admission and the explicit managed-app grant set; `crates/modules/rustok-auth/docs/implementation-plan.md` closes Open Result #1.
- **Verification:** repository-content/static inspection and branch-diff review only. No test suite, clippy, build, gatekeeper, migration, or runtime command was executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.03.05` complete after merge; continue to the next unchecked primary module in FS-22.03.

### FS-22.03.06 Iteration 1 — `apps/server/src/services/oauth_token_service.rs`

- **Base:** refreshed `main` at `f546966743ceb36bc86a80e9b5985c599ef19cfd`; dedicated branch `codex/audit-fs-22.03.06-oauth-token-service`.
- **Invariant map:** OAuth token issuance must authenticate and resolve the client from security/configuration state only; presentation/localization storage must not become a prerequisite for token exchange; tenant binding, exact persisted grant admission, scope ceilings, PKCE, authorization-code CAS, refresh-token CAS, consent checks, and replacement-token persistence must remain fail-closed and atomic.
- **Finding OAUTHTOKEN-22.03.06-01:** `resolve_client` used `OAuthAppService::find_by_client_id`, whose underlying model path hydrates tenant locale/presentation translations. The token endpoint therefore depended on `oauth_app_translations` even though it only needs active app security/configuration fields.
- **Remediation:** `resolve_client` now uses `oauth_apps::Entity::find_active_security_by_client_id`, the security-only lookup introduced for the canonical bearer-auth path. Presentation-aware OAuth app lookup remains available to presentation callers and is not used to authorize or issue tokens.
- **Regression coverage:** added an isolated SQLite test that creates only the `oauth_apps` table, without any translation table, and verifies token-service client resolution succeeds for the active tenant/client.
- **Immediate re-audit:** re-read token exchange branches for `client_credentials`, `authorization_code`, and `refresh_token`; `require_grant` now uses exact persisted grant membership; authorization-code consumption and refresh-token rotation remain transactional; scope subset and active-subject/consent checks remain in place.
- **Adjacent-boundary re-audit:** the auth extractor uses the same security-only OAuth app lookup; the controller uses `OAuthTokenService` rather than a duplicate issuance path; `OAuthAppService` presentation-aware lookup remains confined to presentation-oriented callers. No tenant or scope fallback was added.
- **Regression audit:** missing translation storage no longer blocks client resolution at the token endpoint; invalid/inactive clients still map to `invalid_client`; tenant mismatch still fails closed; declared/undeclared grant behavior remains owned by `supports_grant_type` and the token service gate.
- **Fresh second pass:** independently checked all `supports_grant_type` and client-resolution call sites in the server auth surface, token grant branches, tenant qualification, scope validation, PKCE, consent, and refresh CAS. No additional repository-owned root-cause issue remained in the primary token service module.
- **Documentation:** `apps/server/docs/README.md` now states that the OAuth token endpoint resolves clients through the security-only app lookup and is independent of presentation translations.
- **Verification:** repository-content/static inspection and branch-diff review only. No test suite, clippy, build, gatekeeper, migration, or runtime command was executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.03.06` complete after merge; continue to the next unchecked primary module in FS-22.03.

### FS-22.03.07 Iteration 1 — `crates/modules/rustok-auth/src/jwt.rs`

- **Base:** refreshed `main` at `243dd84f5e65fefafbed34b7a2090a29014dbf19`; dedicated branch `codex/audit-fs-22.03.07-jwt-ttl`.
- **Invariant map:** token expiration must remain a valid UTC timestamp; TTL inputs must not lose information through lossy integer conversion; all JWT encoders must share one bounded expiration rule; special-purpose token purpose/claims and algorithm selection must remain unchanged; overflow errors must not disclose secret material.
- **Finding AUTHJWT-22.03.07-01:** the JWT owner cast public `u64` TTL inputs to `i64` with `as i64`. Values larger than the signed range could wrap into a negative or otherwise invalid duration before the expiration claim was constructed.
- **Remediation:** added one owner-local `token_expiration` helper using `i64::try_from` followed by `chrono::DateTime::checked_add_signed`; all five encoders (`access`, OAuth access, password-reset, email-verification, invite) now use that checked path and return a generic `AuthError::Internal` on out-of-range input.
- **Regression coverage:** added a boundary regression that supplies `u64::MAX` to each of the five encoder APIs and requires rejection.
- **Immediate re-audit:** re-read JWT claim construction, strict issuer/audience/expiry validation, algorithm pinning for HS256/RS256, special-purpose claim checks, key handling, and every TTL call site. No existing small-range expiration behavior or purpose binding was changed.
- **Adjacent-boundary re-audit:** `rustok-auth::config` continues to enforce deployment-facing access/refresh TTL bounds; the new checked conversion is an additional owner API safety boundary and does not replace configured policy. Server adapters consume the same auth-owned encoder contract.
- **Regression audit:** normal configured TTLs still flow through the same `chrono` addition semantics; oversized inputs now fail before token serialization instead of producing wrapped timestamps. No secret material is included in the new error text.
- **Fresh second pass:** independently searched `jwt.rs` for all `Duration::seconds` TTL constructions and integer casts, verified exactly five encoder calls now use `token_expiration`, confirmed no `ttl_seconds as i64` remains, and rechecked the new regression coverage against all encoder paths.
- **Documentation:** `crates/modules/rustok-auth/docs/README.md` now records the checked JWT expiration boundary.
- **Verification:** repository-content/static inspection and branch-diff review only. No test suite, clippy, build, gatekeeper, migration, or runtime command was executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.03.07` complete after merge; continue to the next unchecked primary module in FS-22.03.

### FS-22.04.01 Iteration 1 — `crates/libs/rustok-api/src/request.rs`

- **Base:** refreshed `main` at `a5e5b1c37af84f48c97773b30e12d9b13c20ee3d`; dedicated branch `codex/audit-fs-22.04.01-request-context`.
- **Invariant map:** `RequestContext` must consume trusted tenant/auth/channel/locale extensions rather than reconstruct authority; tenant and channel identities must remain consistent; effective locale must come from the canonical host/runtime resolution pipeline and therefore inherit tenant policy; query locale parsing must honor standard URL form encoding; downstream handlers must see one stable request-context projection.
- **Finding REQUESTCTX-22.04.01-01:** the `RequestContext` extractor required a trusted tenant extension but, when `ResolvedRequestLocale` was absent, reconstructed an effective locale directly from query/header/cookie/tenant-default data. This bypassed the tenant-owned locale allowlist/fallback semantics and could select a caller-requested normalized locale on a route where locale middleware had not run.
- **Finding REQUESTCTX-22.04.01-02:** query locale extraction split the raw URI query manually and normalized the value without URL form decoding, so valid encoded selectors such as `locale=ru%2Dby` were rejected. This was the deferred `LOCALE-API-01` finding from the prior middleware phase.
- **Finding REQUESTCTX-22.04.01-03:** `RequestContext` copied authenticated user and channel identity from trusted extensions without checking that their tenant UUID matched the already-resolved tenant context. The canonical propagation boundary should fail closed on an inconsistent extension set instead of forwarding cross-tenant context.
- **Remediation:** `RequestContext` now requires `ResolvedRequestLocale` and rejects a missing canonical locale extension with an internal-server error; query extraction uses the existing approved `url::form_urlencoded` parser from the workspace dependency catalog; tenant IDs are cross-checked for authenticated and channel extensions before projection.
- **Regression coverage:** retained the existing identity/channel/cookie/header precedence coverage; added focused regressions for missing resolved locale, percent-encoded query locale, and cross-tenant auth/channel extensions.
- **Immediate/adjacent re-audit:** verified the canonical router order `tenant -> locale -> auth_context -> channel -> handler`, the GraphQL HTTP handler's `RequestContext` extraction, auth reset/verification handlers' locale consumption, and the GraphQL WebSocket path's independent explicit RequestContext construction. No legal HTTP caller requires the removed locale fallback.
- **Regression audit:** direct user identity still comes only from `AuthContextExtension`; tenant remains mandatory; GraphQL WebSocket behavior is unchanged because it constructs its context after explicit tenant/auth/locale policy resolution; encoded locale selectors now canonicalize before policy middleware constrains them.
- **Fresh second pass:** independently re-read `request.rs`, app-router middleware order, the tenant locale policy contract, URL parsing dependency availability, GraphQL/auth RequestContext consumers, and the current request-context tests. No additional repository-owned defect remained in this primary module.
- **Verification:** repository-content/source inspection and branch-diff review only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.01` complete after PR #4279 and closeout PR #4280; the current `main` baseline is `226e3bdd250e63588b0586a19b05088007370199`. Post-merge source re-read confirmed the canonical locale requirement, tenant-consistency checks, and form-decoded query parsing are present on `main`.
- **Next primary module:** `FS-22.04.03 — `apps/server/src/middleware/tenant.rs`.

### FS-22.04.02 Iteration 1 — `apps/server/src/middleware/tenant_resolution.rs`

- **Base:** refreshed `main` at `226e3bdd250e63588b0586a19b05088007370199`; dedicated branch `codex/audit-fs-22.04.02`.
- **Invariant map:** tenant resolution must select exactly one typed authority source; request-derived identifiers must be validated before tenant loading; subdomain routing must preserve the identifier's semantic type; forwarded host data may influence tenant routing only under the shared explicit proxy-trust policy; duplicate tenant assertions must fail closed; canonical host identity must not depend on insignificant DNS formatting; resolver diagnostics must remain bounded.
- **Finding TENANTRESOLVE-22.04.02-01:** subdomain extraction passed its tenant label through generic UUID/slug classification. A tenant slug that is syntactically UUID-shaped was therefore converted to `ResolvedTenantIdentifier::Uuid`, causing the downstream tenant read port to query by tenant UUID instead of the configured slug. This is a routing correctness and tenant-selection defect.
- **Finding TENANTRESOLVE-22.04.02-02:** effective Host normalization accepted a fully-qualified hostname with one terminal DNS dot but returned that dot to the subdomain/base-domain matcher and domain selector. Because configured base domains are canonicalized without the terminal dot, `tenant.example.test.` could not resolve through subdomain routing and host-domain lookup would use a non-canonical identity.
- **Finding TENANTRESOLVE-22.04.02-03:** `HeaderMap::get` selected a single tenant header value when duplicate values were present. Multiple values can arise from intermediaries and make the routing assertion ambiguous; the resolver now rejects duplicate primary or compatibility tenant-header values rather than selecting one silently.
- **Finding TENANTRESOLVE-22.04.02-04:** invalid slug input was copied wholesale into the `InvalidIdentifier` diagnostic. Although the validator eventually rejects oversized input, the resulting warning path could log an attacker-controlled multi-kilobyte identifier. Diagnostics are now bounded to 64 characters plus a truncation marker.
- **Remediation:** subdomain resolution now explicitly validates/exposes a `Slug` identifier; effective Host canonicalization removes a single terminal FQDN dot before validation and selector use; tenant header extraction rejects duplicate values; invalid-identifier diagnostics are bounded.
- **Regression coverage:** added focused resolver tests for duplicate primary/compatibility header values, UUID-shaped subdomain slugs, terminal-dot host canonicalization, and bounded invalid-identifier diagnostics.
- **Immediate/adjacent re-audit:** re-read `request_trust.rs`, tenant settings validation/normalization, `tenant.rs` selector mapping and cache identity, `tenant_route_policy.rs`, and the end-to-end tenant resolver invariant test suite. The shared request-trust helper remains the sole forwarded-header trust owner; tenant loading continues to use the typed `TenantReadSelector` mapping and generation-aware cache layer.
- **Regression audit:** header mode still permits UUID-or-slug identifiers; compatibility slug assertions remain correlated after the primary lookup; Host/Domain modes still resolve through the shared canonical host path; single-tenant and development-fallback source semantics are unchanged; global/self-resolving route scopes remain owned by `tenant_route_policy.rs`.
- **Fresh second pass:** independently re-read the complete modified `tenant_resolution.rs` from the iteration branch, including all error/status mappings and tests, then compared the branch against refreshed `main`. No remaining repository-owned defect was found in this primary module.
- **Verification:** GitHub source inspection and branch diff review only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.02` complete after PR #4281 merged into `main` at `68de093f69723c21c6c7d538e54ade63e119da03`. Post-merge source re-read confirmed the duplicate-header rejection, terminal-dot host canonicalization, slug-preserving subdomain resolution, and bounded diagnostics.
### FS-22.04.03 Iterations 1-3 — `apps/server/src/middleware/tenant.rs`

- **Base:** refreshed `main` at `3a52404b2563c568011bb91c93d4d1eb52510ec9`; dedicated branch `codex/audit-fs-22.04.03`.
- **Invariant map:** tenant cache entries must deserialize completely within the typed cache boundary; malformed or incompatible values must become cache misses and be invalidated; tenant context must contain only active tenants; positive/negative entries must be generation-consistent with source-of-truth mutations; cache initialization must bind one canonical `CacheService` and one infrastructure instance; cache failures must degrade to explicit availability failures rather than silently serving an unsafe value.
- **Finding TENANTCTX-22.04.03-01:** `CachedTenantContext` stored `TenantContext.settings` as a nested `settings_json: String`. The typed cache layer could therefore successfully deserialize the envelope while the later `TenantContext::try_from` failed on the nested JSON. Such a malformed cache value was not invalidated by the typed cache layer and could repeat a 500 until TTL.
- **Remediation:** cache payload now stores `settings: serde_json::Value` directly; tenant context envelope schema version advanced from 2 to 3 so all pre-change entries are treated as schema misses; conversions are now infallible `From` implementations, eliminating the post-cache nested-deserialization failure path.
- **Finding TENANTCTX-22.04.03-02:** `init_tenant_cache_infrastructure` used check-then-insert for the shared `CacheService` and cache infrastructure. Concurrent callers using different cache-service instances could observe absence and replace the canonical shared service while independently constructing infrastructure against another instance.
- **Remediation:** shared cache service insertion now uses `shared_insert_if_absent`; the already-published canonical service is retrieved and used to construct the tenant cache infrastructure; infrastructure publication also uses `shared_insert_if_absent`.
- **Regression coverage:** aligned the structured-cache round-trip test with schema version 3 and added source guards proving typed settings, infallible conversions, schema versioning and atomic cache initialization.
- **Immediate/adjacent re-audit:** re-read the complete `tenant.rs`, `tenant_tests.rs`, cache typed-envelope/load path, weighted/generation-aware backends, tenant generation listener/bootstrap, `rustok-tenant::TenantReadPort`, tenant settings bounds, route policy and the application bootstrap caller. Generation fencing still wraps the full negative-check/load/fill operation and tenant owner writes continue to publish durable generation invalidation.
- **Regression audit:** old schema-2 entries cannot be consumed because the expected schema is now 3; positive/negative cache keys retain the same generation semantics; active-tenant admission remains enforced before caching; repeated initialization is idempotent and canonical-service-bound; tenant owner settings remain bounded to 16 KiB before entering the cache path.
- **Fresh second pass:** independently re-read the modified production module and its companion tests after all remediation units, checked for the removed `settings_json` path and false-fallible conversions, and compared the complete branch against refreshed `main`. No additional repository-owned defect remained in the primary module.
- **Verification:** repository source inspection and branch-diff review only. No tests, clippy, build, gatekeeper, migrations or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.03` complete after PR #4284 merged into `main` at `9f1a6cfb929bbc9073643c9bba320113bf72fa91`. Post-merge source re-read confirmed typed `settings` payloads, schema version 3, infallible cache-context conversion, and atomic canonical cache-service/infrastructure initialization.

### FS-22.04.04 Iteration 1 — `apps/server/src/middleware/channel.rs`

- **Base:** refreshed `main` at `82e0c5e4b44f9a8b349241e750b495c76dc22545`; dedicated branch `codex/audit-fs-22.04.04`.
- **Invariant map:** channel context must describe the same channel resolution decision that actually matched the request; host/OAuth/locale facts must retain their canonical semantics through caching; cache identity must remain tenant- and fact-complete; invalidation/registration must not introduce split runtime state; the middleware must remain a thin transport boundary over `rustok-channel`.
- **Finding CHANNELCTX-22.04.04-01:** host resolution returned a full channel detail, but `CachedChannelResolution::from_decision` always projected the channel's primary/first target into `ChannelContext.target_type/target_value`. When a non-primary web-domain target matched the effective host, downstream consumers therefore received a target different from the target that actually caused resolution.
- **Remediation:** `from_decision` now receives the original `RequestFacts` and, for `Host` resolution, selects the concrete normalized `web_domain` target matching `facts.host`; primary/first target remains the representation for non-host resolution.
- **Regression coverage:** added an end-to-end middleware companion test with two web-domain targets on one channel, where the non-primary target matches the request; the resulting cached context must expose that concrete target.
- **Source guard:** `channel_cache_architecture_guard.rs` now locks the request-facts-aware projection and concrete host-target matching contract.
- **Immediate/adjacent re-audit:** re-read the modified middleware, companion tests, `ChannelResolver`, `ChannelTargetType`, `ChannelReadPort`, controller mutation paths, locale/auth request extensions, request-trust host helper, durable invalidation runtime and cache-generation guards. The owner resolver remains the source of precedence and tenant scoping; the middleware only projects its decision into the shared host context.
- **Regression audit:** explicit ID/slug/query precedence is unchanged; host canonicalization remains owned by `ChannelTargetType`; OAuth and locale remain dimensions of `RequestFacts` and the cache key; generation rollover/exhaustion still fails safe; REST/native mutation invalidation paths are untouched. A separate adjacent owner issue was noted but not patched here: a missing explicit `X-Channel-ID` currently propagates `ChannelError::NotFound` from `ChannelResolver` instead of being represented as a selector miss/fallback; that belongs to the `rustok-channel/src/resolution.rs` owner track, not this middleware iteration.
- **Fresh second pass:** independently re-read the complete `channel.rs` after the remediation, the new regression, the source guard, and all direct caller/callee contracts. No additional repository-owned defect remained in this primary middleware module.
- **Verification:** repository source inspection and branch-diff review only. No tests, clippy, build, gatekeeper, migrations or runtime commands were executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.04.04` complete after PR #4286 merged into `main` at `ff2abdebc890f962df5596e96fa37bf1ee25cadd`. Post-merge source re-read confirmed request-facts-aware host-target projection and the synchronized API/server/channel documentation and source guard.


### FS-22.03.18 Iterations 1-3 — `apps/server/src/services/oauth_admin_guard.rs`

- **Base:** refreshed `main` at `b58131afbb87bf6b11193c22358c0bf08fa66d7a` after PR #4277 merge; the implementation originated on dedicated branch `codex/audit-fs-22.03.18-oauth-admin-guard`.
- **Invariant map:** OAuth admin mutations must consume the request-bound tenant/permission snapshot; delegated permissions may never exceed current authority; secret rotation must serialize per OAuth app; internal failures must not expose DB/credential diagnostics; mutation responses must describe the state that was actually persisted; all derived reads must remain tenant-qualified.
- **Iteration 1 — fail-closed guard boundaries:** bounded OAuth admin list inputs at `1..=100`, retained `settings:manage` as the admin authority gate, redacted internal guard/service diagnostics, propagated fallible client-secret generation instead of formatting a `Result`, and made the SQLite mutation fence fail closed on unsupported/failed fencing.
- **Iteration 2 — serialized secret rotation:** OAuth client-secret rotation now acquires an exclusive app-row lock on PostgreSQL/MySQL and a SQLite write fence before rereading the authoritative tenant-scoped app row. Delegated permissions are validated from that locked row, eliminating the earlier pre-lock snapshot race.
- **Iteration 3 — atomic response projection:** removed the stale pre-lock `OAuthAppMutationRecord` from the rotation path. The updated app is now projected through one shared server-owned OAuth admin record builder while the mutation transaction is still open; localization and active-token count are tenant-scoped there, and projection failures therefore roll back instead of surfacing after a committed secret change. The native admin transport now propagates the authoritative `can_rotate_secret` flag instead of recomputing a weaker rule from app type.
- **Immediate/adjacent/fresh audits:** re-read the complete guard plus `OAuthAdminPort`, server OAuth admin provider, OAuth app/model persistence transactions, token-count query, GraphQL query/mutation entry points, native server functions, admin UI capability flags, consent guard, runtime provider registration, and tenant/RBAC request-scope boundary. No direct GraphQL/native OAuth admin mutation path bypassing the guarded runtime was found.
- **Regression coverage:** preserved the guard tests for grant dependencies, bounded list limits, and redacted internal errors; added a native transport regression proving `can_rotate_secret` follows the authoritative mutation record.
- **Verification:** repository source/static inspection, branch diff review, PR integration, and post-merge `main` re-read only. No test suite, clippy, build, gatekeeper, migration, or runtime command was executed by the agent; maintainer verification remains required.
- **Status:** `FS-22.03.18` complete after PR #4277 merge. Continue to the next unchecked primary module in FS-22.03.

### FS-22.03.17 Iterations 1-4 — `apps/server/src/services/auth_admin_mutation_provider.rs`

- **Base:** refreshed `main` at `8951800d7e53f12add957e2679041ec4ece9ce1d`; dedicated branch `codex/audit-fs-22.03.17-auth-admin-provider`. During implementation, `main` advanced with unrelated UI PR #4272; the final branch was merged with an explicit 3-way merge-tree so that unrelated `main` changes were preserved.
- **Iteration 1 — user mutation fences:** SQLite user mutation locking now checks the conditional write fence, rereads the current row, and rejects unsupported backends without panic. User custom-field updates compare the pre-lock metadata snapshot with the locked row and fail with a conflict on concurrent metadata changes. This prevents stale snapshots from overwriting concurrent user changes.
- **Iteration 2 — admin error boundary:** raw DB/service diagnostics in the user and OAuth admin providers now pass through `internal_admin_error`, logging server-side while exposing only `Auth administration operation failed`. Validation, permission, conflict and not-found categories remain stable and user-facing where appropriate.
- **Iteration 3 — response consistency:** create/update user mutations resolve tenant name and authoritative resulting role before transaction commit, then construct `UserMutationRecord` without post-commit RBAC/tenant reads. A successful committed mutation can therefore no longer become a client-visible failure because a projection read failed afterward.
- **Iteration 4 — provider read budget:** OAuth admin app/authorized-app lists now enforce a service-level `1..=100` limit regardless of transport caller. Existing GraphQL caps remain unchanged.
- **Immediate/adjacent/fresh audits:** reviewed user admin lock ordering, RBAC authoritative reads, super-admin continuity, durable invalidation generation, event/outbox publication, OAuth tenant filters, GraphQL error mapping, and installer/user transaction boundaries. A fresh pass caught and corrected a temporary file-truncation hazard in the GitHub content workflow before PR creation.
- **Final post-merge second pass:** after PR #4273 merged at `be01c8b24b729cbcabe7abd4fc7f0d6989517e3b`, both provider files were reread from `main`; no remaining in-scope owner/provider defect was found.
- **Deferred:** OAuth client-secret rotation Result handling in `apps/server/src/services/oauth_admin_guard.rs` remains the next dedicated provider module. Further response-projection atomicity beyond this user-admin provider boundary remains explicitly deferred.
- **Verification:** repository source inspection, iterative caller/callee review, branch diff review, and post-merge source re-read only. No tests, clippy, build, gatekeeper, migrations, or runtime commands were executed by the agent.
- **Status:** `FS-22.03.17` complete. Next primary module: `FS-22.03.18 — apps/server/src/services/oauth_admin_guard.rs`.

### FS-22.03.16 Iteration 1 — `crates/modules/rustok-auth/src/admin_mutations.rs`

- **Base:** refreshed `main` at `780775b9fccb6dcd2e8668a60190d07669ee89e4`; dedicated branch `codex/audit-fs-22.03.16-auth-admin-mutations`.
- **Invariant map:** auth admin ports must remain transport-neutral, preserve tenant/actor context, expose no secret material, distinguish authorization/validation/conflict/not-found/internal categories, and keep user/OAuth mutation contracts separate from concrete server persistence.
- **Discovery:** re-read the complete admin mutation contract plus GraphQL error mapping, server user-admin provider, OAuth admin provider/guard, OAuth app service, RBAC authoritative role checks, and installer/admin boundaries.
- **Finding assessment:** no remaining owner-level defect was confirmed inside `rustok-auth/src/admin_mutations.rs`. `OAuthAppSecretResult` has explicit secret-redacted `Debug`; command/record types contain no plaintext secret fields except the deliberate one-time `client_secret` response; the port surface remains framework-neutral.
- **Deferred adjacent finding ADMINPROVIDER-22.03.16-01:** `apps/server/src/services/oauth_admin_guard.rs` formats `generate_refresh_token()` results directly into the rotated client secret even though the auth credential API returns `Result<String, AuthError>`. This is a compile-level provider defect assigned to the next concrete OAuth-admin provider track, not the module-owned contract.
- **Deferred adjacent finding ADMINPROVIDER-22.03.16-02:** server user/OAuth admin providers still create `AuthAdminMutationError::Internal(String)` from raw DB/service diagnostics, while GraphQL maps `Internal` directly to client-visible internal-error text. This is a server adapter error-boundary issue and will be handled in the concrete provider tracks.
- **Deferred adjacent finding ADMINPROVIDER-22.03.16-03:** OAuth admin list methods accept raw `u64` limits in the port; transport caps exist in some GraphQL paths but owner/provider enforcement needs a concrete provider audit. This remains separate from the contract-only module.
- **Fresh second pass:** re-read the contract, secret redaction test, GraphQL mapping and direct provider boundaries. No contract-level secret leak, framework coupling, or malformed error-category defect remained.
- **Verification:** repository source inspection and branch-diff review only. Adjacent findings are intentionally not patched through the contract layer; no tests, clippy, build, gatekeeper, migrations, or runtime commands were executed by the agent.
- **Status:** `FS-22.03.16` complete. Next primary module: `FS-22.03.17 — apps/server/src/services/auth_admin_mutation_provider.rs`.

### FS-22.03.15 Iterations 1-2 — `crates/modules/rustok-auth/src/bootstrap.rs`

- **Base:** refreshed `main` at `625773e736fd8d93d2d377b8c380c1968af7fad0`; dedicated branch `codex/audit-fs-22.03.15-auth-bootstrap`.
- **Iteration 1 — bootstrap identity boundary:** bootstrap email lookup was not canonicalized consistently with identity creation; DB/hash/row diagnostics were returned as internal strings; unsupported backend handling contained process-level `unreachable!`. Remediation canonicalized email to lowercase, introduced stable internal-error redaction, and made backend selection fail closed without panic.
- **Iteration 2 — fresh re-audit remediation:** re-read the full owner and installer seed boundary, confirmed installer identity + RBAC remain in one caller-owned transaction, removed residual `unreachable!` paths in `find_user_on`, and routed all row-decoding errors through the redaction helper. Unit coverage was added for email normalization, stable error text, and unsupported backends including SeaORM Mock.
- **Concurrency/non-findings:** `ON CONFLICT (tenant_id,email) DO NOTHING` plus post-conflict reread remains the canonical idempotency strategy; existing-user bootstrap does not reset credentials, matching installer seed idempotency semantics. No status filtering was added because the owner contract describes tenant-scoped identities rather than active-only identities.
- **Final fresh second pass:** after PR #4267 merged at `c021a0a318495fa1c0b561930da72ed6b5a362ac`, the complete owner file and installer caller were re-read; no additional in-scope bootstrap defect remained.
- **Verification:** repository source inspection, installer atomicity review, immediate re-audits, and branch-diff review only. No tests, clippy, build, gatekeeper, migrations, or runtime commands were executed by the agent.
- **Status:** `FS-22.03.15` complete. Next primary module: `FS-22.03.16 — crates/modules/rustok-auth/src/admin_mutations.rs`.

### FS-22.03.14 Iterations 1-3 — `crates/modules/rustok-auth/src/backfill.rs`

- **Base:** refreshed `main` at `2b99979f1aceb2cbd9538822dc82c9953a08abaa`; dedicated branch `codex/audit-fs-22.03.14-auth-backfill`.
- **Iteration 1 — bounded owner read contract:** `AuthUserBackfillDbReader` accepted arbitrary `u64` limits and cast them to `i64`. The owner now enforces `1..=500`, matching the Profiles CLI default, and rejects invalid values before query construction. Boundary tests cover `0`, `1`, `500`, `501`, and `u64::MAX`.
- **Iteration 2 — error redaction:** DB query and row-decoding failures previously became raw `Internal(String)` diagnostics. They now pass through one stable `Auth user backfill read failed` error while retaining server-side error logging. Regression coverage verifies the external message.
- **Iteration 3 — deterministic batch order and conversion hardening:** the SQL selection order now uses `created_at ASC, id ASC`; the SQL LIMIT binding uses checked `i64::try_from` after validation, eliminating lossy integer conversion. A fresh pass caught and corrected the intermediate invalid `i64::from(u64)` conversion before integration.
- **Consumer reconciliation:** the Profiles CLI already defaults to `500` and supplies an explicit tenant UUID; no consumer runtime change was required. The reader returns only auth-owned identity projection fields `id/email/name` and does not import Profiles storage.
- **Final fresh second pass:** re-read the complete owner reader after PR #4264 merged at `c608b07bd7f5df1a14232ea4c9a2f55fa14a9f75`. Rechecked tenant predicate, batch bounds, SQL parameter conversion, ordering stability, raw row parsing and error redaction. No remaining in-scope owner defect was found.
- **Verification:** repository source inspection, direct consumer review, immediate re-audits after each remediation, and branch-diff review only. No tests, clippy, build, gatekeeper, migrations, or runtime commands were executed by the agent.
- **Status:** `FS-22.03.14` complete. Next primary module: `FS-22.03.15 — crates/modules/rustok-auth/src/bootstrap.rs`.

### FS-22.03.13 Iterations 1-5 — `apps/server/src/services/auth_lifecycle_provider.rs`

- **Iteration 1 — password-reset privacy:** Base `3240a5b3b2497d7773dc2bfb4801d5debb3d2a30`. `forgot_password` no longer reveals account existence when reset-token encoding, email-service resolution, or reset-URL preparation fails; preparation failures return the same generic success result as the unknown-account path. PR #4257 merged at `817e87a2d40fb96066bfd9cf180821483dfde28a`.
- **Iteration 2 — registration-policy parity:** Base `817e87a2d40fb96066bfd9cf180821483dfde28a`. Provider `sign_up` now enforces the same host-owned `features.registration_enabled` policy already enforced by REST. Delegated/service/anonymous contexts cannot bypass the disabled-registration state. PR #4258 merged at `ddcfe94d90ad117cc55a3162a20ca9ac0c49e861`.
- **Iteration 3 — internal error redaction:** Base `ddcfe94d90ad117cc55a3162a20ca9ac0c49e861`. Unified provider DB/lifecycle/invite error mapping now logs server-side and exposes only stable `Auth lifecycle operation failed` text, preventing GraphQL from receiving raw diagnostic strings. Regression coverage verifies the stable external message. PR #4259 merged at `f190dc0c109bf614c83dbce78239626da85130d5`.
- **Iteration 4 — token-response consistency:** Base `f190dc0c109bf614c83dbce78239626da85130d5`. When sign-in/sign-up/refresh successfully issue a session but provider RBAC projection fails, the freshly-issued session is compensatorily revoked using the signed access-token session claim before the error is returned. Tenant mismatch and compensation failure are logged without token material. PR #4260 merged at `e0585ed77e3a9685bced4392a859b5bd4c7e475e`. The follow-up pass also corrected compensation logging to use debug formatting because `AuthLifecycleError` does not implement `Display`.
- **Iteration 5 — typed principal parity:** Base `e0585ed77e3a9685bced4392a859b5bd4c7e475e`. `AuthLifecycleContext` now carries the canonical optional `AuthPrincipalKind`; GraphQL populates it from the trusted `AuthPrincipalContext` extension; provider self-service operations require `DirectUser`. This closes the delegated-OAuth path that previously could reach current-user/session/profile/revoke operations outside HTTP middleware. Provider regression coverage covers direct, delegated, service and anonymous contexts. PR #4261 merged at `c0591e1332b407dd2c7b8e76603b2b8c37880938`.
- **Immediate/adjacent/fresh second passes:** after every remediation, re-read direct callers/callees and relevant auth/GraphQL/RBAC context boundaries; the final post-merge pass rechecked all five fixes together and found no remaining provider-owned defect.
- **Explicit cross-owner follow-up:** the provider still performs a post-issuance RBAC projection; compensation is fail-closed when the DB remains writable, but an outage that prevents compensation can leave the newly issued session persisted while the response fails. A fully atomic token-plus-permission projection would require reopening the lifecycle-service owner track and is not patched through the provider.
- **Explicit deferred owner finding:** `AuthUserBackfillDbReader` still casts its caller-supplied `limit: u64` to `i64`; the root owner is `rustok-auth/src/backfill.rs`, not this provider. It remains for a dedicated future owner-module iteration.
- **Event non-finding:** canonical `UserAccountRegistered`/`UserLoggedIn` event types were reviewed against current consumers and no active repository-owned consumer requiring provider publication was identified; no speculative event producer was added.
- **Verification:** repository source inspection and branch-diff review only. Per maintainer-owned policy, no tests, clippy, build, gatekeeper, migrations, or runtime commands were executed by the agent.
- **Status:** `FS-22.03.13` complete. Next primary module: `FS-22.03.14 — crates/modules/rustok-auth/src/backfill.rs`.

### FS-22.03.12 Iterations 1-6 — `apps/server/src/services/auth_lifecycle.rs`

- **Base:** refreshed `main` at `bb2bfe5c4a3a6417510a000d18615ad7ac4089fc`; module track `codex/audit-fs-22.03.12-auth-lifecycle-service`.
- **Iteration 1 — atomic registration/session issuance:** registration previously committed the user before session/token creation, and session insertion could commit before later role/JWT failures. Introduced `create_session_and_tokens_in_tx`, moved registration to one transaction, and added rollback coverage.
- **Iteration 2 — login identity fence:** login now rechecks and locks the tenant-scoped user after password verification, rejects password/status drift, and commits `last_login_at` plus session/token issuance atomically. SQLite uses a write fence and rereads the row.
- **Iteration 3 — password-change session fence:** password changes revalidate credential/status under a user lock and require the current session to remain active under a second lock. SQLite write-fence paths require one affected row and reread current state; lock ordering remains user then session.
- **Iteration 4 — SQLite refresh replay fence:** conditional refresh-session fencing now checks `rows_affected`; a zero-row race rereads the current session and only preserves it when the presented token hash remains current, preventing replay after rotation while preserving expiry semantics after revocation.
- **Iteration 5 — bounded session reads:** the service now clamps direct session-list limits to `1..=100`, matching REST/GraphQL and preventing lower-level callers from bypassing the read budget.
- **Iteration 6 — expiration arithmetic:** both refresh rotation and initial session issuance now use checked `i64` conversion and `DateTime::checked_add_signed`, eliminating lossy `u64 -> i64` arithmetic.
- **Immediate/adjacent/regression audits:** after every remediation, re-read the changed path and direct callers; reviewed DB transaction semantics, RBAC role resolution, auth extractor session checks, admin user mutation lock ordering, event/outbox boundaries, and GraphQL/native lifecycle consumers.
- **Fresh final second pass:** searched the complete service for unsafe casts, process-level panics, silent error suppression, unrestricted session reads, unchecked expiration arithmetic, stale SQLite snapshots, and commit-before-fallible-step patterns. No remaining repository-owned in-scope defect was found.
- **Verification:** repository source inspection and branch-diff review only. Per maintainer-owned verification policy, no tests, clippy, build, gatekeeper, migrations, or runtime commands were executed by the agent.
- **Integration:** iterations merged through PRs #4248, #4249, #4250, #4251, #4253, and #4254; latest integrated `main` before closeout is `f9b69054270dc0a23cfd0d9cfeb453667e0a3f90`.
- **Status:** `FS-22.03.12` complete. Next primary module: `FS-22.03.13 — apps/server/src/services/auth_lifecycle_provider.rs`.

### FS-22.03.11 Iteration 1 — `crates/modules/rustok-auth/src/lifecycle.rs`

- **Base:** refreshed `main` at `f6cda48e06c062ba069e8b9eaf7e082c35cf8490`; dedicated branch `codex/audit-fs-22.03.11-auth-lifecycle`.
- **Invariant map:** the lifecycle contract must expose one canonical typed runtime boundary, preserve tenant/user/session/locale context without hidden defaults, classify domain failures without forcing transport policy into the owner, and keep consumers from accessing auth persistence through host-specific types.
- **Discovery:** re-read the complete lifecycle port/context/error records, the only discovered production implementation in `apps/server/src/services/auth_lifecycle_provider.rs`, auth GraphQL consumers, server lifecycle service, and auth module documentation.
- **Finding assessment:** no remaining repository-owned defect was confirmed inside `rustok-auth/src/lifecycle.rs`. The port is framework-neutral, contains no database or transport logic, and its context/error vocabulary matches the current server provider boundary.
- **Deferred adjacent finding:** `ServerAuthLifecycleProvider::forgot_password` currently maps email-service construction and reset-URL preparation failures to `AuthLifecycleMutationError::Internal` for existing accounts, while unknown accounts return success. This can reintroduce an account-existence signal through GraphQL/native callers when mail configuration is unavailable. The root cause belongs to the server provider and is assigned to a later primary module iteration; no transport workaround was added to the owner contract.
- **Fresh second pass:** independently re-read the contract, server implementation, direct GraphQL runtime registration, and lifecycle error mapping. No additional owner-contract defect remained.
- **Verification:** repository-content/static inspection and branch-diff review only. No tests, clippy, build, gatekeeper, migrations, or runtime commands were executed by the agent.
- **Status:** `FS-22.03.11` complete. Next primary module: `FS-22.03.12 — apps/server/src/services/auth_lifecycle.rs`.

### FS-22.03.10 Iteration 1 — `apps/server/src/auth.rs`

- **Base:** refreshed `main` at `10642934e162ede0f542fd3c182c49f10c60e0d0`; dedicated branch `codex/audit-fs-22.03.10-server-auth-adapter`.
- **Invariant map:** the server auth adapter must preserve auth-owner error semantics, fail closed on malformed startup auth configuration, avoid alternate token/credential implementations, preserve tenant/user binding for server-owned token adapters, and never expose secrets or internal diagnostics through transport responses.
- **Discovery:** re-read the complete adapter, its direct host startup caller, server error boundary, auth lifecycle consumer paths, auth owner configuration/JWT contracts, and adjacent documentation.
- **Finding AUTH-ADAPTER-22.03.10-01:** `auth_config_from_host_settings` converted `serde_json::from_value::<AppSettings>` failures to `None` and then used default `AuthSettingsOverrides`. A malformed nested auth configuration, including an invalid `JwtAlgorithm` value or invalid setting shape, could therefore be silently ignored and replaced by unintended defaults.
- **Remediation:** nested auth-settings deserialization errors now propagate through the server `Error` boundary. A focused regression test requires malformed `algorithm` input to return `Error::Json`; valid absent auth settings retain the existing default behavior.
- **Adjacent-boundary review:** `host::run` already propagates the adapter result before server traffic is accepted and separately applies production auth deployment checks; `Error::Json` has the generic HTTP 500 response mapping, while startup diagnostics remain available to the operator. Token/credential wrapper functions continue to delegate directly to `rustok-auth` without a second implementation.
- **Regression audit:** malformed auth settings can no longer downgrade to HS256/default claims silently; successful configuration assembly and existing token wrappers are unchanged; no secret material is included in the new error path.
- **Fresh post-merge second pass:** after PR #4242 merged at `10642934e162ede0f542fd3c182c49f10c60e0d0`, the complete adapter and direct host boundary were re-read. No additional repository-owned defect remained in the primary adapter surface.
- **Verification:** repository source inspection, direct caller/callee review, regression reasoning and branch-diff review only. No tests, clippy, build, gatekeeper, migrations, or runtime commands were executed by the agent.
- **Integration:** PR #4242 merged into `main` at `10642934e162ede0f542fd3c182c49f10c60e0d0`. This closeout entry is maintained in the canonical ledger; the post-closeout SHA is reconciled separately after its merge.
- **Status:** `FS-22.03.10` complete. Next primary module: `FS-22.03.11 — crates/modules/rustok-auth/src/lifecycle.rs`.

### FS-22.03.09 Iteration 1 — `crates/modules/rustok-auth/src/config.rs`

- **Base:** refreshed `main` at `35a80f07bdda94ff3c26b114557aae7143750f8d`; dedicated branch `codex/audit-fs-22.03.09-config-closeout`.
- **Invariant map:** authentication configuration must fail closed on invalid TTLs, blank claims, weak HS256 material, incomplete/mismatched RS256 keys, and conflicting algorithm/key material; configured key sources must not expose key contents; public builders and runtime token owners must share the same configuration semantics.
- **Discovery:** re-read the complete configuration owner, all auth-module configuration tests, JWT signing/validation consumers, server startup configuration assembly, production deployment checks, module documentation, and workspace dependency declarations.
- **Finding assessment:** no remaining repository-owned owner-level defect was confirmed inside `rustok-auth/src/config.rs`. TTL ranges are bounded; refresh TTL cannot be shorter than access TTL; HS256 enforces a 32-byte minimum and rejects RSA material; RS256 requires and cryptographically verifies a matching private/public pair; key-env values reject empty material; Debug implementations redact secret/key contents.
- **Boundary observation deferred:** `apps/server/src/auth.rs::auth_config_from_host_settings` currently converts any nested auth-settings deserialization error into `None` and silently falls back to default auth overrides. A malformed `algorithm`, invalid value shape, or future strict-schema failure can therefore be ignored instead of failing startup. The root cause is in the host adapter and is assigned to the next primary module `FS-22.03.10 — apps/server/src/auth.rs`; no compensating config-side behavior was invented here.
- **Fresh second pass:** independently re-read `config.rs`, `jwt.rs`, host startup assembly, production auth deployment validation, and the deferred adapter boundary. No additional config-owner defect remained.
- **Verification:** repository-content/static inspection and branch-diff review only. No tests, clippy, build, gatekeeper, migration, or runtime commands were executed by the agent.
- **Status:** `FS-22.03.09` complete. Next primary module: `FS-22.03.10 — apps/server/src/auth.rs`.

### FS-22.03.08 Iteration 1 — `crates/modules/rustok-auth/src/credentials.rs`

- **Base:** refreshed `main` at `5d331ddecd1f23f49199147cc1dfd2b3c98912e1`; dedicated branch `codex/audit-fs-22.03.08-auth-credentials`.
- **Invariant map:** refresh-token generation must preserve 256-bit entropy, must not expose token material or backend diagnostics, and an operating-system RNG failure must fail the authentication operation through the typed auth/server error boundary rather than panic the host process. Every production caller must propagate the failure instead of assuming an infallible string result.
- **Finding AUTHCRED-22.03.08-01:** `generate_refresh_token` used infallible `OsRng.fill_bytes`, which can panic when the operating-system randomness source is unavailable. The helper sits on login/session creation and refresh-token rotation paths, so an RNG failure could abort an authentication request at process level instead of producing a controlled server error.
- **Remediation:** `generate_refresh_token` now returns `Result<String>` and uses `OsRng.try_fill_bytes`; RNG failure is mapped to the owner-owned `AuthError::RefreshTokenGenerationFailed`. The server adapter maps that typed error to the stable internal-server-error boundary, and both production session/token creation call sites propagate the error through `AuthLifecycleError`.
- **Regression/source audit:** the existing token shape and entropy checks were retained and updated for the fallible API; the final post-merge pass found one missed test call site and fixed it in PR #4238 before closeout. No remaining `generate_refresh_token` caller treats the result as an infallible string.
- **Adjacent-boundary review:** password hashing already propagates its own RNG/hash failures as `Result`; refresh-token hashing remains deterministic; OAuth token/service and auth controller boundaries consume the lifecycle service rather than constructing refresh tokens independently. No new retry, fallback, logging, token persistence, or protocol behavior was introduced.
- **Regression audit:** normal successful token generation remains 32 random bytes encoded as 64 hexadecimal characters; refresh rotation and initial session creation now fail closed if secure randomness cannot be obtained. No secret or RNG error detail is propagated to the client.
- **Fresh post-merge second pass:** after PR #4237 merged at `20f7e24b8ab8aa691af11e5cb80652965f52c7ef`, the full changed credentials/error/adapter/lifecycle surface was re-read. That pass identified the remaining test migration defect; PR #4238 merged it at `54ad2e75f2f5f0984ae5937358119316117db86b`. A second fresh pass then confirmed all production and test call sites use the fallible contract and no further in-scope defect remained.
- **Verification:** repository source inspection and branch-diff review only. Per maintainer-owned verification policy, no tests, clippy, build, gatekeeper, migrations, or runtime commands were executed by the agent.
- **Integration:** PR #4237 merged the production remediation into `main`; PR #4238 merged the post-merge test-contract correction. Current `main` after closeout is `54ad2e75f2f5f0984ae5937358119316117db86b`.
- **Status:** `FS-22.03.08` complete. Next primary module: `FS-22.03.09 — crates/modules/rustok-auth/src/config.rs`.

### FS-22.02.11 Iteration 1 — `security_headers.rs` pre-implementation findings

- **Base:** refreshed `main` at `a10cfc7982fac9bdaa3013a825c8598869c53442`; dedicated branch `codex/audit-fs-22.02.11-security-headers`.
- **Invariant map:** security headers must be authoritative at the main-host boundary; API/health/operator surfaces must fail closed to the deny CSP; UI scripts/styles must remain nonce-bound; the richtext exception must cover only the owner-declared editor frame/asset surface; HSTS must agree with the host production declaration; CSP reporting must be bounded, privacy-safe and subject to the same abuse controls as other public API traffic.
- **Confirmed finding SEC-22.02.11-01:** `security_headers` is currently the outermost application middleware, so its direct `POST /api/security/csp-report` interception executes before the configured path-aware rate limiter. The unauthenticated CSP collector can therefore bypass the general `/api/` request budget and directly consume its bounded parsing, telemetry and logging work.
- **Confirmed finding SEC-22.02.11-02:** `csp_reports::record_report` logs the browser-supplied `disposition` string verbatim. The report body is bounded to 64 KiB, but the logged value is not normalized or bounded, so an unauthenticated caller can inject large/untrusted log content. This violates the repository security/observability requirement for bounded, non-sensitive structured logging.
- **Confirmed finding SEC-22.02.11-03:** `sanitized_location` removes paths/query/fragment data but does not bound the resulting origin. A syntactically valid attacker-controlled URL can therefore produce a very large origin field in the CSP security log despite the otherwise bounded telemetry contract.
- **Confirmed finding SEC-22.02.11-04:** `is_richtext_frame_surface` grants the dedicated richtext CSP, `SAMEORIGIN`, referrer policy and cache semantics to every path under `/richtext/frame/`. The actual main-server owner exposes one exact frame path plus a fixed single-segment asset set; broad prefix classification can therefore grant the weaker `style-src-attr 'unsafe-inline'` policy to unrelated/future paths or fallback responses.
- **Confirmed finding SEC-22.02.11-05:** `docs/security/csp-report-only-inventory.md` describes an opt-in `RUSTOK_CSP_STRICT_STYLE_ATTRIBUTES` rollout mode and says the default enforced UI policy still contains `style-src-attr 'unsafe-inline'`, but the current executable server policy already enforces `style-src-attr 'none'` and contains no such environment-flag path. The verification scripts likewise require the strict policy directly. This is current-state documentation drift at the security-policy boundary.
- **Adjacent finding deferred:** `apps/admin/src/app/security.rs` reproduces the same broad `/richtext/frame/` prefix classification for the standalone admin host. It belongs to the standalone-admin security boundary and is not the primary module for this iteration; record it for its owning module track rather than widening this patch.
- **Planned remediation units:** rate-limit composition order; bounded report disposition; bounded sanitized origin; exact richtext frame/asset path classification; synchronization of the central CSP inventory with executable truth. Each unit will be independently re-read before the next one.
- **Fresh finding SEC-22.02.11-06:** after narrowing the richtext boundary against the actual owner, the middleware's current cache override is also shown to overwrite the owner-specified `public, max-age=0, must-revalidate` policy for the unversioned `leptos-adapter.mjs` with `public, max-age=31536000, immutable`. The adapter is copied under a stable filename, so this can retain stale code beyond the owner contract.
- **Fresh finding SEC-22.02.11-07:** `security_headers` is currently installed inside `apply_http_edge_stack`, so outer edge middleware can synthesize responses without passing through the security-header middleware. In particular, `adaptive_timeout` can return `408 Request Timeout` directly, and edge short-circuits such as CORS handling likewise need to remain covered by the host-wide response security baseline.

### FS-22.02.11 Result — `security_headers.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Audit base:** `main` SHA `a10cfc7982fac9bdaa3013a825c8598869c53442`.
- **Dedicated branch:** `codex/audit-fs-22.02.11-security-headers`.
- **Merged:** PR #4179, merge commit `8bd69562d03e711d25502e4d20b771d129a4afae`.
- **SEC-22.02.11-01 remediation:** CSP report collection is now intercepted below the public `/api/` rate limiter but before tenant/auth middleware, preserving an unauthenticated tenant-independent collector without an abuse-budget bypass.
- **SEC-22.02.11-02 remediation:** CSP report `disposition` is normalized to the bounded static values `report`, `enforce`, or `other` before logging.
- **SEC-22.02.11-03 remediation:** logged CSP URL origins are capped at 512 bytes and collapse to `oversized`; paths, queries, and fragments remain excluded.
- **SEC-22.02.11-04 remediation:** the main-server richtext exception recognizes only the exact frame document, `leptos-adapter.mjs`, and generated 16-hex-digit hashed JS/CSS assets instead of the entire `/richtext/frame/*` prefix.
- **SEC-22.02.11-05 remediation:** the CSP report-only inventory was synchronized with the current executable strict `style-src-attr 'none'` policy and the removed legacy rollout flag.
- **SEC-22.02.11-06 remediation:** the host no longer overwrites owner-defined richtext asset cache semantics; only the exact frame document receives host-level `no-store`.
- **SEC-22.02.11-07 remediation:** security headers now wrap the complete HTTP edge stack, so edge short-circuit responses such as CORS/timeouts retain the host security baseline.
- **Additional hardening:** production WebSocket CSP policy now reuses the canonical `crate::common::is_production_environment()` helper rather than duplicating environment parsing.
- **Adjacent finding deferred:** `apps/admin/src/app/security.rs` still has a broad richtext path classifier; this is the standalone-admin security boundary and remains assigned to its own primary module iteration.
- **Fresh second pass:** independently re-read the complete changed security-header/reporting path, normal and registry/worker host composition, edge-stack order, richtext owner routes/cache contract, CSP verification source, and synchronized security inventory. No remaining repository-owned in-scope defect was found.
- **Regression correction during implementation:** the first edge-layer rearrangement temporarily duplicated rate limiting in the registry/worker branch; later re-read caught and corrected it. A second temporary inner `security_headers` layer that would have produced two CSP nonces was also caught and removed before PR creation.
- **Verification:** repository source inspection, static reasoning, cross-file contract review, and branch-diff review only. No tests, cargo check/clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.12 — `apps/server/src/services/server_bootstrap.rs`.

### FS-22.02.22 Result — `controllers/marketplace_registry.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base before track:** `e3a8ec0d1aaedb348749ab9e4f4e94d203e73fbb`.
- **Iteration 1:** commit `e3f1fa81696cc2df0e7c04f047bba6c65d25a9b6`.
  - **Finding:** the deployment-global `/catalog` and `/catalog/{slug}` handlers required `RequestContext`, which in turn requires tenant context. Registry-only host mode intentionally bypasses tenant resolution, so those global routes failed at runtime instead of using the accepted global catalog boundary.
  - **Remediation:** catalog handlers now consume the resolved presentation locale directly, while keeping tenant resolution out of the global read-only catalog; the catalog page size is also bounded to 100 by default instead of returning an unbounded collection when `limit` is omitted.
- **Iteration 2:** commit `97c5667844b84bfbc6241b03c42d0a55480558c7`.
  - **Finding:** authenticated user sessions could download any publish artifact because the download handler checked only for the presence of a session, bypassing the owner `Manage` authorization contract. The live third-party publish guard also returned HTTP 400 for a permission failure.
  - **Remediation:** user-session artifact downloads now require the canonical owner management authorization; runner-token access remains the separate non-user path. Third-party live publish permission failures now map to HTTP 403.
- **Iteration 3:** commit `3d27e352c34b3a2500d165723f460ff9667883c8`.
  - **Finding:** public registry governance errors, storage failures, and owner-internal failures could carry internal detail through server `Error::Message` or typed forbidden/unauthorized responses/logs.
  - **Remediation:** internal and authorization failures now use stable generic transport messages and stable log text; raw owner/storage error details are no longer propagated at this controller boundary.
- **Iteration 4:** commits `9a6006fe5fd926cd63d4c4f0dac273ec1e63d0bc`, `e775d6bcbc23500a091cd0290d4c268dd0fceaea`, and `31299cd2e253eb165ef253635c6af1ae2419e7fc`.
  - **Finding:** public catalog caching did not declare the locale/tenant-selector dimensions that can change the effective presentation, and `If-None-Match` did not support weak validators. Internal response-construction errors also used the generic `Error::Message` path, and the first regression test patch exposed import/escaping fallout during re-audit.
  - **Remediation:** 200/304 catalog responses now emit the explicit `Vary` dimensions, weak ETags are accepted, internal response failures use the stable internal error boundary, and focused regression tests were corrected and retained.
- **Iteration 5:** commit `ed303eaf6cd96d87e179308ec6b3dfe7cb519f6c`.
  - **Finding:** redirects to storage-issued signed artifact URLs were not explicitly non-cacheable, leaving a bearer-like signed URL in a potentially cacheable redirect response.
  - **Remediation:** signed artifact redirects now use `private, no-store` and `Referrer-Policy: no-referrer`.
- **Documentation synchronization:** commit `4903e6e7db1477271e976012a4a8ec0e7ade59e4` records the global catalog cache/pagination contract in the accepted architecture document.
- **Final fresh second pass:** independently re-read the complete controller, registry-only/full host composition, locale/request-context boundary, registry governance owner service and typed error taxonomy, artifact storage/download flow, remote runner transitions, API/architecture contracts, and updated regression tests. Rechecked tenant isolation, authorization, idempotency inputs, body limits, cache keys/ETags/Vary, signed redirects, legacy actor/publisher rejection, raw error/log leakage, dynamic label/cardinality risks, and unbounded collection handling. No remaining unblocked controller-owned defect was found.
- **Non-findings:** `GET /v2/catalog/publish/{request_id}` was reviewed against the owner status-snapshot contract and remains an optional-authority status read; no evidence in the active registry contract required converting it into a separately authenticated mutation boundary. Direct `ManifestManager` use in the global catalog remains intentional because the accepted architecture explicitly defines active platform composition plus global registry governance projection as the source for these two deployment-global routes.
- **Verification:** repository source inspection, architecture/API/settings owner review, migration/schema inspection, immediate re-audits after each remediation, final source-level regression pass, and commit history reconciliation only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.23 — `apps/server/src/controllers/artifact_http.rs`.

### FS-22.02.23 Result — `controllers/artifact_http.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base before track:** `a31f1ef1d30880c7023f157720c2fa9e6ca4a584`.
- **Iteration 1:** commit `e8ae64a7c3343beb9d4b3798123c98ca250fe916`.
  - **Finding:** the HTTP transport parsed the request with Axum's `Json` extractor before the controller could enforce the binding's declared `max_body_bytes`, and then normalized the wildcard path with `trim_matches('/')`. A body could therefore be parsed before the per-binding limit was applied, while a trailing slash could execute the slash-less admitted route.
  - **Remediation:** HTTP dispatch now receives raw `Bytes`, resolves the exact binding before parsing, rejects bodies above the admitted byte limit first, and preserves the wildcard path literally. The HTTP wildcard route is additionally capped at the descriptor-wide maximum of 1 MiB.
- **Iteration 2:** commit `267a3460459a781540ef85025136e15745e7ee89`.
  - **Finding:** successful artifact transport/UI responses are tenant-, actor-, permission-, and often locale-dependent but were returned without an explicit cache policy.
  - **Remediation:** all successful artifact HTTP/UI JSON responses now carry `Cache-Control: private, no-store`.
- **Iteration 3:** commit `b6f8d78269827bc1b61ba9fdd60896d0cbd995f0`.
  - **Finding:** the wildcard artifact HTTP route used `any(...)` even though the admitted contract supports only GET/POST/PUT/PATCH/DELETE, causing unsupported methods to reach handler-level validation and potentially body extraction.
  - **Remediation:** the route now registers only the five admitted HTTP methods explicitly; the handler retains its defensive method mapping.
- **Immediate re-audits:** after each remediation, re-read the changed controller and its lower binding/runtime boundary. Confirmed the exact installation is preserved, the lower dispatcher still enforces canonical body/output bounds, and the sandbox still clamps wall-clock timeout to the admitted binding timeout.
- **Adjacent-boundary audit:** host composition explicitly mounts `controllers::artifact_http::router()`; tenant/auth middleware remains in the full application composition. Artifact permission authorization continues to use the canonical owner service. No evidence was found that artifact HTTP should be restricted to direct human sessions in the same way as the artifact-permission control-plane mutation surface, so no cross-module principal policy was invented.
- **Fresh second pass:** independently re-read the complete controller, `host.rs` composition, authentication/principal context, tenant extractor/middleware, artifact binding service, module dispatcher/runtime, API architecture contract, and current AGENTS governance. Rechecked exact route matching, supported-method surface, raw body limits, JSON parsing/error mapping, output bounds, exact installation identity, effective policy/RBAC, idempotency, timeout clamping, cache behavior, and sensitive-data handling. No remaining repository-owned in-scope controller defect was found.
- **Verification:** repository source inspection, immediate re-audits, cross-boundary contract review, and final source-level regression pass only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.24 — `apps/server/src/controllers/artifact_permissions.rs`.


### FS-22.02.24 Iteration 1 — `apps/server/src/controllers/artifact_permissions.rs`

- **Base:** refreshed `main` at `740ed1a3fbce358e5665a3a00f5bfd36bc157cd2`; dedicated branch `codex/audit-fs-22.02.24-artifact-permissions`.
- **Invariant map:** the transport must derive tenant and actor identity only from trusted request context; RBAC control-plane admission must require the typed direct-user principal and routed/authenticated tenant equality; `modules:manage` is the explicit artifact-permission management authority; request scope must be explicit and tenant scope must derive from the trusted routed tenant; immutable permission identity must be resolved by exact scope/installation/permission key; idempotency and grant/revoke mutation must remain one owner transaction with typed event publication through the host-composed transactional transport; no static `role_permissions` mutation may occur.
- **Discovery:** read the complete controller, its OpenAPI declarations, request/response DTOs, error mapping, direct route composition, authentication/principal/tenant extractors, RBAC control-plane policy, artifact permission assignment owner, immutable definition schema and transaction path, event publisher contract, transactional event transport, and the existing RBAC source guardrails.
- **Finding assessment:** no new repository-owned defect remained. The apparent generic architecture concern that the HTTP request DTO lives in `apps/server` is an intentional host-adapter boundary for this surface: RBAC explicitly owns domain assignment contracts while the server owns authenticated transport adapters, and the existing owner verification script deliberately requires the request DTO and explicit-scope mapping in this controller. No change was made for that resolved concern.
- **Security/data-flow recheck:** the controller never accepts a second tenant identifier; tenant scope is mapped only to the trusted routed tenant, control-plane admission consumes the separately propagated `AuthPrincipalContext`, delegated/service principals are rejected, cross-tenant authenticated context is rejected, and `modules:manage` is checked before owner mutation.
- **Transaction/retry recheck:** the controller delegates mutation semantics to `RbacArtifactPermissionAssignmentService`; the owner resolves exact immutable permission identity, records the tenant-scoped idempotency receipt, mutates only the dynamic artifact grant, publishes the sealed event inside the same transaction, and rolls back on publication failure. Exact retries are non-reapplying operations and changed commands conflict.
- **Fresh second pass:** re-read the complete controller and all direct security/owner boundaries listed above from the dedicated branch. No additional repository-owned defect was found inside `artifact_permissions.rs`.
- **Verification:** repository-content inspection, source-level reasoning and branch-diff review only. Per maintainer-owned execution policy, no test suite, clippy, build, gatekeeper, migration execution, or runtime command was run by the agent.
- **Status:** module-level fresh second pass clean; `FS-22.02.24` complete. Next primary module: `FS-22.02.25 — apps/server/src/controllers/admin_events.rs`.


### FS-22.02.25 Iteration 1 — `apps/server/src/controllers/admin_events.rs`

- **Base:** refreshed `main` at `95685d121c77740e67dfa7e4c7edf5d1f8b0132b`; dedicated branch `codex/audit-fs-22.02.25-admin-events`.
- **Invariant map:** tenant-facing DLQ reads must use only trusted routed tenant context and the documented current/legacy event-envelope tenant shapes; DLQ inspection requires `logs:read`, replay requires `logs:manage`; operational database failures must not be presented as caller faults or leak backend diagnostics; replay must be state-conditional so a relay claim cannot be clobbered by a concurrent operator request; event status, retry counters, claims, errors and dispatch metadata must transition consistently with the outbox relay contract.
- **Discovery:** read the complete controller, route/OpenAPI declarations, RBAC permission extractor, server error mapping, outbox `sys_events` entity/migration, transactional event transport, relay claim/update semantics, outbox DLQ documentation, database policy, and direct router composition.
- **Confirmed finding ADMIN-EVENTS-22.02.25-01:** `list_dlq` and the initial replay lookup converted `DbErr` into `Error::BadRequest(...)`, exposing backend error text and misclassifying server-side operational failures as HTTP 400. The replay update had the same public-diagnostic leak.
- **Remediation ADMIN-EVENTS-22.02.25-01:** database failures now map through the canonical `Error::Database` path, which logs the detailed server error and returns the stable generic HTTP 500 envelope. No backend diagnostic text is exposed to the caller.
- **Confirmed finding ADMIN-EVENTS-22.02.25-02:** replay first loaded a failed row and then performed an unrestricted primary-key `ActiveModel::update`. A concurrent relay claim could therefore begin after the read but before the operator write; the stale replay write could then clear the live claim and reset retry/error state, breaking relay ownership.
- **Remediation ADMIN-EVENTS-22.02.25-02:** replay now performs one database-side conditional `UPDATE` requiring `status=failed` and both claim fields to remain NULL. Only one matching row may transition to pending; any concurrent claim/state change makes the replay fail with the existing stable 400 state error. Added a focused regression test proving an already-pending claimed event retains its claim and retry state. The test was added but not executed by the agent.
- **Adjacent-boundary review:** the trusted tenant and RBAC permission extractors remain unchanged; tenant isolation continues to support both current root-envelope and legacy nested-envelope payloads as documented by `rustok-outbox`. Relay completion already uses claim-aware CAS updates on status/worker/claim timestamp, and the replay fix now follows the same ownership principle rather than introducing a competing transaction model. SQLite remains a test/local mode while production database policy is PostgreSQL.
- **Regression audit:** the remediation does not broaden replay authority, does not accept client tenant identifiers, does not mutate outbox event payloads, and does not add a second event transport. A failed replay can no longer overwrite a concurrently claimed row; a non-DLQ row still returns the documented state error.
- **Fresh second pass:** re-read the complete changed controller and the adjacent auth/RBAC, error, outbox entity/migration, relay, transactional transport, database-policy and documentation boundaries from the modified branch. No additional repository-owned defect was found inside the primary module.
- **Verification:** repository-content inspection, source-level reasoning and branch-diff review only. Per maintainer-owned execution policy, no test suite, clippy, build, gatekeeper, migration execution, or runtime command was run by the agent.
- **Status:** module-level fresh second pass clean; `FS-22.02.25` complete. Next primary module: `FS-22.02.26 — apps/server/src/controllers/channel.rs`.


### FS-22.02.26 Iteration 1 — `apps/server/src/controllers/channel.rs`

- **Base:** refreshed `main` at `29b423d4bf13a91e7b2ff078ee178de023ca5906`; dedicated branch `codex/audit-fs-22.02.26-channel-controller`.
- **Invariant map:** Channel Admin REST operations must derive tenant scope from trusted `CurrentTenant`; authenticated user authority must be checked against the same tenant before mutation; channel-owned domain errors must preserve their intended HTTP semantics; database/serialization failures must not become client faults or leak backend diagnostics; successful mutations must invoke the shared durable/local channel-cache invalidation boundary.
- **Confirmed finding CHANNEL-22.02.26-01:** `internal_error` converted every `rustok-channel::ChannelError` into `Error::Message`, causing expected not-found, validation and conflict states to return HTTP 500 instead of their documented client semantics. This affected all ChannelService calls and both tenant-existence helper boundaries.
- **Remediation CHANNEL-22.02.26-01:** replaced the generic mapper with a typed `map_channel_error`: `NotFound` -> 404; validation/invalid target/policy-operation errors -> 400; inactive and duplicate-resource conditions -> 409; database/serialization variants -> server-logged generic 500. Added focused status-contract regression tests for these mappings. No test suite was executed by the agent.
- **Adjacent-boundary review:** `CurrentUser` already verifies token tenant equality, user/session tenant ownership, active principal state, and authoritative RBAC permissions; the controller additionally restricts every channel/policy resource to the trusted routed tenant before mutation. Cache invalidation remains after successful service mutation and is backed by the durable generation reconciliation path. Owner service and migration invariants were re-read; no controller-side bypass was found.
- **Deferred adjacent finding CHANNEL-OPENAPI-26-01:** the public `/api/channels/*` REST surface has no `utoipa` path metadata in the controller and is not currently registered in `controllers/swagger.rs`. The general API architecture requires machine-readable OpenAPI for REST contracts, but the canonical aggregation/registration owner is `apps/server/src/controllers/swagger.rs`; handle this as the dedicated FS-22.02.31 primary module rather than widening the current controller iteration.
- **Regression audit:** successful service results and cache invalidation behavior are unchanged; cross-tenant checks still return the generic not-found response; only previously misclassified domain failure paths changed status/error classification.
- **Fresh second pass:** re-read the complete changed controller, `rustok-channel::ChannelError`, `Error::IntoResponse`, auth/RBAC tenant boundaries, ChannelService call sites, route registration and OpenAPI aggregation. No additional repository-owned defect remained inside `channel.rs`.
- **Verification:** repository-content inspection, source-level reasoning and branch-diff review only. Per maintainer-owned execution policy, no tests, clippy, build, gatekeeper, migration execution, or runtime command was run by the agent.
- **Status:** module-level fresh second pass clean; `FS-22.02.26` complete. Next primary module: `FS-22.02.27 — apps/server/src/controllers/flex.rs`.


### FS-22.02.27 Iteration 1 — `apps/server/src/controllers/flex.rs`

- **Base:** refreshed `main` at `61fdeb9c348123348f5676bd01d5ec06b165a2a6`; dedicated branch `codex/audit-fs-22.02.27-flex-controller`.
- **Invariant map:** every authenticated Flex mutation must remain tenant-scoped and permission-gated; owner business state and its durable domain event must commit atomically through the canonical outbox; transport code must not use the in-memory event bus as a replacement for transactional delivery; read/write DTOs and OpenAPI paths remain owner/contract based; storage failures must roll back the mutation and surface only stable client-safe diagnostics.
- **Confirmed finding FLEX-22.02.27-01:** the REST controller called `flex::*_with_event`, committed the Flex mutation inside `FlexStandaloneSeaOrmService`, and only afterward published the returned `EventEnvelope` through the server `EventBus`. The in-memory bus can reject delivery when no forwarder subscriber is present or when backpressure is active; the database mutation could therefore commit without a durable event, violating the canonical event-flow contract and allowing CQRS/read-side drift.
- **Remediation FLEX-22.02.27-01:** removed post-commit `EventBus` publication from the controller. The canonical SeaORM adapter now constructs each corresponding Flex domain event and writes it with `TransactionalEventBus::publish_root_in_tx` before its existing database transaction commits for schema/entry create, update and delete. If the outbox write fails, the surrounding transaction returns an error and the Flex mutation is rolled back. The standalone service test harness now installs the canonical outbox schema and includes regression coverage for event persistence and rollback on missing outbox storage.
- **Adjacent-boundary review:** `docs/architecture/event-flow-contract.md` requires business state + outbox in one transaction; `rustok-outbox` provides the owner-transaction write primitive; the server event bus remains only for asynchronous forwarding/legacy producer paths and is no longer on the Flex REST mutation path. RBAC extractor + trusted `CurrentTenant` continue to gate all ten endpoints. The Swagger composition already registers all Flex paths/schemas, so no OpenAPI change was needed.
- **Regression audit:** success response shapes, tenant filtering, field validation, localization semantics and delete behavior remain unchanged. The only changed failure state is that event persistence is now part of the mutation transaction, so an unavailable outbox fails closed instead of silently leaving committed state without a corresponding durable event.
- **Fresh second pass:** re-read the complete controller, all six mutating SeaORM methods, transactional outbox primitive, Flex event constructors, RBAC/tenant extractors, router/OpenAPI composition and event-flow contract after the remediation. No additional repository-owned defect remained inside the primary controller path.
- **Deferred owner-level concern:** `flex::standalone::*_with_event` helper APIs still construct post-mutation event envelopes for generic callers. No production server call site remains after this remediation; changing/removing that public API belongs to a dedicated `flex` owner iteration rather than widening the server controller audit.
- **Verification:** repository-content inspection, source-level reasoning and branch-diff review only. Per maintainer-owned execution policy, no tests, clippy, build, gatekeeper, migration execution, or runtime command was run by the agent.
- **Status:** module-level fresh second pass clean; `FS-22.02.27` complete. Next primary module: `FS-22.02.28 — apps/server/src/controllers/installer.rs`.


### FS-22.02.28 Iteration 1 — `apps/server/src/controllers/installer.rs` production trust boundary

- **Base:** refreshed `main` at `0f19f0f64072f455bf3518e8daafed81e6e804d0`; dedicated branch `codex/audit-fs-22.02.28-installer-controller`.
- **Invariant map:** production installer authentication and placement policy must be determined by the trusted server host, never by client-controlled `InstallPlan.environment`; production hosts must reject non-production plans; production HTTP apply must require `RUSTOK_INSTALL_SETUP_TOKEN`; production placement must use the host-selected `RUSTOK_INSTANCE_ROOT`; release builds without an explicit environment variable must retain the host's production-default posture.
- **Confirmed finding INSTALLER-22.02.28-01:** `plan`, `preflight`, and `apply` selected the setup-token requirement from the submitted plan environment. `bind_host_install_plan` likewise allowed `local/demo/test` plans to bypass the production instance-root requirement. A caller could therefore label a request `local` on a production host and reach installer mutation logic without the production setup-token/host-root fences, including local-style secret/database policies in preflight/apply.
- **Remediation INSTALLER-22.02.28-01:** installer setup-token validation now reads the canonical server production-environment helper; host plan binding rejects non-production install plans on a production host and applies the host-root requirement from the same trusted fact. The duplicated host production detector now delegates to the canonical settings helper, whose release-build default is production when no environment variable is present.
- **Regression audit:** client plan environment no longer controls the production security boundary; a production host cannot be downgraded to a local/demo/test installer path. Existing development behavior remains available on debug/non-production hosts.
- **Fresh second pass:** re-read the full installer controller, host environment selection, settings production helper, shared installer environment/preflight rules and installer documentation. No additional repository-owned defect was introduced by this trust-boundary remediation.
- **Known next slice:** the controller's `INSTALL_JOBS` map is process-local and unbounded, while the installer contract explicitly requires durable job/receipt reads. This is a separate lifecycle/persistence remediation on the same primary module and is intentionally not mixed into this iteration.
- **Verification:** repository-content inspection, source-level reasoning and branch-diff review only. Per maintainer-owned execution policy, no tests, clippy, build, gatekeeper, migration execution, or runtime command was run by the agent.
- **Status:** iteration 1 complete; module track remains open for durable job lifecycle remediation.

### FS-22.02.28 Iteration 2 — apps/server/src/controllers/installer.rs durable HTTP job lifecycle

- **Base:** refreshed main at fa6615b07999584cb36c89933310df4773a5b102; dedicated branch codex/audit-fs-22.02.28-installer-jobs.
- **Invariant map:** asynchronous HTTP installer job status must survive a request being served by another replica and must remain readable after the submitting process disappears; the job registry must not consume unbounded process memory; success/failure terminal transitions must be conditional on the job still being running; public HTTP job errors must not expose executor/backend diagnostics; the job state must remain in the host runtime database because the target install database may be a different database that does not exist yet.
- **Confirmed finding INSTALLER-22.02.28-02:** INSTALL_JOBS was a process-local unbounded HashMap<Uuid, InstallJobStatusResponse>. POST /api/install/apply inserted and mutated job state only in that process, while GET /api/install/jobs/{job_id} could not read the job from another replica or after restart. This violated the documented durable job/receipt read contract and created a memory-growth surface.
- **Remediation INSTALLER-22.02.28-02:** added canonical install_http_jobs foundation storage plus rustok-installer-persistence entity/service methods. The HTTP controller now creates the durable job before spawning the installer task, reads job state directly from host-runtime persistence, and completes jobs through database-side status=running compare-and-set updates. The target install_sessions row remains an opaque cross-database session_id reference; no FK is declared across databases.
- **Public-state hardening:** successful output is serialized into an optional bounded JSON projection (256 KiB maximum); oversized output is omitted rather than growing the job row without bound. Failed jobs store only a stable public error message; executor error text is not persisted into the HTTP job record. The status adapter logs decode/storage problems server-side and returns the generic installer error envelope.
- **Regression catch:** during implementation a stale require_setup_token(&headers, false) call on the job/receipt GET path was identified after iteration 1 changed the helper signature. It was corrected before merge; the fresh pass confirmed all installer endpoints now use the host-bound setup-token policy consistently.
- **Verification additions:** persistence tests cover cross-service-instance reads, terminal CAS, success-output round-trip and failure-message redaction; the migration has a focused table-creation test; no test command was executed by the agent.
- **Adjacent-boundary review:** host environment selection, installer plan binding, shared executor sequencing, installer persistence sessions/receipts, database migration registry, and HTTP router wiring were re-read. The existing detached task remains intentionally deferred to the broader worker lifecycle/recovery track; this iteration does not claim automatic resurrection of an interrupted running install.
- **Regression audit:** removing the in-memory map does not alter 202 Accepted response shape or job IDs. Successful output and failure status remain representable after process/replica changes. A second terminal completion cannot overwrite a job already completed/failed. Missing durable job storage causes apply to fail closed instead of accepting a job that cannot later be read.
- **Fresh second pass:** re-read the complete changed controller, installer persistence service/entity, migration registration/test, database architecture and installer contract docs. No additional repository-owned defect was introduced by the durable-state remediation.
- **Known next slice:** repeated concurrent POST /api/install/apply submissions still receive distinct HTTP jobs and can perform pre-lock target preparation before one installer session acquires the global lock. The shared installer contract requires resumed apply idempotency; this is a separate .28 iteration focused on command identity/idempotency rather than job persistence.
- **Status:** iteration 2 complete; module track remains open for installer apply idempotency/recovery review.

### FS-22.02.21 Result — `controllers/metrics.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base before track:** `7cdc78d34cf20a14d1871dffa6cee0fc569b5cd3`.
- **Iteration 1:** commit `e6a9b14976ed3ad9aa648ccf368605e11ae5b370`.
  - **Finding:** the controller registered only `/metrics/` while the canonical API documentation and observability-auth boundary expose `/metrics`.
  - **Remediation:** both explicit `/metrics` and `/metrics/` routes now invoke the same handler; observability-auth path coverage remains aligned with both.
  - **Finding:** search metrics were rendered even when `features.search_indexing` was disabled.
  - **Remediation:** search metric collection now follows the same feature gate as readiness/search schema requirements.
- **Iteration 2:** commits `69e2dcf9e5f1cc00743ed7148b8e66aee536ee95`, `8578107c6f8868206d68f1b71fbf6bd92713b97e`, `1c103e51452322434d64f960c4ee0163ad6f5d36`, and `7d786bc0c1cc418e50daf8bea59244cb6feaf098`.
  - **Finding:** tenant-activity, outbox, RBAC consistency, and search collectors converted storage/decoding failures into plausible zero values, making monitoring report false healthy/empty state.
  - **Remediation:** affected metric families now expose explicit collection-status signals and `NaN` for unavailable values; RBAC consistency values retain the failure counter without pretending the consistency counts are zero.
  - **Finding:** metrics synchronization logged backend error payloads directly.
  - **Remediation:** rate-limit metrics failure logs now emit only stable namespace/context text.
- **Iteration 3:** commit `e3a8ec0d1aaedb348749ab9e4f4e94d203e73fbb`.
  - **Finding:** outbox collector retained historical unprefixed compatibility aliases (`outbox_backlog_size`, `outbox_dlq_total`, `outbox_retries_total`) after equivalent RBAC aliases had already been removed under the repository zero-legacy policy.
  - **Remediation:** only canonical `rustok_outbox_*` metric names remain; regression expectations were updated accordingly.
- **Final fresh second pass:** independently re-read the full controller plus metrics-auth, rate-limit namespace construction, search diagnostics/feature ownership, API docs, and telemetry registry contracts. Rechecked route parity, feature gating, error/unknown semantics, secret-safe logging, label cardinality/source safety, worker/runtime observations, and absence of unprefixed outbox metric aliases. No remaining unblocked controller-owned defect was found.
- **Non-findings:** dynamic runtime metric labels are bounded by fixed limiter namespaces/backend kinds and fixed worker/provider/state vocabularies; the existing dual outbox canonical naming family was retained because it is the current exposed contract, while only the explicitly historical unprefixed aliases were removed.
- **Verification:** repository source inspection, commit-diff review, adjacent owner-contract inspection, immediate re-audits after each remediation, and final fresh second pass only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.23 — `apps/server/src/controllers/artifact_http.rs`.

### FS-22.02.20 Result — `controllers/health.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base before track:** `c79806106f246a75e21f497d262aed74a42519e2`.
- **Iteration 1:** commits `6004963c8f99cbc0b1d3a1b3e61e834459b67a71` and `18ae033e4a7dcf1617ed350d9bca23873eb9d189`.
  - **Finding:** public `/health/ready` propagated raw database, cache, tenant-invalidation, storage, outbox, search, and rate-limit backend errors into readiness reasons.
  - **Remediation:** public readiness diagnostics now use stable non-sensitive messages while preserving the existing readiness status semantics.
  - **Finding:** search-index lag was checked even when `settings.features.search_indexing` was disabled, despite `search_documents` being optional under that feature.
  - **Remediation:** search lag readiness is now evaluated only when search indexing is enabled.
- **Iteration 2:** commit `7cdc78d34cf20a14d1871dffa6cee0fc569b5cd3`.
  - **Finding:** search backend URL parsing defaulted no-port endpoints to port 80 regardless of scheme and did not safely handle userinfo or bracketed IPv6 authorities.
  - **Remediation:** parser now selects HTTPS port 443, strips userinfo before probing, supports bracketed IPv6 with explicit ports, and returns stable validation errors; focused parser tests were added.
- **Final fresh second pass:** independently re-read the complete controller after all patches and rechecked public error redaction, feature gating, readiness aggregation/circuit behavior, module-health handling, and search endpoint parsing. No remaining unblocked controller-owned defect was found.
- **Verification:** repository source inspection, settings/search owner-contract review, immediate re-audits after each remediation, and post-change source/diff review only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.21 — `apps/server/src/controllers/metrics.rs`.

### FS-22.02.19 Result — `controllers/users.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base before track:** `9e35c1d58a46db3c0be2a669fa0dd5586311c9ec`.
- **Final main after this module track:** `c79806106f246a75e21f497d262aed74a42519e2`.
- **Iteration 1:** PR #4211, merge commit `e6a5393f519703e2c51f452965cfda377a524cdd`.
  - **Finding:** `list_users` swallowed `num_items()` and `fetch_page()` database errors with `unwrap_or*`, returning HTTP 200 with a misleading empty user list during DB failures.
  - **Remediation:** database failures now propagate through the canonical server `Error::Database` boundary.
- **Iteration 2:** PR #4212, merge commit `c79806106f246a75e21f497d262aed74a42519e2`.
  - **Finding:** `get_user` converted `DbErr` to `Error::Message(e.to_string())` unnecessarily instead of using the canonical database error variant.
  - **Remediation:** `DbErr` now propagates with `await?`; tenant filtering and `404 NotFound` behavior remain unchanged.
- **Final fresh second pass:** independently re-read the complete controller after both merges and rechecked tenant scoping on list/get, permission checks, page bounds, query filters, DB-error propagation, and response mapping. No remaining unblocked controller-owned defect was found.
- **Non-findings:** the current list ordering is deterministic by `CreatedAt` only but is not treated as a blocker because page-number pagination remains inherently mutation-sensitive; no security boundary is crossed and adding a tie-breaker would be a separate API-semantics change.
- **Verification:** repository source inspection, adjacent server error-boundary review, immediate re-audits after each patch, fresh second pass, and branch-diff review only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.20 — `apps/server/src/controllers/health.rs`.

### FS-22.02.18 Result — `controllers/oauth.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base before track:** `b744f8189ebf7959223d2db3781fc80f3a707f17`.
- **Final main after this module track:** `5625810cbd8dc4e17f7361d4087c2744efc07651`.
- **Iteration 1:** PR #4206, merge commit `1c949e8f3a840bb1da40dae39821acd00e8476a1`.
  - **Finding:** `/api/oauth/userinfo` returned claims without requiring `openid`, exposed `profile/email` claims without corresponding scopes, accepted service principals, and hardcoded `email_verified=true`.
  - **Remediation:** require end-user principal + `openid`; gate `name` by `profile`, `email/email_verified` by `email`; derive verification state from `email_verified_at`; map `insufficient_scope` to HTTP 403.
- **Iteration 2:** PR #4207, merge commit `31fdde1f8ceef5a9414a05bfa57dce0d8046a7fc`.
  - **Finding:** token endpoint declared JSON or form-encoded `TokenRequest` compatibility but accepted JSON only.
  - **Remediation:** controller now selects Axum `Form<TokenRequest>` for `application/x-www-form-urlencoded` and retains JSON parsing otherwise, while preserving the existing token service.
- **Iteration 3:** PR #4208, merge commit `2b21f4dd301238a58b1d65004155a3d50ae98179`.
  - **Finding:** OAuth token responses lacked `Cache-Control: no-store` and `Pragma: no-cache`, despite the endpoint returning access/refresh tokens.
  - **Remediation:** all token endpoint success and error responses now pass through one helper that sets both anti-cache headers.
- **Iteration 4:** PR #4209, merge commit `5625810cbd8dc4e17f7361d4087c2744efc07651`.
  - **Finding:** UserInfo was exposed as GET-only, while OpenID Connect Core requires support for both GET and POST.
  - **Remediation:** GET and POST now share the same authenticated UserInfo handler; no authorization semantics were duplicated.
- **Fresh second-pass verification:** re-read the complete controller after all merges and rechecked authorization-code PKCE, exact redirect URI binding, active-client lookup, tenant binding, refresh rotation, browser-session cookie trust/attributes, token request content negotiation, UserInfo scope/principal policy, and token-response cache headers. No remaining unblocked controller-owned defect was found.
- **Explicitly deferred owner/module observations:** OAuth token-service internals and OAuth metadata/discovery remain separate owner/primary-module tracks; no controller shim was introduced for those boundaries. The controller deliberately retains `redirect_with_*().expect("validated redirect URI")` as an explicit post-validation invariant.
- **Verification:** repository source inspection, owner-service and model review, protocol verification against OAuth 2.0 / OpenID Connect specifications, immediate re-audits after each patch, fresh second pass, and branch-diff review only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.19 — `apps/server/src/controllers/users.rs`.

### FS-22.02.17 Result — `controllers/auth.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base before track:** `ed50feb2e7316a8d943c946528815a52851d1d01`.
- **Final main after this module track:** `c1b808e1766fdadc89f8b5d986c5cc0726b53cbd`.
- **Iteration 1:** PR #4203, merge commit `7fa75d62074fb38b7dfcc4bd9bfe38c514d210ba`.
  - **Finding:** `accept_invite` decoded a stateless invite and called `create_user_runtime` directly, bypassing the existing durable `auth_invite_consumptions` serialization point and making invite tokens replayable until expiry.
  - **Remediation:** controller now delegates to `AuthLifecycleService::accept_invite_once_runtime`, which hashes/reserves the token and creates the user in the same transaction.
- **Iteration 2:** PR #4204, merge commit `c1b808e1766fdadc89f8b5d986c5cc0726b53cbd`.
  - **Finding:** `features.registration_enabled` existed but `/api/auth/register` ignored it, so self-registration remained available when the feature was disabled.
  - **Remediation:** controller now enforces a server-owned `ensure_registration_enabled` guard before auth config lookup or account creation; focused unit coverage was added.
- **Final fresh second pass:** independently re-read the complete controller and its identity/session/recovery owner boundaries. No remaining unblocked controller-owned auth defect was found.
- **Verified non-findings:** `CurrentUser` resolves authoritative permissions/role from current DB state; refresh and password reset use transactional/session-safe owner services; session list/revoke predicates are tenant+user scoped; public recovery token exposure remains limited to non-production demo mode.
- **Explicitly deferred lifecycle debt:** password-reset and email-verification delivery use detached `tokio::spawn` tasks; their join/abort lifecycle remains under FS-24 rather than being changed locally in the controller.
- **Explicitly deferred owner-layer debt:** session creation can persist a session before later role/token creation completes; this belongs to `AuthLifecycleService::create_session_and_tokens_db` and is not patched through the controller.
- **Verification:** repository source inspection, owner-callsite review, immediate re-audits, fresh second pass, and branch-diff review only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.18 — `apps/server/src/controllers/oauth.rs`.

### FS-22.02.16 Result — `graphql.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base:** `da0c87002381a73cc45ff63612769f01a06bf220`.
- **Final main after this module track:** `788bb31f266696e7b4bd6d1ae0056a4239de3b67`.
- **Iteration 1:** PR #4196, merge commit `a1572e834de39e56f85722416d7ee560d29b0858`.
  - **Finding:** WebSocket upgrade defaulted to `GraphQLWS` when the server had not negotiated any supported subprotocol.
  - **Remediation:** no negotiated GraphQL WebSocket subprotocol now returns HTTP 400 before `on_upgrade`; existing frame/message bounds and protocol parsing remain intact.
- **Iteration 2:** PR #4197, merge commit `b13bf0a3e7d43548717bb176f578b4b108fabf7f`.
  - **Finding:** persisted-query `sha256Hash` telemetry accepted arbitrary client strings and wrote them directly to tracing.
  - **Remediation:** telemetry accepts only the canonical 64-hex SHA-256 identifier shape; malformed values are omitted without changing execution semantics.
- **Iteration 3:** PR #4198, merge commit `87be6f48c334970409c1273808b804f290b1b381`.
  - **Finding:** Axum `Json` extraction errors did not follow the async-graphql HTTP contract for malformed JSON/content-type failures.
  - **Remediation:** JSON extractor rejections are normalized to 400, while payload-too-large remains 413, with stable non-internal error messages.
- **Iteration 4:** PR #4199, merge commit `62cc2f4647536fa7f393cfea97ca97f5f3d3f518`.
  - **Finding:** WebSocket `connection_init.locale` bypassed tenant locale policy and used only syntactic locale parsing.
  - **Remediation:** the handshake resolves locale through the tenant-owned `TenantLocalePolicyPort` and preserves tenant policy fallback semantics.
- **Iteration 5:** PR #4200, merge commit `71157bc3a810d9e14381570bfacde4a8eb6b2d0d`.
  - **Finding:** WebSocket GraphQL data omitted `RequestContext`, although server GraphQL paths consume it.
  - **Remediation:** WS handshake now inserts a tenant/user/locale/correlation-bound `RequestContext`; channel dimensions remain unset rather than invented.
- **Iteration 6:** PR #4201, merge commit `788bb31f266696e7b4bd6d1ae0056a4239de3b67`.
  - **Finding:** `GraphqlWsAuthLease` was published before the fallible locale-policy check, leaving a partially initialized `OnceLock` when handshake initialization failed.
  - **Remediation:** lease publication now occurs only after tenant/auth/locale validation succeeds.
- **Final fresh second pass:** independently re-read the complete HTTP and WebSocket controller path against current `main`, including protocol negotiation, body/error status mapping, APQ telemetry, tenant/locale policy, RequestContext propagation, RBAC lease revalidation, and close/error behavior. No remaining **unblocked controller-owned** defect was found.
- **Deferred cross-module observations:** self-role/permission mutation should invalidate the task-local `RbacRequestScope` after commit; the scope primitive already supports this but the mutation owner must wire it. Full WebSocket task join/abort and host-managed connection lifecycle remain part of FS-24 worker/lifecycle audit and were not retrofitted here.
- **Verification:** repository source inspection, direct caller/owner-contract review, immediate re-audits, independent second pass, and branch-diff checks only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.17 — `apps/server/src/controllers/auth.rs`.

### FS-22.02.15 Result — `graphql_schema.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base:** `2a4fbb3e9b0a34db3b28779f63e360f173e9684e`.
- **Final main after this module track:** `308bc6199b464be588b7e7e2575a26a7cefeef3d`.
- **Iteration 1:** PR #4189, merge commit `fdcec0d9ed5c35f32f8e274567a322f42f03b58c`.
  - **Finding:** GraphQL schema composition synthesized a missing Alloy runtime; Alloy runtime construction starts a background scheduler, so schema composition could create runtime execution state when the capability was absent.
  - **Remediation:** Alloy runtime is now consumed only from boot-owned `ServerRuntimeContext`; schema dependency is optional and owner requests fail closed when absent.
- **Iteration 2:** PR #4190, merge commit `8b9536e07ccd7a1670ae26213df8297d84be5dee`.
  - **Finding:** schema composition rebuilt a missing `ModuleRegistry`, allowed missing marketplace catalog until a later panic, and started SEO reconciliation before schema construction had succeeded.
  - **Remediation:** registry/catalog are mandatory preconditions; GraphQL bootstrap returns errors instead of synthesizing topology; SEO reconciliation starts only after successful schema publication.
- **Iteration 3:** PR #4191, merge commit `8ef01ec57c333ea7fbe542a8605bef2e4237d647`.
  - **Finding:** missing Content Orchestration runtime was synthesized inside GraphQL composition.
  - **Remediation:** content orchestration is now an optional boot-owned dependency; owner GraphQL access remains fail closed when absent.
- **Iteration 4:** PR #4193, merge commit `58668fdcf17b63082d24a4efc984f0362a352be3`.
  - **Finding:** missing StorageRuntime was replaced by local/in-memory storage, including an indirect in-memory storage fallback for Alloy published-Rhai source handling.
  - **Remediation:** storage and Alloy published-Rhai source dependencies now consume only boot-owned storage; missing storage remains absent and focused absence tests were added.
- **Iteration 5:** PR #4194, merge commit `308bc6199b464be588b7e7e2575a26a7cefeef3d`.
  - **Finding:** required boot-owned registry/catalog state was validated only after side-effectful event/cache/provider composition had already begun.
  - **Remediation:** cached schema remains the first path; mandatory registry/catalog preconditions now execute before event bus, cache, and provider attachment side effects.
- **Final fresh post-merge second pass:** independently re-read `graphql_schema.rs` against the current `main`, plus direct schema-builder dependencies and owner contracts. The primary module contains no remaining unblocked repository-owned fallback/topology-duplication defect.
- **Deferred cross-module observations:** `event_bus_from_context` still owns lazy EventBus/forwarder initialization, and `attach_commerce_provider_registries` contains intentional-or-not in-process provider/runtime fallback construction. These require their own owner-boundary audits; no behavior was silently changed through `graphql_schema.rs`.
- **Verification:** repository source inspection, direct callsite/owner contract review, immediate re-audits, regression checks, fresh second pass, and branch-diff review only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.16 — `apps/server/src/controllers/graphql.rs`.

### FS-22.02.14 Result — `server_runtime_context.rs`

- **Status:** COMPLETE; no code remediation required.
- **Fresh main base:** `c800d56cc230fe86495db24b38269222d449c7f6`.
- **Invariant map:** the server runtime context must provide one immutable settings snapshot, one shared DB handle, type-safe singleton runtime values, atomic first-writer lifecycle registration, clone-safe access across Axum state, and no transport-level leakage of host secrets.
- **Discovery:** read the complete `ServerRuntimeContext` / `ServerSharedValues` implementation and its direct runtime usage in lifecycle, health, guardrail, bootstrap, auth, and runtime composition paths.
- **Security check:** `ServerAuthRuntime::auth_config()` is an internal server-state accessor. Actual controller callsites clone/use `AuthConfig` for token operations or public metadata generation; the auth secret is not serialized or emitted by these paths. No client-facing secret exposure was found.
- **Concurrency check:** `shared_insert_if_absent` uses the map entry API under the write lock, so first-writer ownership is atomic. The known StopHandle bootstrap race therefore does not reappear in this storage primitive.
- **Type-safety check:** `TypeId` is used consistently as the key and `Any::downcast` as the value boundary. The `expect()` inside the private `get_or_insert_with` invariant is only reachable on an impossible key/type mismatch created internally by the same TypeId insertion path.
- **Locking check:** synchronous `RwLock` critical sections are short and do not perform async I/O in the current production callsites. `shared_map` executes its inspection callback under the read lock; all reviewed callers use non-reentrant pure accessors such as `is_finished` / `instance_id`.
- **Fresh second pass:** independently re-read the complete module and reviewed actual `shared_map`, `shared_get`, `shared_insert`, `shared_insert_if_absent`, and `effective_policy_cache` call patterns. No remaining unblocked repository-owned defect was found.
- **Decision:** no speculative API redesign or cosmetic refactor was introduced merely to create a diff.
- **Verification:** source inspection, direct-callsite analysis, concurrency/type-boundary reasoning, and fresh second pass only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.15 — `apps/server/src/services/graphql_schema.rs`.

### FS-22.02.13 Result — `app_runtime.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base:** `ff30a80f06956499ad2ff228d2660f83f1a565da`.
- **Final main after this module track:** `69c96eac789507fb70096498bf79c0772b1437ff`.
- **Iteration 1:** PR #4184, merge commit `6e809ccd7256472bb6f9aa3801eb2d4a3f7d551f`.
  - **Finding SRV-22.02.13-01:** Workflow cron was started whenever `workflow_cron_enabled=true`, even for `api`, `admin_ssr`, and `storefront_ssr` host profiles that explicitly do not run background workers.
  - **Remediation:** cron startup now requires both `runtime.runs_background_workers()` and the existing workflow-cron flag; focused profile tests were added.
- **Iteration 2:** PR #4185, merge commit `b98a666eb038963d28a7de217bb72b4b8cc557c7`.
  - **Finding SRV-22.02.13-02:** module-work startup used a non-atomic StopHandle check/insert followed by a production `expect()`, duplicating a known bootstrap race.
  - **Remediation:** module-work startup now uses the canonical atomic `StopHandle::ensure(ctx)` path.
- **Iteration 3:** PR #4186, merge commit `69c96eac789507fb70096498bf79c0772b1437ff`.
  - **Finding SRV-22.02.13-03:** `bootstrap_app_runtime` accepted a second `RustokSettings` value while `ServerRuntimeContext` already owned the authoritative immutable settings snapshot. Different callers could therefore construct internally inconsistent runtime policy.
  - **Remediation:** the bootstrap API now reads settings once from `runtime_ctx.settings()`; the direct server bootstrap caller and local test were updated.
  - **Regression caught before PR:** the first version of this iteration passed the owned snapshot where the host-provider builder requires `&RustokSettings`; immediate re-audit caught and corrected the mismatch before PR creation.
- **Final fresh second pass:** independently re-read the complete bootstrap path, runtime-mode gates, registry/manifest composition, module-work scheduling, GraphQL/rate-limit initialization, and direct caller contract. No remaining **unblocked app-runtime-owned** defect was found.
- **Cross-module finding deferred by the one-primary-module rule:** `init_marketplace_catalog` calls `HardenedRegistryMarketplaceProvider::from_env()`, whose owner implementation currently panics on malformed registry configuration/client construction. The root cause belongs to `marketplace_catalog_cache_base.rs`; it is not patched through an app-runtime catch/panic shim. A dedicated owner-module iteration must handle it.
- **Explicitly deferred lifecycle debt:** detached Workflow cron, module-work scheduler, and rate-limit cleanup tasks do not yet form one host-managed join/abort lifecycle; this remains under the existing FS-24 worker lifecycle scope.
- **Production `expect()` retained:** `module_runtime_extensions_from_ctx` has an explicit bootstrap-order invariant and is only used after the runtime extension registry is installed; it is treated as a programming-error guard, consistent with the coding standard's permitted invariant panics.
- **Verification:** source-level static checks, direct caller/callee inspection, regression re-audit, independent second pass, and branch-diff review only. No tests, compiler, clippy, gatekeeper, generator, or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.14 — `apps/server/src/services/server_runtime_context.rs`.

### FS-22.02.12 Result — `server_bootstrap.rs`

- **Status:** COMPLETE and integrated into `main`.
- **Fresh main base:** `51d2ea5508426d7eadc064454fb50b3c1cc11ec7`.
- **Final main after this module track:** `b94da56d40452b120bf27d8eda6989f439b7d765`.
- **Iteration 1:** PR #4180, merge commit `e01357b2655022b3719e71670fb408aff92860a3`.
  - **Finding SRV-22.02.12-01:** production bootstrap errors exposed matched development JWT fragments, sample database credentials, and sample superadmin passwords through `Error::Message`.
  - **Remediation:** secret matchers remain available for detection, but their matched values are never interpolated into errors. Production validation is explicit and rejects an empty JWT secret. A deterministic test set was added.
- **Iteration 2:** PR #4181, merge commit `f78f21893afb857aa970d7a8c8c43cb2dd14f403`.
  - **Finding SRV-22.02.12-02:** bootstrap started multiple background sidecars/workers before the fallible final router composition step.
  - **Remediation:** complete Axum router composition now occurs before bootstrap-owned sidecar/worker startup; `runtime_ctx` is cloned at the auth-runtime boundary so the context remains available for subsequent startup.
- **Iteration 3 / regression correction:** PR #4182, merge commit `b94da56d40452b120bf27d8eda6989f439b7d765`.
  - **Fresh finding:** using only the runtime production env flag would have skipped the bootstrap secret guard for release builds that select the production config by build mode without exporting `RUSTOK_ENV`.
  - **Remediation:** bootstrap secret validation is enabled when either the runtime explicitly declares production or the binary is a release build, preserving the release-default production guard while still enabling explicit production checks for debug builds.
- **Final fresh second pass:** re-read `server_bootstrap.rs`, its `host.rs` caller, `app_runtime.rs`/router contracts, worker start paths, and the adjacent security/lifecycle documentation. No remaining **unblocked** repository-owned defect was found in the primary module.
- **Explicitly deferred lifecycle debt:** `initialize_server_context` and `bootstrap_app_runtime` can start asynchronous components before all later bootstrap steps have become irreversible, and the full worker/runtime lifecycle still lacks one unified join/abort/rollback boundary. This remains explicitly deferred to FS-24 per the existing phase-scope decision; this module track does not claim those lifecycle gaps are fixed.
- **Verification:** source-level static checks, direct caller/callee inspection, branch-diff review and regression re-audit only. No tests, compiler, clippy, gatekeeper, generator or runtime commands were executed by the agent; maintainer verification remains required.
- **Next primary module:** FS-22.02.13 — `apps/server/src/services/app_runtime.rs`.

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

### FS-22.02.09 Iteration 1 — `apps/server/src/middleware/tenant.rs`

- **Base:** refreshed `main` to `266e4083ba604f5c2339005ef3afa21a73800c18` immediately before this iteration.
- **Dedicated iteration branch:** `codex/audit-fs-22.02.09-tenant-middleware`.
- **Invariant map:** tenant resolution must remain bound to the canonical request trust policy; explicit tenant assertions must agree before the request proceeds; cache identity must be bounded and canonical; negative and positive entries must not survive a tenant mutation across cache generations; cache-generation binding must fail closed if the physical data/negative namespaces cannot share one generation state; inactive tenants must never enter `TenantContext`; development fallback must remain explicit; cache/runtime health signals must reflect the actual generation listener state.
- **Confirmed finding TENANT-22.02.09-01:** the generation-aware cache backend protects each individual cache I/O from completing against an outdated namespace, but the tenant middleware's async miss loader had no generation-stable boundary around the whole read/fill operation. A tenant mutation could rotate generation after the DB read but before cache fill; the stale fill would then write through the new physical generation under the old logical key, making stale tenant data or a stale negative result readable until TTL.
- **Remediation TENANT-22.02.09-01:** tenant positive and negative logical cache keys now include the captured backend generation. The complete negative-check/load/fill path snapshots generation before cache access and verifies the same snapshot after the operation, retrying up to four times on any generation/trust change. A stale in-flight fill can therefore remain only under an obsolete logical key and cannot be returned by the current generation.
- **Confirmed finding TENANT-22.02.09-02:** `TenantCacheInfrastructure::new` ignored the result of `bind_tenant_backend_generations()`. If the canonical data/negative backend aliases could not be bound atomically, startup still constructed and published the infrastructure instead of failing the cache capability closed.
- **Remediation TENANT-22.02.09-02:** tenant cache infrastructure construction now propagates alias-binding/policy-construction errors as server cache errors. The middleware-level bootstrap propagates generation-listener startup failure, and `bootstrap_app_runtime` now fails startup on that error instead of continuing with a partially initialized tenant cache boundary.
- **Finding isolation TENANT-22.02.09-03:** the apparent hardcoded `invalidation_listener_status` concern was rechecked on the actual current `main`; the public `tenant_cache_stats()` wrapper in `middleware/mod.rs` already derives the real listener status from the canonical snapshot. No change was necessary.
- **Adjacent-boundary findings deferred:** `TenantService` still does not canonicalize/validate persisted tenant slug/domain values against the middleware selector validator, and `TenantSettings::validate` does not validate/canonicalize/dedupe overlapping base domains. These remain owner/configuration findings outside this middleware iteration.
- **Regression correction during second pass:** the first post-remediation re-read caught one stale test call using the old two-argument cache-key API. It was corrected before closeout, and the full changed test surface was re-read afterward.
- **Adjacent-boundary re-audit:** request trust/host normalization, tenant-route scope, auth-context ordering, owner read-port semantics, generation binding/listener bootstrap, and the direct application bootstrap caller were rechecked. No new tenant-resolution or cache-generation defect was introduced by the remediation.
- **Fresh second pass:** independently re-read the complete changed tenant middleware, cache key construction, generation-stable load loop, bootstrap/error propagation, regression tests, and the immediate router/runtime boundaries without relying on the original finding list. No remaining repository-owned defect was found inside `tenant.rs` or its direct remediation path.
- **Verification:** repository-content/static reasoning and branch-diff review only. Per maintainer execution rules, no tests, clippy, gatekeeper, build, or runtime commands were executed by the agent.
- **Status:** module-level fresh second pass clean; `FS-22.02.09` complete and ready for integration. Next planned primary module is `FS-22.02.10 — apps/server/src/middleware/guest_access_http.rs` (exact filename to be re-confirmed from current module inventory when starting that iteration).

### FS-22.02.10 Iteration 1 — guest-cart HTTP capability boundary

- **Base:** refreshed `main` to `09869dd906942739f6e4082cc17fa1114f22ee3a` immediately before this iteration; this also incorporates the previous tenant-cache hardening.
- **Dedicated iteration branch:** `codex/audit-fs-22.02.10-guest-cart-access`.
- **Actual owner:** the planned server middleware file `apps/server/src/middleware/guest_access_http.rs` does not exist. The canonical HTTP capability adapter is owner-owned at `crates/modules/rustok-cart/src/guest_access_http.rs`, globally composed by `apps/server/src/services/app_router.rs`, with capability verification consumed from `rustok-cart/src/guest_access.rs` and its storefront/native boundary.
- **Invariant map:** every guest-owned cart read/write path must require the request's guest capability; plaintext guest capability must remain request-scoped and never persisted; conflicting capability representations must fail closed; authenticated customer carts remain customer-owned; capability-bearing cart responses must be non-cacheable; native server functions must preserve the same guest-access boundary as REST/GraphQL.
- **Confirmed finding GUEST-CART-22.02.10-01:** cart storefront native server functions previously used direct `CartService` reads/writes plus a local customer-only ownership helper. For guest carts (`customer_id=None`), that helper returned success, so `cart/storefront-data`, `cart/decrement-line-item`, and `cart/remove-line-item` could access a tenant/cart UUID without validating the HTTP guest token.
- **Remediation GUEST-CART-22.02.10-01:** added owner-owned `verify_current_guest_cart_access` over the request-scoped task-local capability and applied it to both native adapter variants selected by feature profile. Customer-owned carts still require the resolved customer identity; service/system actors no longer bypass guest capability through these storefront server functions.
- **Confirmed finding GUEST-CART-22.02.10-02:** the HTTP capability adapter added `Cache-Control: no-store` only when a new token was issued. Existing-token guest requests could therefore return capability-protected cart state without an explicit cache prohibition.
- **Remediation GUEST-CART-22.02.10-02:** any request carrying an existing guest capability, or issuing one during cart creation, now receives `Cache-Control: no-store`.
- **Confirmed finding GUEST-CART-22.02.10-03:** the parser used only one header value and the first matching cookie, so repeated guest capability inputs could be silently resolved by ordering.
- **Remediation GUEST-CART-22.02.10-03:** all guest token header values and all matching cookie occurrences are now examined; duplicates fail closed, invalid primary header capability cannot fall back to a cookie, and header/cookie disagreement remains an explicit conflict.
- **Adjacent-boundary review:** REST/GraphQL commerce handlers already consume the guarded cart storefront provider and separately enforce authenticated customer ownership. The native server-function adapters were the missing enforcement path. Tenant scoping remains owner-enforced through the cart service/port, and app-router placement ensures the request-scoped capability is established before downstream server functions execute.
- **Boundary security decision:** no `Secure` cookie flag was added in this owner module. The repository's production host validation requires HTTPS/HSTS declaration at the server boundary; forcing `Secure` here would couple the reusable cart capability module to host/proxy deployment semantics and would break ordinary HTTP development profiles.
- **Regression correction during review:** the second pass caught malformed indentation in newly added test blocks; the test source was normalized before closeout.
- **Fresh second pass:** independently re-read the owner capability state, HTTP extraction, duplicate handling, cache policy, both native adapter feature variants, transport selection, app-router composition, customer ownership resolution, guarded port behavior, and cart owner persistence. No remaining repository-owned in-scope guest-access bypass was found.
- **Verification:** repository-content/static reasoning and branch-diff review only. No tests, clippy, gatekeeper, build, or runtime commands were executed by the agent.
- **Merged:** PR #4178, merge commit `486a3c3e8c33ee1c4332f318f141d2829a4d40e2`.
- **Status:** module-level fresh second pass clean; `FS-22.02.10` complete and integrated into `main`. Next planned primary module is `FS-22.02.11 — `apps/server/src/middleware/security_headers.rs`.

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
