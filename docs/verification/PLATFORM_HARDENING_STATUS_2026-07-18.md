---
id: doc://docs/verification/PLATFORM_HARDENING_STATUS_2026-07-18.md
kind: verification_status
language: markdown
status: active
as_of: 2026-07-18
---
# Platform Hardening Status — 2026-07-18

This addendum records source-level changes that landed after the last full rewrite of `PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md`. Where the older plan still describes an item as pending, this document is the current source-level status until the plan is safely rebased on the active `main` branch.

## Completed source contracts

- **HARD-101 — CSP enforcement:** server-hosted and standalone admin policies now enforce `style-src-attr 'none'`; Rust and Next inline-style registers are empty and ratcheted to zero. The bounded report collector remains enabled for operational evidence.
- **HARD-202 — Rust-host browser smoke:** a commit-pinned Playwright workflow prepares PostgreSQL through the migration harness under a bounded `CREATEDB NOSUPERUSER` role, builds embedded admin assets with exact Trunk inputs, starts the real monolith and exercises `/health`, server-hosted storefront and `/admin/` under strict CSP assertions. Runtime execution evidence is still pending.
- **HARD-204 — API compatibility:** base and head OpenAPI/GraphQL artifacts are generated and compared by a base-owned `pull_request_target` policy. Exception-register and infrastructure changes require explicit approval.
- **HARD-205 — migration compatibility:** append-only plans, fresh/incremental PostgreSQL smoke, N-1 upgrade, rollback smoke and exact backfill fixtures are connected through artifacts. Database execution uses a bounded `LOGIN CREATEDB NOSUPERUSER` role.
- **HARD-206 — release source contract:** canonical signed SemVer tags, workspace/lock/changelog parity, two-job archive reproducibility, server-only SPDX 2.3 dependency closure, exact SHA-256 assets, GitHub attestations, GHCR digest publication and immutable GitHub Release preflight are implemented.
- **Embedded admin release inputs:** clean release and container builds now prepare `apps/admin/dist` before compiling `rustok-server`; Trunk is fixed at `0.21.14`, Cargo locking is required, Tailwind uses a portable Node hook and embedded assets are generated for `/admin/`.
- **Release trust boundary:** release, policy and hardening workflows use commit-pinned GitHub-owned actions. Changes to release infrastructure require `release-infra-approved` or explicit dispatch approval.
- **Runtime image inputs:** the release image uses dated Debian `bookworm-20260713-slim` pinned to index digest `sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818`; runtime packages resolve from Debian Snapshot `20260713T000000Z`. The image runs as UID/GID `10001` and carries max provenance plus an SBOM.

## Open operator and repository work

1. **Branch protection is still unverified.** Required checks must include browser E2E, Rust-host browser smoke, hardening, API compatibility, migration compatibility and release-infrastructure policy before production-ready status.
2. **Dependency lock refresh remains pending.** Regenerate `Cargo.lock`, prove `rsa` and `atomic-polyfill` are absent from the selected graph, run audit tools and remove the final two waivers only after evidence passes.
3. **First production release evidence is pending.** Enable repository immutable releases, create a signed annotated tag, run the tag workflow, verify five release assets, checksums, attestations and the GHCR digest.
4. **Browser/runtime execution remains pending.** Run the Next and Rust-host workflows, then separately smoke standalone admin and Page Builder custom viewport, zoom, iframe, overlays, drag/drop and resize interactions.
5. **Failed-release recovery must be rehearsed.** A failure after the immutable version image tag is pushed but before GitHub Release publication can block a blind rerun. Follow the release readiness checklist and record an incident-approved recovery rather than deleting or overwriting published evidence casually.
6. **Standalone admin container topology remains under review.** The historical nginx CSR Dockerfile does not exercise the nonce-bearing SSR security adapter; production evidence should use the SSR host or explicitly document a separate static CSP contract.

## Required validation evidence

Run these locally or in CI; no result should be inferred from source inspection alone:

```bash
cargo fmt --all -- --check

node scripts/verify/verify-csp-reporting-contract.mjs
node scripts/verify/verify-csp-inline-style-exceptions.mjs
node scripts/verify/verify-csp-next-style-boundary.mjs

node scripts/verify/verify-api-compatibility-self-test.mjs
node scripts/verify/verify-api-compatibility-infra-self-test.mjs
node scripts/verify/verify-api-compatibility-contract.mjs

node scripts/verify/verify-migration-plan-self-test.mjs
node scripts/verify/verify-migration-backfill-self-test.mjs
node scripts/verify/verify-migration-infra-self-test.mjs
node scripts/verify/verify-migration-compatibility-contract.mjs

node scripts/verify/verify-release-tooling-self-test.mjs
node scripts/verify/verify-release-infra-self-test.mjs
node scripts/verify/verify-release-supply-chain-contract.mjs
node scripts/verify/verify-release-runtime-image-contract.mjs
node scripts/verify/verify-release-readiness-contract.mjs
node scripts/verify/verify-rust-host-browser-contract.mjs

cargo generate-lockfile
cargo tree -i rsa --workspace --all-features
cargo tree -i atomic-polyfill --workspace --all-features
cargo audit
```

## Evidence rule

“Implemented” in this addendum means the source contract and regression gate exist. It does **not** mean tests, CI, a browser smoke, a migration run or a production release succeeded. Those results must be recorded separately with exact commit, run, artifact and environment identifiers.
