---
id: doc://docs/verification/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Verification Plans

This section collects verification plans for the main platform circuits and captures the minimum local verification path for the module system.

## Purpose

- store verification plans in one place;
- separate periodic verification from live/remediation documentation;
- provide a single entry point for targeted and broad runs;
- capture mandatory quality gates for platform modules.

Execution plans and remediation backlogs should not live in this section as an endless task list. Only verification rules, target commands, and links to profile plans remain here.

## Main Documents

- [Summary Verification Plan](./PLATFORM_VERIFICATION_PLAN.md)
- [Foundation Layer Verification](./platform-foundation-verification-plan.md)
- [API Surface Verification](./platform-api-surfaces-verification-plan.md)
- [Frontend Surface Verification](./platform-frontend-surfaces-verification-plan.md)
- [Quality and Operational Readiness Verification](./platform-quality-operations-verification-plan.md)
  (including Docs quality gates baseline per DOC-07)
- [Core Integrity Verification](./platform-core-integrity-verification-plan.md)
- [RBAC, Server and Runtime Module Verification](./rbac-server-modules-verification-plan.md)
- [Leptos Library Verification](./leptos-libraries-verification-plan.md)

## Minimum Verification Path for Platform Modules

For scoped platform modules, the canonical local path is:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

`module validate` checks the module contract and local docs, while `module test` builds a targeted test/check plan for the module itself and its UI packages.

If the composition contract of the entire platform changes, additionally run:

```powershell
cargo xtask validate-manifest
```


## Reference Artifacts Pipeline (DOC-09 / B11)

For phase 1 of DOC-09, use a single cross-platform Node.js exporter of reference artifacts:

```bash
node scripts/verify/export-reference-artifacts.mjs artifacts/reference
```

In CI and Unix environments, `scripts/verify/export-reference-artifacts.sh` remains a thin
wrapper over the same exporter; there is no separate Bash implementation.

What the script does:

- generates rustdoc for `rustok-server` and `rustok-workflow` (unless `SKIP_RUSTDOC=1` is set);
- saves OpenAPI to `openapi/openapi.json` and `openapi/openapi.yaml`;
- saves full GraphQL introspection to `graphql/introspection.json`;
- saves GraphQL SDL from `/api/graphql/schema.graphql` to `graphql/schema.graphql`;
- writes `manifest.json` and legacy `manifest.txt` with timestamp/base_url/git commit.

Environment variables:

- `RUSTOK_BASE_URL` — base URL of the server (default `http://127.0.0.1:5150`);
- `SKIP_RUSTDOC=1` — skip `cargo doc` and export only API artifacts.

Minimum verification set for PR (B11):

```bash
cargo xtask --help
node scripts/verify/export-reference-artifacts.mjs artifacts/reference
node scripts/verify/verify-reference-artifacts.mjs artifacts/reference
rg -n "openapi/|graphql/|manifest.json" artifacts/reference -S
```

## Reference Artifacts Pipeline in CI (DOC-09 / B12)

Phase 2 for DOC-09 is executed via the CI job `reference-artifacts` in
`.github/workflows/ci.yml`.

The job must:

- start the runtime (`rustok-server`) and wait for `/api/openapi.json`;
- run `scripts/verify/export-reference-artifacts.sh artifacts/reference`;
- verify the layout and export completeness via `node scripts/verify/verify-reference-artifacts.mjs artifacts/reference`;
- publish `artifacts/reference/**` via `actions/upload-artifact`;
- be included in the aggregate gate `ci-success`.

Minimum check for B12 in PR:

```bash
rg -n "reference-artifacts|export-reference-artifacts|upload-artifact|ci-success" .github/workflows/ci.yml
```

## Windows Hybrid Path

On the current Windows environment, the mandatory local verification path must not depend on Bash as a hard prerequisite.

