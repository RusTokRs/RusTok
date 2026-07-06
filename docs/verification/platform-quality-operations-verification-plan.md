---
id: doc://docs/verification/platform-quality-operations-verification-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Verification Plan: Quality and Operational Readiness

- **Status:** current detailed checklist
- **Scope:** quality-gates, observability, release-readiness, security/dependency hygiene, documentation-code synchronization
- **Companion plan:** [Main Platform Verification Plan](./PLATFORM_VERIFICATION_PLAN.md)

---

## Current Quality and Operations Contract

This plan is not for the historical log of CI incidents, but to confirm
that the local and CI quality verification path reflects the current platform code.

The check covers:

- local quality checks that must be reproducible without GitHub as the source of truth
- observability and operational surfaces that must match the runtime contract
- release-readiness and security/dependency hygiene
- synchronization of central docs, local docs and verification entrypoints

## Phase 1. Local Quality Baseline

### 1.1 Rust workspace baseline

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] targeted `cargo test`, if runtime contract, DTO, migration path or shared service layer changed

### 1.2 Module baseline

- [ ] `cargo xtask validate-manifest`
- [ ] targeted `cargo xtask module validate <slug>` for affected modules
- [ ] targeted `cargo xtask module test <slug>` for affected modules
- [ ] Support/capability crates do not replace scoped module checks.

### 1.3 Frontend/i18n baseline

- [ ] `npm run verify:i18n:ui`
- [ ] `npm run verify:i18n:contract`
- [ ] `npm run verify:storefront:routes`, if storefront routes, locale-prefixed paths or host/UI wiring changed

## Phase 2. Observability and Operational Readiness

### 2.1 Runtime observability contract

- [ ] `/metrics`, `/health`, `/health/live`, `/health/ready`, `/health/runtime`, `/health/modules` match the current host/runtime contract.
- [ ] Tracing, OTEL and logging do not diverge from server bootstrap and operational docs.
- [ ] Build progress, background tasks, workflow/outbox/cache operational flows do not lose the observability contract during runtime changes.

### 2.2 Local operational tooling

- [ ] `scripts/verify/*` and `scripts/architecture_dependency_guard.py` reflect the current code and active boundary rules.
- [ ] For Windows, the mandatory local path remains executable without Bash as a hard prerequisite.
- [ ] Legacy shell checks, if needed, are documented as a perimeter path, not as the only way to confirm a contract.

### 2.3 Compose and local stack readiness

- [ ] `docker-compose*.yml`, `ops/grafana/`, `ops/prometheus/` and related runbooks do not diverge from the current dev/runtime picture.
- [ ] The observability stack describes the actual local circuit, not a historical rollout.

## Phase 3. Security and Dependency Hygiene

### 3.1 Security baseline

- [ ] Auth/session/RBAC verification notes match the current server/runtime contract.
- [ ] Tenant isolation, input validation and secret handling do not diverge from central docs and local docs.
- [ ] Capability crates and automation paths do not bypass the common authorization model.

### 3.2.1 CI non-regression gates

- [x] `platform-contract` workflow contains `cargo xtask validate-manifest` and `cargo xtask module validate`.
- [x] Coverage threshold is taken from `scripts/ci/coverage-threshold.env` (`RUSTOK_MIN_COVERAGE_PERCENT`) and applied via `scripts/ci/check-coverage.sh`.
- [x] CI publishes LCOV artifact, SBOM/provenance job remains in required aggregate, and `cargo-deny-action` is not removed from security gates.
- [x] `scripts/ci/check-dependabot-directories.py` confirms that all directories from `.github/dependabot.yml` exist and stale paths are not returned.

### 3.2 Dependency and manifest hygiene

- [ ] `cargo deny`, `cargo audit` and similar quality tools are treated as quality signals consistent with the current workflow.
- [ ] Manifest hygiene does not duplicate the scoped module contract and does not conflict with `cargo xtask validate-manifest`.
- [ ] Support tooling is not documented as a mandatory gate if it is not reproducible in the current local baseline.

## Phase 4. Documentation Sync and Release-Readiness

### 4.1 Central docs

- [ ] `docs/index.md`, `docs/verification/README.md`, `docs/modules/*`, `docs/architecture/*`, `docs/UI/*` reflect the current code and navigation.
- [ ] Verification plans remain a checklist layer, not an archive of past CI failures.
- [ ] Old rollout/install/investigation notes are either updated or explicitly superseded by current live docs.

### 4.2 Local docs

- [ ] Changed `apps/*` and `crates/*` synchronize root `README.md`, `docs/README.md` and `docs/implementation-plan.md`.
- [ ] Public contracts in `README.md` remain in English, central docs in `docs/` are now also in English.
- [ ] Documentation describes the actual source of truth, not temporary workarounds.

### 4.3 Release-readiness

- [ ] The local quality baseline is reproducible before publishing changes.
- [ ] Environment blockers are recorded separately and do not mask code/docs drift.
- [ ] Release/readiness notes do not live only in CI; critical limitations are reflected in local docs and runbooks.

## Targeted Local Checks

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] `cargo xtask validate-manifest`
- [ ] targeted `cargo xtask module validate <slug>`
- [ ] targeted `cargo xtask module test <slug>`
- [ ] `npm run verify:i18n:ui`
- [ ] `npm run verify:i18n:contract`
- [ ] `npm run verify:storefront:routes`, if storefront/runtime routing contract is affected
- [ ] `powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1`, if architecture/runtime boundary changed

## Open Blockers

- [ ] Do not turn this document into a list of one-time CI errors or GitHub-specific workarounds.
- [ ] Record local prerequisites and environment blockers briefly and separately from the contract layer.
- [ ] Any new quality gate should first be described as a locally reproducible workflow, and only then as a CI integration.

## Related Documents

- [Main Verification README](./README.md)
- [Foundation verification](./platform-foundation-verification-plan.md)
- [Performance baseline](../architecture/performance-baseline.md)
- [Runtime guardrails](../guides/runtime-guardrails.md)
- [`rustok-module.toml` Contract](../modules/manifest.md)


## Docs quality gates (DOC-07 baseline)

The purpose of this block is to establish minimum docs quality gates that
must be executed for PRs with documentation changes.

### Minimum gates

1. Markdown lint on changed files:

```bash
npx --yes markdownlint-cli <changed-files>
```

2. Link-check on changed files:

```bash
lychee --no-progress <changed-files>
```

3. Manual check of changed links and anchors in `docs/index.md` + affected
   `README.md`/`docs/*.md` sections.

### Status rules

- `pass` — command completed with `exit code 0`;
- `fail` — command completed with non-zero `exit code`;
- `blocked` — command cannot be executed due to environment limitations
  (e.g., `lychee` binary not available).

For `fail`/`blocked`, a `Verification Evidence` block with the reason and
follow-up task is mandatory in the PR.

### Boundaries of this document

This plan captures the verification contract and does not replace CI workflow files.
Integration into CI is done as a separate change when explicitly requested.
