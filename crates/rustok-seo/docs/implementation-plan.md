# `rustok-seo` implementation plan

## 1. Purpose

This document is the executable roadmap for taking `rustok-seo` from a broad,
feature-complete SEO control-plane to a production-grade, independently
extensible SEO platform component.

The plan is based on a source audit of the current `main` branch performed on
2026-07-14. It replaces the previous closeout-only checklist with a risk-driven
sequence that covers correctness, security, architecture, performance,
operability, storefront parity, and long-term SEO capabilities.

The intended outcome is not a rewrite. Existing public contracts remain
additive `v1` surfaces, owner modules continue to own entity authoring, and
`SeoPageContext` remains the canonical storefront contract.

## 2. Current architecture

`rustok-seo` currently provides:

- tenant-aware metadata with explicit, generated, and fallback precedence;
- typed `SeoPageContext` output for Rust and Next storefronts;
- redirects, robots, sitemap generation, sitemap submission, and job history;
- typed JSON-LD blocks and schema validation;
- bulk remediation/import/export jobs;
- diagnostics and cross-link suggestions;
- revisions and rollback;
- typed SEO events, outbox delivery tracking, index delivery tracking, repair,
  replay, and dead-letter handling;
- GraphQL and REST control-plane surfaces;
- owner integration through `rustok-seo-targets` providers;
- admin support through `rustok-seo-admin-support`.

The architectural direction is correct: owner modules publish typed SEO target
capabilities and `rustok-seo` centralizes policy and the storefront read model.
The remaining work is primarily about enforcing boundaries, making writes and
delivery atomic, replacing full-scan paths with batch/read-model paths, and
closing production evidence.

## 3. Verified strengths to preserve

1. **Canonical storefront contract.** `SeoPageContext = route + document`
   provides a stable typed boundary for metadata, redirects, canonical URLs,
   hreflang, robots, social cards, pagination, and JSON-LD.
2. **Owner-module responsibility.** Entity-specific authoring remains in
   pages/product/blog/forum admin surfaces rather than moving into a monolithic
   SEO editor.
3. **Capability registry.** `rustok-seo-targets` already supplies the correct
   extension seam for authoring, routing, bulk summaries, and sitemap
   candidates.
4. **Typed effective-state provenance.** Explicit/generated/fallback source
   information is exposed to clients and diagnostics.
5. **Typed schema blocks.** Runtime consumers no longer need to interpret one
   untyped JSON blob.
6. **Tenant/RBAC-aware transports.** GraphQL and REST expose additive control
   surfaces with module and permission checks.
7. **Operational primitives.** Delivery tracking, replay, repair, cursors,
   bounded retries, and runbooks are present and should be hardened rather than
   replaced.
8. **Host boundary guardrails.** The admin package and storefront fixtures
   already have structural verification scripts.

## 4. Audit findings and priority

### P0 — configuration corruption can be hidden

`SeoService::load_settings` currently deserializes tenant settings with
`unwrap_or_default()`. A malformed or incompatible payload therefore silently
turns into defaults. This can unexpectedly enable sitemaps, remove allowlists,
or change robots/template behavior without an operator-visible failure.

**Required correction:** introduce versioned settings, strict validation,
explicit migration, and an observable invalid-configuration state. Defaults are
valid only when no tenant override exists, not when an override is malformed.

### P0 — write and event delivery boundaries are not atomic

Metadata, translations, revisions, redirects, and job state are persisted before
SEO events are published. Publication errors are logged and swallowed. Several
multi-step write paths also run without one database transaction.

This permits states such as:

- metadata updated but one or more translations not updated;
- entity state committed but no durable SEO event recorded;
- revision number calculated concurrently and then rejected by a unique
  constraint;
- delivery tracker state and outbox state diverging after a partial failure.

**Required correction:** all state-changing use cases must use a unit-of-work
that commits domain data and a durable outbox record in one transaction. Direct
best-effort publication remains an adapter concern after commit, never the only
record of the change.

### P0 — sitemap submission permits SSRF-capable destinations

