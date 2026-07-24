# SEO hardening progress

This checklist is the canonical implementation status for the SEO hardening work.
A task is marked complete only after its implementation has been committed to `main`.
Automated verification is recorded separately because direct pushes currently do not receive GitHub status checks.

## P0 — correctness and security

- [x] Reject malformed persisted SEO settings instead of silently replacing them with defaults. Settings use one unversioned schema. (`71fc61a`, `3b138ad`, `7593f8c`)
- [x] Harden sitemap submission endpoints against SSRF, redirects, proxies, private addresses, and unsafe DNS resolution. (`4292d1e`)
- [x] Require a validated public origin and remove the implicit localhost fallback. (`170588a`, `260410a`)
- [x] Persist redirect mutations, delivery tracking, redirect events, reindex events, and cursors transactionally. (`c28a201`)
- [x] Make redirect event idempotency transition-scoped so disable/reactivate cycles emit fresh events and reindex requests. (`7593f8c`)
- [x] Restrict absolute redirect targets to HTTP(S), reject URL credentials, and normalize trailing-dot hosts. (`7593f8c`)
- [x] Persist sitemap jobs, generated files, delivery tracking, and generated events transactionally. (`5840246`)
- [x] Persist sitemap submission outcome and submitted event transactionally after external HTTP completes. (`5840246`)
- [x] Tenant-scope sitemap file aggregation for job reads. (`7593f8c`)
- [x] Persist SEO metadata, translations, delivery tracking, and reindex events transactionally. (#2056)
- [x] Persist revision creation and its event transactionally. (#2059)
- [x] Persist revision rollback and all resulting events transactionally. (#2061)
- [x] Persist bulk terminal state and terminal event transactionally. (#2051)

## Regression coverage

- [x] Add an integration regression test proving that redirect data and delivery tracking roll back when the transactional event transport fails. (`1d5144c`, `c940afd`)
- [x] Add an integration regression test proving that sitemap jobs, generated files, and delivery tracking roll back when the transactional outbox write fails. (`a4d9476`)
- [x] Add contract coverage for transition-scoped redirect events, safe redirect targets, fail-closed settings, and tenant-scoped sitemap reads. (`7593f8c`)
- [x] Add rollback coverage for metadata and revision transactions. (#2056, #2059, #2061)
- [x] Add rollback coverage for bulk terminal state and terminal event transactions. (#2051)

## P1 — performance and maintainability

- [x] Remove avoidable direct owner dependencies from the SEO crate. (#2064)
- [x] Split the broad `SeoService` facade into focused application services. (#2067)
- [x] Replace the linear redirect cache scan with indexed exact and wildcard lookup structures. (#2078)
- [ ] Remove N+1 query patterns from bulk operations and diagnostics.
  - [x] Load bulk-list explicit metadata and translations in bounded batches and reuse one settings/module snapshot. (#2082)
  - [x] Reuse the batched full projection for bulk selection, preview, queue sizing, and CSV export execution. (#2083)
  - [ ] Batch diagnostics metadata and page-context reads.
- [ ] Move synchronous in-memory SEO pipelines to bounded background execution.
- [ ] Require explicit authorization for worker and operator entry points.
- [ ] Classify retryable, terminal, validation, and configuration failures explicitly.

## Verification status

- [x] Run `cargo fmt --check` for the affected workspace packages. (scoped PR #2022 verification; landed via #2051)
- [x] Run `cargo check -p rustok-seo`. (scoped PR #2022 verification; landed via #2051)
- [ ] Run `cargo test -p rustok-seo`. Full suite currently has nine pre-existing failures outside the bulk terminal slice.
- [x] Compile all SEO tests and run the bulk terminal integration, bulk service unit, and bulk event unit scopes. (scoped PR #2022 verification; landed via #2051)
- [ ] Confirm GitHub Actions status checks for the hardening commits.

The connected local execution environment does not provide a Rust toolchain. PR #2022 supplied scoped Rust verification; PR #2051 is the clean follow-up without the temporary workflow, patch script, or `Cargo.lock` churn. PRs #2056, #2059, #2061, #2064, #2067, #2078, #2082, and #2083 continue the SEO hardening work without fresh test execution at the user's request. The full-suite checkbox remains open because nine pre-existing failures outside these slices still need resolution.
