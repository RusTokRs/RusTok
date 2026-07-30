# rustok-search implementation plan

## Current state

`rustok-search` owns normalized search documents, PostgreSQL FTS, catalog and Blog
projection ingestion, analytics, dictionaries, query rules, rebuild/diagnostics,
and module-owned admin/storefront surfaces. It remains separate from
`rustok-index`; consumers depend on published Search contracts rather than index
runtime types. The FFA split is `phase_b_ready` with focused core, transport, and
UI packages.

Canonical result navigation has a single owner policy:
`canonical_search_result_url` in `crates/rustok-search/src/engine.rs`. It derives
product, content, and Blog URLs from normalized `SearchResultItem` values before
transport serialization. Blog navigation requires the canonical
`source_module=blog` / `entity_type=blog_post` pair and a bounded ASCII slug from
the owner-projected payload. Missing, malformed, spoofed, traversal, whitespace,
and overlong values fail closed. Content source-module values are bounded before
they enter a query string.

GraphQL Search, storefront native Search, Search admin preview, and admin global
search all delegate to this single owner policy. The storefront transport facade
returns the selected transport payload unchanged: there is no transport fallback,
no post-processing navigation module, and no local Blog route builder. The Search
admin native adapter is split into focused include-parts for API facade, read
handlers, write handlers, normalization, execution pipeline, mapping, and support.
Only the mapping part converts normalized results to admin DTOs, and it delegates
URL resolution to `canonical_search_result_url`.

Blog ingestion has two executable, unrun harness layers. A routing target locks
Blog lifecycle, module-toggle, and targeted/full reindex events. An env-gated
PostgreSQL target creates an isolated schema, runs Search migrations, projects
real Blog source rows through `SearchIngestionHandler`, verifies lifecycle and
payload replacement, checks tenant-scoped full rebuild, targeted missing-post
cleanup, and module-disabled cleanup followed by enable-time rebuild. Source-table
availability resolves through the active PostgreSQL `search_path` instead of
hard-coding `public`.

The generic platform-settings projection is now secret-safe: `search.api_key` is
bootstrap-only, tenant writes containing it fail closed, and public read/update
responses expose only `api_key_configured`. Historical rows remain inaccessible
through the API but require an append-only migration owned by the platform settings
table owner for physical at-rest removal.

Search ingestion is not yet durably terminal. `OutboxRelay` may mark a remote or
local transport publish successful before the in-process module handler succeeds.
The module event dispatcher retries a handler four times, then records an error and
discards the result; broadcast lag similarly skips deliveries. The admin rebuild
endpoint publishes `ReindexRequested` through the same best-effort local listener
path, so it is not an independent durable repair mechanism. Search remains blocked
until handler completion is backed by a durable per-consumer inbox/job/DLQ or an
owner-local repair command that does not depend on the same lossy path, with retained
recovery evidence.

## FFA/FBA status

- FFA status: `phase_b_ready`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Provider contracts: `SearchQueryPort` and `SearchSuggestionPort` in
  `crates/rustok-search/contracts/search-fba-registry.json`.
- Static provider evidence:
  `crates/rustok-search/contracts/evidence/search-contract-test-static-matrix.json`.
- Executable provider fallback evidence:
  `crates/rustok-search/contracts/evidence/search-runtime-fallback-smoke.json`.
- Executable provider contract evidence:
  `crates/rustok-search/contracts/evidence/search-runtime-contract-smoke.json`.
- Provider invocation evidence:
  `crates/rustok-search/contracts/evidence/search-runtime-invocation-trace.json`.
- Canonical URL status: `source_verified_no_compile`.
- Canonical URL evidence:
  `crates/rustok-search/contracts/evidence/search-canonical-url-contract.json`.
- Canonical URL guardrail:
  `scripts/verify/verify-search-canonical-url-contract.mjs`.
- Secret-projection guardrail:
  `scripts/verify/verify-search-settings-secret-projection.mjs`, executed by the
  standard Search FBA verifier.
- Blog projection evidence:
  `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`.
- Blog projection harness status: `executable_no_run`; execution remains user-owned
  and requires `RUSTOK_SEARCH_TEST_DATABASE_URL` or PostgreSQL `DATABASE_URL`.
- GraphQL and all native/admin mappings use the same Search-owned URL function.
- The removed storefront `transport/navigation.rs` path is forbidden by the
  canonical URL guardrail.
- Transport-local `derive_search_result_url`, `derive_admin_search_result_url`,
  `enrich_search_result_urls`, and Blog route constants are forbidden.
- Blog projection table discovery and reads use the active `search_path`.
- `TenantModuleToggled(blog, false)` deletes only the current tenant Blog search
  scope; enabling the module rebuilds it from retained owner rows.
- Targeted Blog reindex deletes stale documents before source lookup, so a missing
  owner post cannot leave obsolete search data behind.

## Deployment and connector boundary

Search is a whole-module extraction pilot. Remote deployment contains the complete
`rustok-search` owner, including `SearchEngine`, ranking, dictionaries, query
rules, analytics, URL policy, and PostgreSQL baseline. Storefront and admin
consumers call normalized Search contracts and never construct application routes.
The extraction boundary follows
[Media and Search Extraction Boundaries](../../../DECISIONS/2026-07-16-media-search-extraction-boundaries.md).

Meilisearch, Typesense, and Algolia remain connector implementations inside the
Search service. They receive canonical `SearchQuery` and document inputs and
return normalized `SearchResult` and suggestion DTOs. Connector results must pass
through Search-owned mapping before transport serialization. Connector credentials
are host-bootstrap secrets and must never cross the generic tenant settings API.