Tenant settings accept arbitrary `http`/`https` sitemap submission endpoints.
The runtime then performs server-side requests to those endpoints. Scheme
validation alone does not prevent loopback, private-network, link-local,
metadata-service, redirect-chain, DNS rebinding, or credential-in-URL targets.

**Required correction:** apply a shared outbound URL policy with hostname/IP
resolution checks, redirect restrictions, explicit provider allowlists or
operator-approved destinations, bounded response bodies, and audit logging.
Production should default to known adapters rather than arbitrary URLs.

### P0 — public base URL can fail open to localhost

`public_base_url` falls back to environment variables and finally
`http://localhost:5150`. That is useful for development but unsafe as a silent
production fallback because it can generate invalid canonical, robots, and
sitemap URLs.

**Required correction:** introduce a validated `PublicOrigin` value object.
Production startup or tenant activation must fail validation when no public
origin is available. Development fallback must be explicit and environment
scoped.

### P1 — production dependency graph still couples SEO to owner modules

The runtime uses the provider registry, but the `server` feature still directly
depends on pages, product, blog, forum, and commerce crates, and service code
re-exports owner services. Built-in registry construction is test-only, yet the
production feature graph remains coupled.

**Required correction:** keep provider implementations in owner modules and
register them at host composition time. Move any integration fixtures to a test
support crate or dev-only feature. `rustok-seo` production dependencies must not
require specific content modules.

### P1 — redirect resolution is a per-tenant full list plus linear scan

The cache stores all redirect rows for a tenant. Every request performs a linear
exact scan followed by a linear wildcard scan. Cache invalidation is process
local, and wildcard precedence is based on row order rather than an explicit
specificity policy.

**Required correction:** compile redirects into an immutable resolver snapshot:

- hash lookup for exact routes;
- deterministic wildcard ordering by specificity and priority;
- validation for ambiguous/overlapping patterns;
- versioned invalidation across application instances;
- metrics for cache age, load size, hits, misses, and resolution latency.

### P1 — bulk and diagnostics paths perform full-scope work in request flows

Current flows collect complete provider scopes in memory, repeatedly call
`seo_meta`, and in diagnostics also resolve page context per target. Bulk queue
creation resolves all target IDs before enqueueing. CSV import payloads are
stored in job JSON, and list/status queries sometimes load all rows before
applying limits or aggregation in application code.

**Required correction:** add cursor-based batch capabilities to the existing
provider port, SQL-side pagination/aggregation, batch metadata loading, durable
job checkpoints, and asynchronous read models.

### P1 — sitemap generation is synchronous and memory-oriented

Sitemap generation collects all URLs into memory, renders all files, persists
content rows sequentially, and submits endpoints in the same use case. Large
sites will experience long request lifetimes, high memory usage, and difficult
retry semantics.

**Required correction:** separate generation, persistence/publication, and
submission into idempotent job steps. Stream provider candidates, write
artifacts incrementally, and publish an atomic manifest only after successful
generation.

### P1 — module/RBAC checks are concentrated in transports

Transport checks are present, but internal service methods do not consistently
enforce module state or operation policy. New adapters, workers, or tests can
accidentally call a control-plane method without the same guard.

**Required correction:** model caller intent in application commands and apply
module/policy enforcement at the application boundary. Transport checks remain
an additional layer.

### P2 — SEO quality rules need versioning and incremental evaluation

Diagnostics already cover important errors, but the current implementation is a
single synchronous summary with hard-coded rules and a fixed exposed issue
limit. Rule changes can alter readiness scores without a policy version, and
large tenants require a materialized/incremental read model.

**Required correction:** introduce versioned rule packs, stable issue identity,
suppression/acknowledgement, incremental recomputation, and score versioning.

### P2 — production observability and SLOs are incomplete

The module logs many failures, but it needs a unified metrics and trace model for
public resolution, control-plane writes, jobs, external submissions, event
outbox, index delivery, and diagnostics freshness.

**Required correction:** define SLOs, metrics, alerts, dashboards, and trace
correlation before final live closeout.

## 5. Target architecture

### 5.1 Package boundaries

The target dependency direction is:

```text
owner modules ──implement──> rustok-seo-targets contracts
      │                             │
      └──register providers─────────┘
                                    │
host composition ───────────────> rustok-seo application
                                    │
                                    ├── ports: repositories / outbox / clock
                                    ├── read models: redirects / diagnostics
                                    └── adapters: DB / HTTP / object storage
```

Rules:

- `rustok-seo` must not import owner application services in production code;
- owner modules must not render final SEO documents;
- hosts must not implement local target mappings, precedence, or raw JSON-LD
  handling;
- transports call application commands/queries and do not own business rules;
- repositories and outbound HTTP are adapters behind explicit ports;
- durable events are written in the same transaction as domain state.

### 5.2 Application components

Split the current broad `SeoService` facade internally into focused components
while keeping public compatibility during migration:

- `SeoMetaApplication` — resolve, upsert, publish revision, rollback;
- `SeoRouteApplication` — route resolution and canonical/hreflang policy;
- `SeoRedirectApplication` — redirect authoring and resolver snapshots;
- `SeoSitemapApplication` — job orchestration and artifact publication;
- `SeoBulkApplication` — selection, preview, execution, resume/cancel;
- `SeoDiagnosticsApplication` — snapshots, rules, suppressions;
- `SeoDeliveryApplication` — outbox/index status, repair, replay;
- `SeoSettingsApplication` — versioned settings and validation.

`SeoService` may remain as a compatibility facade that delegates to these
components until callers are migrated.

### 5.3 Core ports

Add or formalize:

- `SeoUnitOfWork`;
- `SeoMetaRepository` and batch read methods;
- `SeoSettingsRepository`;
- `SeoRedirectRepository` plus `RedirectSnapshotStore`;
- `SeoJobRepository` with lease/checkpoint semantics;
- `SeoArtifactStore` for CSV and sitemap artifacts;
- `SeoOutboxWriter`;
- `SeoOutboundHttpPolicy`;
- `SeoClock` and deterministic ID/idempotency services;
- batch/cursor methods on `SeoTargetProvider` rather than new owner-specific
  interfaces.

### 5.4 Error and failure policy

- Invalid persisted settings: visible configuration error; no silent default.
- Invalid public origin: fail closed in production.
- Domain write succeeds only when domain state and outbox state commit together.
- External submission failure never rolls back generated sitemap artifacts, but
  remains an explicit step state with retry policy.
- Provider failure is isolated per provider/job partition and is visible in job
  results.
- Public SEO read paths use explicitly classified fallback behavior and emit
  telemetry whenever fallback occurs.

## 6. Delivery phases

## Phase 0 — baseline, contracts, and measurable acceptance

**Priority:** P0  
**Dependencies:** none  
**Goal:** create a safe baseline before behavior changes.

### Work

- Record current GraphQL/REST/Next/Rust fixture outputs as compatibility goldens.
- Add benchmark fixtures for 1k, 10k, and 100k targets and redirects.
- Document existing database constraints and add missing invariant tests.
- Define public API additive-compatibility rules and deprecation procedure.
- Define initial SLOs and a performance budget.
- Add an architecture decision record for transaction/outbox ownership and host
  provider composition.

### Deliverables

- contract snapshot and semantic parity suite;
- benchmark harness and reproducible seed data;
- ADRs for unit-of-work/outbox, provider registration, artifact storage, and
  outbound URL policy;
- issue map linking every following phase to code and tests.

### Definition of done

- existing fixtures pass unchanged;
- baseline p50/p95/p99 and memory figures are captured;
- every P0 change has a rollback flag or migration rollback procedure.

## Phase 1 — security and data integrity hardening

**Priority:** P0  
**Dependencies:** Phase 0  
**Goal:** remove silent corruption, SSRF exposure, and partial-write states.

### Work

1. Replace `unwrap_or_default` settings parsing with:
   - `schema_version`;
   - strict deserialize + semantic validation;
   - explicit migration from legacy payloads;
   - operator-visible invalid state;
   - audit event on settings changes.
2. Introduce `PublicOrigin` validation and environment-specific fallback policy.
3. Introduce one outbound URL security policy for canonical/redirect/sitemap
   URLs where applicable:
   - allow only `http`/`https`;
   - reject userinfo and unsafe ports by policy;
   - normalize IDNA and host casing;
   - reject loopback, private, link-local, multicast, unspecified, and metadata
     service ranges;
   - revalidate resolved addresses and redirects;
   - cap redirects, timeout, response size, and concurrency.
