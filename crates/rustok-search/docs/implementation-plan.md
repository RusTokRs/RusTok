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

Search settings have one owner boundary. Tenant-effective settings are read and
written through `SearchSettingsService` and the `search_settings` table. The
server-wide generic `platform_settings` service no longer admits a `search`
category, does not serialize bootstrap `search.api_key`, filters historical generic
Search rows from list responses, and rejects generic Search reads and writes before
database access.

Periodic release verification found two unresolved runtime boundaries. Public
GraphQL and native storefront Search accept `channel_id` from caller input rather
than deriving it from trusted `RequestContext`, while `PgSearchEngine` applies the
channel only to attribute filters, facets, and sorting—not to the ranked product
result set. Product search documents also omit the canonical
`metadata.channel_visibility.allowed_channel_slugs` projection, so an active
product can remain searchable in a channel where the Commerce storefront would
hide it.

The durable projection contract is also incomplete. The generic
`search_projection_inbox` and watermark schema exists, but only Forum events use
it and its background reconciler. Content, Product, Blog, locale, tenant, and
ordinary reindex work can still run directly through the process-local event
handler. Terminal handler failure or broadcast lag is logged but has no durable
Search consumer receipt, automatic retry lane, or guaranteed source rebuild, so a
projection can remain stale after recovery.

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
- Blog projection evidence:
  `crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json`.
- Blog projection harness status: `executable_no_run`; execution remains user-owned
  and requires `RUSTOK_SEARCH_TEST_DATABASE_URL` or PostgreSQL `DATABASE_URL`.
- GraphQL and all native/admin mappings use the same Search-owned URL function.
- The removed storefront `transport/navigation.rs` path is forbidden by the
  canonical URL guardrail.
- Transport-local `derive_search_result_url`, `derive_admin_search_result_url`,
  `enrich_search_result_urls`, and Blog route constants are forbidden.
- Generic `platform_settings/search` is forbidden by the Search FBA guard.
- Blog projection table discovery and reads use the active `search_path`.
- `TenantModuleToggled(blog, false)` deletes only the current tenant Blog search
  scope; enabling the module rebuilds it from retained owner rows.
- Targeted Blog reindex deletes stale documents before source lookup, so a missing
  owner post cannot leave obsolete search data behind.
- Storefront channel authority and product visibility remain `blocked`.
- Durable non-Forum projection replay/recovery remains `blocked`.

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
through Search-owned mapping before transport serialization.

`rustok-index` remains a separate ingestion/read-model owner. Query-time reads of
index-owned category and attribute tables should move to Search-owned denormalized
fields before database isolation. Search continues to own event ingestion and
rebuild behavior through replayable event transport.

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
12. Removed the legacy generic `platform_settings/search` execution path and added
    a fail-closed FBA ownership guard so runtime connector secrets stay inside the
    Search owner boundary.

## Next results

1. **Close storefront channel authority and visibility.** Derive channel identity
   from trusted `RequestContext` for GraphQL and native storefront surfaces,
   denormalize canonical product channel visibility into Search-owned documents,
   backfill existing documents safely, and make base results, totals, facets,
   typo fallback, and attribute operations use one fail-closed channel predicate.
   **Done when:** caller-supplied channel IDs cannot select another channel and a
   restricted product is absent from every Search response outside its allowed
   channel.
2. **Generalize durable Search projection recovery.** Use the existing generic
   inbox/watermark schema for Content, Product, Blog, locale, tenant, and reindex
   events; add bounded retry, dead-letter diagnostics, ordered replay, restart and
   lag recovery, and source-of-truth rebuild evidence.
   **Done when:** terminal handler failure, transport lag, duplicate/out-of-order
   delivery, and process restart cannot leave a stale projection without an
   observable durable recovery action.
3. **Execute canonical URL evidence.** Run core URL-policy tests, GraphQL
   storefront Search, native storefront Search, Search admin preview, and admin
   global search against projected product, content, and Blog documents. Retain
   proof that malformed Blog payloads remain non-navigable everywhere.
4. **Verify click analytics.** Confirm every Search surface records the canonical
   href without reconstructing routes in analytics code.
5. **Execute live Blog projection evidence.** Run routing and PostgreSQL harnesses
   and retain migration/`pg_trgm`, event-delivery, targeted missing-post cleanup,
   module-disable cleanup, and category reindex results.
6. **Execute live provider evidence.** Run query and suggestion providers under
   deadline, error, locale, tenant, channel, ranking, and catalog-filter conditions.
7. **Add external engines only as adapters.** Meilisearch, Typesense, or Algolia
   connectors must not bypass Search ports, owner URL mapping, or PostgreSQL
   baseline selection.

## Verification

- `cargo test -p rustok-server settings_service`
- `cargo test -p rustok-search engine::tests::canonical_url`
- `node scripts/verify/verify-search-canonical-url-contract.mjs`
- `node scripts/verify/verify-search-canonical-url-contract.test.mjs`
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
- Scope inspected: `Search settings ownership, GraphQL/native storefront trust, PostgreSQL query channel semantics, product visibility projection, event dispatch, durable inbox/reconciliation, projection transactions, locale/delete/rebuild paths, connector and UI boundaries`
- Findings: `P0=0, P1=3, P2=0, P3=1`
- Fixed in this pass: `merged PR #2557 / commit 2a40ffc372449d6729b7d86fd6135b49555f7e9a, removing the non-authoritative generic platform_settings/search secret-bearing path and adding an owner-boundary guard`
- Remaining risks or blockers: `P1: public GraphQL and native storefront Search trust caller channel_id and PgSearchEngine does not apply channel visibility to the base product result set; P1: non-Forum Search projections lack a durable consumer receipt, retry/DLQ lane and guaranteed replay/rebuild after terminal handler failure or broadcast lag; P3: projector_legacy.rs remains a production implementation behind a compatibility-named facade and should be replaced rather than wrapped`
- Evidence: `source audit confirms RequestContext owns trusted channel id/slug, storefront Search instead passes normalized caller input, base FTS/typo CTEs filter only tenant/locale/query, and canonical Commerce visibility uses metadata.channel_visibility.allowed_channel_slugs; search_projection_inbox is generic but ForumProjectionInbox/Reconciler and its server worker are Forum-only; PR #2557 had a conflict-free three-file diff, while broad Cargo gates were blocked by unrelated repository compilation, Cargo.lock/Athanor, expired-advisory and invalid-MSRV workflow failures`
- Next action: `on a fresh branch, implement one Search-owned denormalized channel-visibility contract across projector/backfill/query and trusted storefront adapters with PostgreSQL evidence; separately generalize the existing durable projection inbox/reconciler to every Search source and retain outage/restart/replay evidence`
- Resume command: `rg "channel_id: input.channel_id" crates/rustok-search/src/graphql/query.rs crates/rustok-search/storefront/src/transport/native_server_adapter.rs && rg "query.channel_id" crates/rustok-search/src/pg_engine.rs && rg "let _ = handler.handle_with_retry|ForumProjectionInbox|ForumProjectionReconciler" crates/rustok-core/src/events/handler.rs crates/rustok-search/src apps/server/src/services`