`rustok-index` remains a separate ingestion/read-model owner. Query-time reads of
index-owned category and attribute tables should move to Search-owned denormalized
fields before database isolation. Search continues to own event ingestion and
rebuild behavior, but current in-process handler delivery is not a sufficient durable
replay contract.

## Completed implementation slices

1. Added Blog lifecycle Search projection, targeted/full reindex, module-toggle
   handling, stale cleanup, and tenant isolation.
2. Added isolated-schema PostgreSQL Blog projection harnesses and active
   `search_path` discovery.
3. Added `canonical_search_result_url` with product, content, and Blog routing,
   bounded slug validation, source/entity ownership, and query-injection guards.
4. Exported the owner policy and migrated GraphQL result projection.
5. Migrated storefront native result projection to the owner policy.
6. Removed storefront post-transport navigation enrichment and deleted its source
   and focused verifier fixtures.
7. Migrated Search admin preview mapping to the owner policy.
8. Migrated admin global search to the owner policy and admitted canonical
   `blog_post` results through the Blog read permission.
9. Split the Search admin native adapter into focused source parts while preserving
   its public transport API.
10. Added current-only machine-readable evidence and negative fixtures that reject
    every transport-local URL implementation and require no transport fallback.
11. Added canonical URL ownership to the standard Search FBA gate alongside the
    provider port, fallback, runtime contract, and invocation evidence.
12. Removed `search.api_key` from generic settings projections, rejected new tenant
    storage of the key, exposed only `api_key_configured`, and added a standard FBA
    secret-projection guard.

## Next results

1. **Make Search delivery durably terminal.** Add a consumer-owned durable inbox/job
   with idempotent completion, retry, DLQ, lag, restart, and replay semantics, or an
   equivalent durable consumer transport. Outbox terminal state must not imply Search
   projection completion until the consumer result is durable.
2. **Add an independent owner repair entrypoint.** Rebuild/reconcile commands must call
   Search owner services directly or enqueue a durable Search job; they must not publish
   `ReindexRequested` into the same lossy local listener path they are intended to repair.
3. **Scrub historical credentials through the correct owner.** Add an irreversible,
   append-only platform migration that removes `api_key` and the computed marker from
   existing `platform_settings` rows where `category = 'search'`.
4. **Persist ordering authority for destructive events.** Add source revision or an
   authoritative re-read rule for delete/restore so delayed deletes cannot erase a
   newer projection.
5. **Execute canonical URL evidence.** Run core URL-policy tests, GraphQL storefront
   Search, native storefront Search, Search admin preview, and admin global search
   against projected product, content, and Blog documents.
6. **Execute live Blog projection evidence.** Retain migration/`pg_trgm`, event-delivery,
   targeted missing-post cleanup, module-disable cleanup, category reindex, and failure
   recovery results.
7. **Execute live provider evidence.** Run query and suggestion providers under deadline,
   error, locale, tenant, channel, ranking, and catalog-filter conditions.
8. **Add external engines only as adapters.** Meilisearch, Typesense, or Algolia
   connectors must not bypass Search ports, owner URL mapping, PostgreSQL baseline
   selection, or the bootstrap credential boundary.

## Verification

- `cargo test -p rustok-search engine::tests::canonical_url`
- `node scripts/verify/verify-search-canonical-url-contract.mjs`
- `node scripts/verify/verify-search-canonical-url-contract.test.mjs`
- `node scripts/verify/verify-search-settings-secret-projection.mjs`
- `cargo test -p rustok-server settings_service --lib -- --nocapture`
- `cargo test -p rustok-search --test blog_ingestion_contract_test`
- `RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-search --test blog_projection_postgres_test`
- `node scripts/verify/verify-search-blog-projection.mjs`
- `node scripts/verify/verify-search-blog-projection.test.mjs`
- `npm run verify:search:fba`
- `npm run verify:search:ui-boundary`
- `cargo xtask module validate search`

## References

- [Crate README](../README.md)
- [Search documentation](./README.md)
- [Search FBA registry](../contracts/search-fba-registry.json)

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-30`
- Scope inspected: `connector-secret projection, generic settings GraphQL read/write, Search ingestion/projectors, tenant/locale/channel scope, module event registration, outbox/local fan-out, dispatcher retry/lag behavior and admin rebuild entrypoint`
- Findings: `P0=0, P1=2, P2=2, P3=0`
- Fixed in this pass: `removed search.api_key from public generic settings projections; rejected new tenant storage of the bootstrap key; made api_key_configured read-only; added service regressions and an FBA-integrated static guard`
- Remaining risks or blockers: `P1: remote/local outbox publication can become terminal before Search handler success, while final handler errors and broadcast lag are discarded; ReindexRequested uses the same non-durable path. P2: historical platform_settings search rows still require a platform-owner irreversible scrub migration. P2: destructive delete events have no persisted source revision, so out-of-order delete/restore safety is not proven. Draft PR #2512 is not mergeable with current main and same-SHA Search FBA/CI remain queued.`
- Evidence: `source inspection of EventDispatcher, EventRuntime local fan-out, SearchIngestionHandler, SearchProjector and native rebuild transport; Index Scale Run Contract and Index Scale Evidence passed on head 30c003b; Search hardening/CI were still queued; local targeted execution was unavailable because github.com DNS/direct HTTPS failed`
- Next action: `move to core/outbox and determine the owner-correct durable consumer completion model; later add the platform settings scrub migration and retained Search replay/recovery evidence`
- Resume command: `node scripts/verify/verify-search-settings-secret-projection.mjs && npm run verify:search:fba && cargo test -p rustok-server settings_service --lib -- --nocapture && cargo test -p rustok-search --test blog_ingestion_contract_test -- --nocapture`