4. Replace arbitrary sitemap endpoints with typed adapters plus an explicit
   custom-endpoint escape hatch restricted to platform operators.
5. Wrap metadata + translations + revision/outbox writes in one transaction.
6. Wrap redirect state + outbox writes in one transaction.
7. Make revision allocation concurrency safe using a database-enforced sequence
   or bounded retry on unique conflict.
8. Audit and enforce expected unique/check constraints for tenant/target/locale,
   revision, redirect, delivery, and job identities.
9. Add authorization/module-policy checks to application commands and workers.

### Deliverables

- versioned settings model and migration;
- shared URL security module with DNS/IP tests;
- transactional command handlers and durable outbox writer;
- migration set for constraints/indexes;
- negative security and concurrency test suite.

### Definition of done

- malformed settings cannot silently activate defaults;
- no sitemap submission can reach disallowed network ranges, including through
  DNS or redirects;
- fault-injection tests prove no partial metadata/translation/outbox commit;
- 100 concurrent revision publications produce a contiguous unique sequence or
  a documented monotonic equivalent;
- policy checks are identical through GraphQL, REST, admin server functions, and
  workers.

## Phase 2 — redirect and query-path performance

**Priority:** P1  
**Dependencies:** Phase 1 constraints  
**Goal:** make hot public paths predictable under large tenant datasets.

### Work

- Build immutable `RedirectResolverSnapshot` with exact map and deterministic
  wildcard matcher.
- Define wildcard priority/specificity and reject ambiguous patterns.
- Add cache generation/version and cross-instance invalidation.
- Push list limits, filtering, and delivery status aggregation into SQL.
- Add covering indexes based on measured query plans.
- Add batch metadata/translation reads and remove N+1 calls from bulk lists.
- Cache validated normalized settings by tenant/version.
- Add request coalescing for concurrent snapshot/settings loads.

### Deliverables

- redirect resolver component and migration tooling;
- query plan tests and DB-side aggregation queries;
- batch metadata repository API;
- performance dashboard and regression thresholds.

### Definition of done

- exact redirect lookup is amortized O(1);
- wildcard lookup follows a documented deterministic policy;
- redirect cache invalidation propagates across instances within the agreed SLO;
- bulk list does not issue one metadata query per row;
- p95 public SEO context and redirect latency meet the Phase 0 budget at the
  100k fixture size.

## Phase 3 — complete modular decoupling

**Priority:** P1  
**Dependencies:** Phase 0 ADRs; can overlap late Phase 2  
**Goal:** make SEO installable without pages/product/blog/forum dependencies.

### Work

- Remove owner crate dependencies from the production `rustok-seo` feature
  graph.
- Remove owner-service re-exports from SEO service modules.
- Move built-in integration assembly to host composition or a dedicated test
  support package.
- Extend `SeoTargetProvider` with cursor/batch operations needed by bulk,
  diagnostics, and sitemap jobs.
- Add provider capability version metadata and startup compatibility checks.
- Isolate provider failures and expose provider health/readiness.
- Add boundary tests that fail when `rustok-seo` imports owner crates.
- Decompose `SeoService` behind the compatibility facade.

### Deliverables

- clean dependency graph;
- versioned provider contract;
- host composition examples and tests;
- focused application components with unchanged external `v1` contracts.

### Definition of done

- `rustok-seo` compiles and runs with a synthetic provider and no built-in owner
  modules;
- adding a new owner target requires only provider implementation and host
  registration;
- no storefront or transport contains target-kind switch statements outside the
  shared registry/contracts;
- module boundary verification runs in CI.

## Phase 4 — asynchronous, resumable bulk and sitemap pipelines

**Priority:** P1  
**Dependencies:** Phases 1–3  
**Goal:** support large tenants without long request transactions or full-memory
processing.

### Work

- Introduce job leasing, heartbeat, checkpoint, retry, cancel, and resume.
- Persist selection/filter snapshots and enumerate targets in cursor batches.
- Use stable idempotency keys per job partition and target operation.
- Move CSV input/output and sitemap content to `SeoArtifactStore`; keep only
  metadata/checksums in relational tables.
