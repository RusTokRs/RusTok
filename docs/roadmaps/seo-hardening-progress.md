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
- [ ] Persist SEO metadata, translations, delivery tracking, and reindex events transactionally.
- [ ] Persist revision creation and its event transactionally.
- [ ] Persist revision rollback and all resulting events transactionally.
- [ ] Persist bulk terminal state and terminal event transactionally.

## Regression coverage

- [x] Add an integration regression test proving that redirect data and delivery tracking roll back when the transactional event transport fails. (`1d5144c`, `c940afd`)
- [x] Add an integration regression test proving that sitemap jobs, generated files, and delivery tracking roll back when the transactional outbox write fails. (`a4d9476`)
- [x] Add contract coverage for transition-scoped redirect events, safe redirect targets, fail-closed settings, and tenant-scoped sitemap reads. (`7593f8c`)
- [ ] Add rollback coverage for metadata and revision transactions.
- [ ] Add rollback coverage for bulk terminal state and terminal event transactions.

## P1 — performance and maintainability

- [ ] Remove avoidable direct owner dependencies from the SEO crate.
- [ ] Split the broad `SeoService` facade into focused application services.
- [ ] Replace the linear redirect cache scan with indexed exact and wildcard lookup structures.
- [ ] Remove N+1 query patterns from bulk operations and diagnostics.
- [ ] Move synchronous in-memory SEO pipelines to bounded background execution.
- [ ] Require explicit authorization for worker and operator entry points.
- [ ] Classify retryable, terminal, validation, and configuration failures explicitly.

## Verification status

- [ ] Run `cargo fmt --check` for the affected workspace packages.
- [ ] Run `cargo check -p rustok-seo`.
- [ ] Run `cargo test -p rustok-seo`.
- [ ] Confirm GitHub Actions status checks for the hardening commits.

The current execution environment does not provide a Rust toolchain, and direct commits have not received GitHub status checks. These verification boxes must remain open until they are actually executed.