Minimum Windows-native set:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
npm run verify:i18n:ui
npm run verify:i18n:contract
npm.cmd run verify:storefront:routes
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1
```

Additionally:

- Python-dependent checks are run via the installed Python.
- Bash-only scripts are allowed as legacy/perimeter checks, but not as the sole way to confirm a module contract on this machine.
- Fast source-level runtime invariant checks that do not require full Rust compilation may live in `scripts/verify/*.mjs`; current example is `node scripts/verify/verify-runtime-context-invariants.mjs` for channel context/cache-key, locale-cache metrics and evidence `pages -> page_builder`.
- Migration-safety gate is enforced in CI as a separate job `migration-smoke`: it uses a PostgreSQL service and runs `./scripts/verify/verify-migration-smoke.sh` in apply-from-zero and incremental modes.

## Runtime/Backend Regression Runbook

Quick diagnostics for persistent backend/runtime guardrails:

| Symptom | Quick Check | What to Look at on Failure |
|---|---|---|
| Drift module graph / `pages` dependencies | `cargo xtask validate-manifest` + `node scripts/verify/verify-runtime-context-invariants.mjs` | `modules.toml`, `docs/modules/registry.md`, runtime `dependencies()` evidence and registry contract tests must all consistently hold `pages -> [content, page_builder]`. |
| Channel resolution without locale/OAuth dimensions | `node scripts/verify/verify-runtime-context-invariants.mjs` + targeted `cargo test -p rustok-server middleware::channel` | Source-order middleware chain must execute as `locale -> auth_context -> channel`; `RequestFacts` must take `ResolvedRequestLocale.effective_locale` and `AuthContextExtension.client_id`, and `ChannelCacheKey` must include both fields. |
| Locale DB amplification / cache regression | `cargo test -p rustok-server middleware::locale` and check `/metrics` for `rustok_tenant_locale_cache_hits_total`, `rustok_tenant_locale_cache_misses_total`, `rustok_tenant_locale_db_queries_total`, `rustok_tenant_locale_cache_invalidations_total` | Repeated tenant-bound requests within TTL should produce cache hits, disabled locale should remain limited by tenant policy, invalidation/TTL should update the snapshot. |
| Migration dependency failure | `./scripts/verify/verify-migration-smoke.sh` and `RUSTOK_MIGRATION_SMOKE_INCREMENTAL=1 ./scripts/verify/verify-migration-smoke.sh` | Check `migration_dependencies()` in module crates, aggregation in server migrator, duplicate/cycle/missing dependency tests and FK/cross-module migration order. |

For local short iterations, start with fast source-level checks (`node ...` / `rg`), and leave PostgreSQL smoke for migration changes or CI when the build starts taking too long.

## Roles of `xtask` and `scripts/*` (Updated 2026-05)

To avoid duplicating tooling and diverging contracts:

- `xtask` (Rust) — **canonical entrypoint** for platform and module contracts that must run identically on Linux/macOS/Windows.
- `scripts/verify/*.sh` and `scripts/verify/*.mjs` — **perimeter and specialized audit checks** where fast grep/smoke and shell orchestration are more important.
- `scripts/verify/*.ps1` — parity scripts for Windows where a Bash check is mandatory but must have a native fallback.

Practical criteria for choosing an implementation:

1. **Write in `xtask` (Rust)** if:
   - the check is part of the mandatory module acceptance path;
   - cross-platform support without Bash is required;
   - structured parsing is needed (`modules.toml`, manifests, wiring, registry contracts).
2. **Keep in `sh`/`mjs`** if:
   - it is a perimeter/security smoke with many external CLIs;
   - the check is ad-hoc audit in nature and not a module gate;
   - speed of edits in CI orchestration is critical.
3. **Remove/collapse duplicates** if:
   - one script only proxies another without added logic;
   - the command is already covered by `cargo xtask ...` and does not add a separate contract.

## Page Builder FBA Verification Baseline (Wave 0/Wave 1 Gate)

For the `page_builder -> pages` track, the mandatory minimum gate before advancing between waves:

```bash
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs
```

Baseline gate composition:

1. parity of provider/consumer contract versions;
2. required fallback/toggle profile structure;
3. toggle profile value consistency (`all_on/publish_off/preview_off/builder_off`).

This baseline gate is used as a mandatory artifact for Sprint/Wave evidence in `docs/modules/page-builder-implementation-plan.md`.

## What Is Considered Mandatory for Module Unification

When changing the module system or a module's local contract, verify not only the code but also the documentation layer:

- presence of `README.md`, `docs/README.md`, `docs/implementation-plan.md`;
- consistency of `modules.toml` and `rustok-module.toml`;
- correctness of admin/storefront manifest wiring;
- accuracy of central docs in `docs/modules/*` and `docs/index.md`.

Support/capability crates may participate in general documentation unification, but scoped `module validate` applies only to slugs from `modules.toml`.

## How to Use the Plan Set

1. Start with the [summary verification plan](./PLATFORM_VERIFICATION_PLAN.md) if a broad run is needed. Its current-cycle cursor is the source of truth for resume order: Core modules first, then `apps/server`, non-module foundation crates, optional/domain modules, public surfaces, and closing gates.
2. Switch to the profile plan if a specific circuit is changing: foundation, API, frontend, RBAC, UI libraries.
3. For targeted module work, first run `cargo xtask module validate <slug>`, not a full workspace-wide run.
4. Record unresolved blockers in the profile plan or in the local docs of the corresponding component, rather than turning `docs/verification/README.md` into a backlog.

## Cyclic Pre-Release Runs

The summary plan is also the durable controller for repeated agent sweeps. During a
cycle, each visited component records the current cycle identifier, status, findings,
fixes, evidence, next action, and resume command under
`## Periodic release verification handoff` in its existing
`docs/implementation-plan.md`.

Only the summary plan owns resettable queue marks and the active cursor. The local
handoff owns component-specific resume evidence. A local completion from an older
cycle never completes the component in a new cycle. After the closing gate, the agent
increments the cycle identifier, resets the summary queue, and starts again from the
Core modules. `rustok-core` remains a separate foundation crate and must not be
described as a Core module.

### Principle for Operational Script Tests

- Tests in `scripts/tests/*` and `scripts/ci/test_*.py` must use isolated fixture directories (`mktemp` / `tempfile`) and not depend on the current state of the repository.
- The repository may temporarily contain drift/legacy data; this must not make script tests flaky during local runs and in CI.

## Update Policy

When architecture, API, UI contracts, module system, observability, or quality gates change:

1. Update local docs of the affected `apps/*` or `crates/*`.
2. Update the profile verification plan in this folder if the verification procedure itself changed.
3. Update related central docs in `docs/modules/*`, `docs/architecture/*` and `docs/index.md`.
4. If a module's acceptance contract changes, synchronously update the [manifest-layer contract](../modules/manifest.md).

## Statuses

- `Not started`
- `In progress`
- `Completed`
- `Blocked`

> Document status: current. For the module system, this README must remain synchronized with `cargo xtask module validate`, `cargo xtask module test` and central docs in `docs/modules/*`.