- Stream CSV parsing and generation with explicit file/row/field size limits.
- Split sitemap workflow into:
  1. enumerate candidates;
  2. normalize/deduplicate;
  3. generate chunks;
  4. validate artifacts;
  5. atomically publish manifest;
  6. submit providers;
  7. retain/expire old generations.
- Support incremental sitemap generation from owner change events when the
  dataset warrants it.
- Add per-provider and per-partition failure artifacts.

### Deliverables

- worker runtime and job state machine;
- artifact storage adapter and retention policy;
- resumable bulk and sitemap implementations;
- load/fault tests for worker restart and duplicate delivery.

### Definition of done

- API queue calls remain bounded regardless of selection size;
- a worker can restart at any checkpoint without duplicate final effects;
- a single provider/row failure does not discard successful partitions;
- sitemap publication never exposes a partial generation;
- 100k target fixture completes inside the agreed throughput and memory budget.

## Phase 5 — diagnostics read model and rule engine

**Priority:** P1  
**Dependencies:** provider batches from Phase 3; event reliability from Phase 1  
**Goal:** provide fast, explainable, versioned SEO quality diagnostics.

### Work

- Define stable issue key: tenant + target + locale + rule + policy version.
- Extract current checks into versioned rule packs.
- Separate blocking correctness rules from recommendations and experiments.
- Build an incremental diagnostics snapshot updated from SEO/owner events.
- Add full recompute and reconciliation jobs.
- Store first-seen, last-seen, resolved-at, evidence, and remediation metadata.
- Add suppress/acknowledge/expire workflow with RBAC and audit trail.
- Version readiness score formulas and expose score provenance.
- Keep on-demand validation for preview, but serve tenant summaries from the
  read model.

### Deliverables

- rule engine interfaces and initial rules migrated from current diagnostics;
- diagnostics issue/read-model tables;
- incremental updater and full reconciliation worker;
- admin filters, suppression, and remediation links.

### Definition of done

- diagnostics summary latency is independent of total target count within the
  read-model freshness SLO;
- every score is reproducible from a named policy version;
- event loss or drift is repaired by reconciliation;
- rule additions do not require changing transports or owner modules.

## Phase 6 — SEO completeness and quality features

**Priority:** P2  
**Dependencies:** Phases 3–5  
**Goal:** reach a high-quality modern CMS SEO feature set without weakening the
core boundaries.

### Work

- Add sitemap `lastmod`, image, video, news, and alternate-language extensions
  through typed provider capabilities.
- Add configurable route-level robots policy and environment-wide accidental
  indexing protection for non-production hosts.
- Add canonical/hreflang graph validation, including reciprocal links,
  normalized origins, pagination, and channel/domain rules.
- Expand structured-data validation into versioned schema rule packs with
  required/recommended property diagnostics by target kind.
- Add safe preview for resolved head tags and search/social snippets.
- Add redirect impact preview, chain detection at write time, import/export, and
  bulk conflict reporting.
- Add optional typed submission adapters such as IndexNow where product policy
  requires them; keep arbitrary provider URLs disabled by default.
- Add content freshness, orphan/cross-link, duplicate title/description, and
  thin-content signals through owner-provided safe fields.

### Deliverables

- additive provider capability contracts;
- extended sitemap and schema validators;
- preview and remediation UI;
- documented search-engine integration policy.

### Definition of done

- features are additive and owner-neutral;
- all emitted metadata is covered by Rust/Next parity fixtures;
- schema and sitemap outputs pass internal validators and representative external
  validation smoke tests;
- non-production environments cannot become indexable accidentally.

## Phase 7 — observability, reliability, and operations

**Priority:** P1  
**Dependencies:** begins in Phase 1; completes after Phases 4–6  
**Goal:** make the module supportable under production incidents.

### Work

Define metrics and traces for:

- public page-context latency, result/fallback class, and provider latency;
- redirect cache generation, hits/misses, wildcard depth, and invalidation lag;
- settings validation failures and active invalid configurations;
- command transaction latency and outbox commit failures;
- outbox/index backlog age, retry count, dead letters, and replay progress;
- bulk/sitemap queue depth, lease age, throughput, checkpoint lag, and failures;
- artifact bytes, retention, and cleanup failures;
- sitemap submission result by typed adapter;
- diagnostics snapshot age, drift, issue counts, and recompute duration.

Add:

- trace correlation from API command to DB transaction, outbox event, worker,
  and index request;
- structured audit events for settings, redirects, bulk force-overwrite,
  revision rollback, suppression, replay, and custom endpoint changes;
- dashboards and alerts tied to SLOs;
- backup/restore and retention verification;
- chaos/fault-injection scenarios in CI or staging.

### Suggested SLOs

Initial targets must be confirmed by Phase 0 measurements:

- public SEO context availability: >= 99.95%;
- public SEO context p95 excluding owner backend outage: <= 100 ms server time;
- exact redirect resolution p95: <= 10 ms server time after cache warmup;
- outbox/index delivery freshness p99: <= 5 minutes;
- diagnostics read-model freshness p99: <= 15 minutes;
- no unbounded queue age without an alert;
- zero silent settings fallback and zero partial sitemap publication.

### Definition of done

- every production incident class in the runbooks has a signal, alert, owner,
  and recovery action;
- dashboards are linked from the operations runbook;
- a staging incident drill proves replay, repair, worker restart, and artifact
  rollback procedures.

## Phase 8 — D8/D9 live closeout and rollout

**Priority:** P0 for production claim  
**Dependencies:** at minimum Phases 1, 2, 3, and 7; remaining phases may roll out
incrementally  
**Goal:** replace static readiness claims with deployed evidence.

### Work

- Execute deployed GraphQL/REST semantic parity, RBAC, and module-gating matrix.
- Capture before/after outbox and index delivery counters.
- Validate idempotency, retry, dead-letter, repair, and historical replay.
- Validate Next robots, sitemap, home/non-home metadata, and fallback behavior.
- Validate Leptos page-context and rendered head parity.
- Validate media descriptor success and degraded behavior:
  `omit_image_metadata`, `keep_existing_seo_image`, and relative-URL proxy
  fallback.
- Execute sitemap partial-failure and blocked-SSRF scenarios.
- Execute malformed settings and invalid public-origin scenarios.
- Execute backlog, partial-indexing, bulk restart, sitemap restart, and
  replay/reindex incident drills.
- Collect redacted artifacts and owner sign-off from platform, frontend,
  security, and operations.

### Deliverables

- signed live evidence packet;
- resolved high-severity parity/security defects;
- rollout and rollback record;
- updated fixture status from static seed to live-verified.

### Definition of done

- no open P0 defects;
- no unexplained Rust/Next or GraphQL/REST semantic divergence;
- security review accepts outbound URL and public-origin controls;
- operational reviewers successfully execute recovery without code changes;
- closeout rules are met without bypasses or manual fixture edits that hide a
  runtime failure.

## 7. Workstreams and dependencies

```text
Phase 0 baseline
  ├─> Phase 1 security/integrity ──> Phase 2 hot paths
  │                  │                    │
  │                  └──────────────> Phase 4 async jobs
  ├─> Phase 3 decoupling ──────────> Phase 4 async jobs
  │                  └──────────────> Phase 5 diagnostics
  ├────────────────────────────────> Phase 7 observability
  Phase 4 + Phase 5 ───────────────> Phase 6 quality features
  Phase 1 + 2 + 3 + 7 ────────────> Phase 8 live closeout
```

Recommended execution order:

1. Phase 0;
2. Phase 1 in full;
3. Phase 2 and Phase 3 in parallel;
4. Phase 4 and Phase 5 in parallel after shared batch contracts settle;
5. Phase 7 continuously, with final hardening before closeout;
6. Phase 6 incrementally behind additive capabilities;
7. Phase 8 for production sign-off.

## 8. Pull request slicing

Avoid one cross-cutting rewrite PR. Preferred slices:

1. settings parser/version/error contract;
2. `PublicOrigin` and URL policy;
3. sitemap submission typed adapters and SSRF tests;
4. metadata unit-of-work + outbox;
5. redirect unit-of-work + outbox;
6. revision concurrency and data constraints;
7. SQL-side list/aggregation limits;
8. redirect resolver snapshot and invalidation;
9. batch metadata repository;
10. owner dependency removal and host composition;
11. provider cursor/batch contract;
12. worker/job lease framework;
13. artifact store and CSV streaming;
14. sitemap pipeline;
15. bulk pipeline;
16. diagnostics rule extraction;
17. diagnostics read model and reconciliation;
18. extended sitemap/schema capabilities;
19. dashboards/runbooks/live evidence.

Each PR must include migration/rollback notes, tests, telemetry, and contract
impact. Feature flags must be tenant-aware where mixed-version rollout is
possible.

## 9. Quality gates

### Mandatory per PR

- formatting, lint, and targeted unit/integration tests;
- tenant isolation and RBAC negative tests for new control paths;
- migration up/down or documented irreversible migration approval;
- no new owner-module import in `rustok-seo`;
- no raw host-local SEO precedence or target mapping;
- bounded input, memory, retry, and error-message behavior;
- metrics/traces for new asynchronous or external operations;
- compatibility fixture update only when the change is intentionally additive.

### Mandatory before phase completion

- `cargo xtask module validate seo`;
- `cargo check -p rustok-seo --tests --config profile.dev.debug=0`;
- targeted `rustok-seo` unit/integration/fault tests;
- `cargo check -p rustok-seo-admin --features ssr --config profile.dev.debug=0`;
- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`;
- `cargo check -p rustok-storefront --config profile.dev.debug=0`;
- `cargo check -p rustok-server --lib --config profile.dev.debug=0`;
- `npm --prefix apps/next-admin run lint`;
- `npm --prefix apps/next-admin run typecheck`;
- `npm --prefix apps/next-frontend run lint`;
- `npm --prefix apps/next-frontend run typecheck`;
- `npm --prefix apps/next-frontend run verify:seo-runtime-fixtures`;
- `npm run verify:seo:fba`;
- `node scripts/verify/verify-seo-admin-boundary.mjs`;
- new dependency-boundary, URL-security, transaction fault-injection,
  performance, and job-resume suites introduced by this plan.

## 10. Success metrics

The module is considered mature when:

- zero production owner-module dependencies exist in `rustok-seo`;
- zero silent settings fallbacks occur;
- domain write + durable outbox is atomic for every state-changing command;
- public origin and external destinations are validated and fail closed;
- redirect and page-context latency remain within SLO at the 100k fixture size;
- bulk, sitemap, and diagnostics work is cursor/batch based and restart-safe;
- diagnostics are versioned, explainable, suppressible, and reconciled;
- Rust and Next consume the same canonical contract without local SEO policy;
- live GraphQL/REST/storefront/worker evidence is signed;
- incident recovery is demonstrated, not only documented.

## 11. Immediate next actions

1. Open Phase 0 tracking issues and ADRs.
2. Implement strict settings parsing and invalid-configuration reporting.
3. Implement `PublicOrigin` and shared outbound URL security policy.
4. Transactionalize metadata/translation/outbox and revision allocation.
5. Transactionalize redirects/outbox.
6. Add DB-side limits/aggregations and redirect performance benchmarks.
7. Remove production owner dependencies while preserving provider registration.
8. Execute the updated D8/D9 matrix only after the P0 controls are deployed.

## 12. References

- [SEO documentation](./README.md)
- [SEO replay/repair runbook](./replay-repair-runbook.md)
- [SEO operations runbook](./operations-runbook.md)
- [Runtime parity fixtures](../../../apps/next-frontend/contracts/seo/runtime-parity-fixtures.json)
- [`rustok-seo-targets`](../../rustok-seo-targets/src/lib.rs)
- [SEO service facade](../src/services/mod.rs)
- [Metadata service](../src/services/meta.rs)
- [Redirect service](../src/services/redirects.rs)
- [Sitemap service](../src/services/sitemaps.rs)
- [Bulk service](../src/services/bulk.rs)
- [Diagnostics service](../src/services/diagnostics.rs)
- [Event/index delivery service](../src/services/events.rs)
