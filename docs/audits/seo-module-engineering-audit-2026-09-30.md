# SEO module — engineering audit

**Date:** 2026-09-30  
**Scope:** `rustok-seo`, `rustok-seo-targets`, migrations/entities, GraphQL/REST, workers/events/index delivery, owner integrations, Leptos storefront, Next storefront/runtime/admin.  
**Method:** source-level tracing of request paths, provider contracts, persistence, queue state transitions, renderers, transport fallbacks, and static verification files. This deliverable records findings; it does not silently implement fixes.

## Executive conclusion

The SEO module has a sound high-level decomposition: tenant-aware services, a target-provider registry, explicit capability flags, transactional events, durable job tables, and read-only public delivery. The implementation is not yet contract-complete, however. Important fields and operations are present in DTOs and API shapes but are not connected to render/runtime behavior; several “bounded” paths bound only the inner worker chunk while materializing unbounded tenant-sized data earlier; and the two storefront runtimes do not provide parity.

The highest-risk confirmed issue is a redirect validation bypass. A protocol-relative target such as `//attacker.example/path` is classified as a relative route, stored, and can be returned in the storefront `Location` header when target resolution does not find an internal page. Browsers interpret that value as a network-path reference and navigate to the attacker-controlled origin. The same shared validator is used for canonical input, so the URL trust boundary is also unsafe for future/current consumers that pass the raw value through.

No Rust test result is claimed: this checkout has no `cargo` binary (`cargo: command not found`).

## Severity summary

| ID | Severity | Area | Finding |
|---|---|---|---|
| SEO-REDIR-001 | **P1 / security** | Redirects | `//host/path` bypasses the absolute-host allowlist and can become an external `Location`. If SEO update users are strictly fully trusted internal operators, classify as P2; otherwise treat as release-blocking. |
| SEO-BULK-001 | P2 | Bulk export | Enqueueing resolves the complete selection and stores every target ID in a JSON job payload. |
| SEO-BULK-002 | P2 | Bulk read/provider contract | The provider returns a complete `Vec`; bulk listing materializes all rows, then paginates in memory. |
| SEO-BULK-003 | P2 | Bulk import | The CSV is scanned and the complete input string is retained in the job payload; there is no byte/row/payload limit at the queue boundary. |
| SEO-SITEMAP-001 | P2 | Sitemaps | All provider candidates are collected, globally sorted/deduplicated, and only then split into fixed 500-entry files. |
| SEO-SITEMAP-002 | P2 | Persistence/lifecycle | Sitemap jobs/files and bulk jobs/artifacts have no retention/deletion policy; repeated runs grow database storage. |
| SEO-OPS-001 | P2 | Operational reads | Job list methods fetch all historical rows before applying `limit`; artifact metadata reads still query complete artifact records. |
| SEO-REDIR-002 | P2 | Redirect semantics | Wildcard precedence depends on source/database order instead of an explicit priority contract. |
| SEO-REDIR-003 | P2 | Canonical/redirect semantics | Canonical route resolution runs before manual redirect lookup; the precedence is implicit and cannot be configured or overridden by a redirect. |
| SEO-URL-001 | P2 | URL normalization | Values are trimmed for validation but the original string is persisted; relative URL validators do not reject protocol-relative forms. |
| SEO-SET-001 | P2 | Settings | Multiple persisted settings are normalized but have no runtime consumer: custom robots/disallow/crawl delay, canonical slash mode, redirect TTL, sitemap chunk/changefreq, metadata limits, branding/social defaults, and related fields. |
| SEO-LIFE-001 | P2 | Worker lifecycle | One `seo_bulk_enabled` switch controls a poller that also runs sitemap and index-repair jobs; disabling “bulk” leaves unrelated queues unprocessed. |
| SEO-SITEMAP-003 | P2 | Public delivery | `/sitemap.xml` checks `sitemap_enabled`; `/sitemaps/{name}` serves a retained child file without the same check. |
| SEO-CONTRACT-001 | P2 | Provider architecture | Bulk/sitemap provider contracts have no cursor, limit, streaming, or server-side selection primitive. |
| SEO-QUEUE-001 | P2 | Index repair queue | Check-then-insert deduplication is not atomic; worker running-job detection is global rather than tenant-isolated. |
| SEO-RUNTIME-001 | P2 | Next storefront | Next declares redirect data but never applies `redirect()` and currently resolves only the hard-coded home route. Leptos applies redirects, so runtime behavior differs. |
| SEO-RUNTIME-002 | P2 | Next metadata | Backend canonical paths can already contain locale prefixes; `localizedPath()` unconditionally prefixes them again in fallback canonical/hreflang generation. |
| SEO-RUNTIME-003 | P2 | Next sitemap | Next rejects sitemap child URLs whose origin differs from the API origin, while backend generation uses the configured public origin. Multi-origin deployments fall back to static output. |
| SEO-RUNTIME-004 | P2 | Leptos locale parity | The native server adapter reads `tenant.settings["default_locale"]` instead of the tenant’s `default_locale` field. |
| SEO-TENANT-001 | P2 | Persistence | Core SEO tables store `tenant_id` but lack tenant foreign keys/cascades; only some child-to-job relations are enforced. |
| SEO-TARGET-001 | P2 | Product/category ownership | Product category SEO persistence and translation ownership exist, but the generic SEO registry exposes only `product`; category pages therefore have no provider route/bulk/sitemap integration. |
| SEO-ADMIN-001 | P2 | Admin | Next’s central SEO page is index repair/replay only. Owner Leptos panels exist for product/blog/forum entities, but there is no confirmed central management surface for redirects/settings/sitemaps/bulk/diagnostics; the ownership decision requires explicit owner-by-owner coverage. |

## Detailed findings

### 1. Redirect and URL trust boundary

**Evidence**

- `crates/modules/rustok-seo/src/services/redirects.rs:572-608` sends every value beginning with `/` through `normalize_route`. `normalize_route` only requires one leading slash and rejects whitespace; it does not reject a second leading slash.
- `upsert_redirect` validates `input.target_url` at `redirects.rs:237-243`, then persists the original untrimmed value at `redirects.rs:260-272`.
- `routing.rs:80-132` tries to resolve an internal redirect target, then returns the raw target in the fallback `SeoRedirectDecision` when no target context exists.
- `apps/storefront/src/lib.rs:413-420` and `527-535` pass that decision directly to `redirect_response`, including the configured status code.
- `meta.rs:140-147` reuses the same validator for `canonical_url`.

**Exploit path**

1. A caller with the tenant’s SEO update permission submits a redirect target `//attacker.example/path`.
2. `validate_target_url` treats it as a relative route because it starts with `/`; the absolute-host allowlist is never consulted.
3. The raw value is stored.
4. If no internal SEO target resolves for that value, storefront response construction emits `Location: //attacker.example/path`.
5. A browser resolves the header relative to the current scheme as an external network-path URL.

The fix must be centralized, not added only to one GraphQL resolver: reject `//` after trimming in every relative URL validator; normalize before persistence; validate canonical/template/bulk paths through the same function; and add REST, GraphQL, Leptos, and Next regression tests. The test suite currently checks HTTP(S), credentials, and schemes but does not test protocol-relative targets (`redirects.rs:720-744`).

`normalize_source_pattern` has a separate defect at `redirects.rs:610-627`: wildcard sources are trimmed and checked for `*` count but do not pass through whitespace-free route validation. Invalid wildcard patterns and protocol-relative source forms can therefore be stored even though ordinary exact routes are stricter.

### 2. Redirect precedence and cache semantics

`RedirectLookup::from_source` indexes duplicate exact/wildcard entries by first source index (`redirects.rs:64-89`). The source list is loaded with `.all()` and no ordering (`redirects.rs:409-419`), so duplicate/colliding wildcard behavior depends on database return order. Prefix/suffix lengths are sorted, but the selected redirect record for the same pattern is not assigned an explicit priority. The data model has no priority field. Add a documented precedence tuple and enforce it in SQL/indexing and tests (for example exact > longest prefix > longest suffix > explicit priority > stable ID).

`routing.rs:54-78` resolves content canonical routes before checking manual redirects at `:80`. This may be the intended invariant, but it is an implicit policy: a redirect that should supersede a stale canonical owner cannot do so. The ordering should be an explicit contract with tests for canonical-vs-manual redirects, loops, expiry, and disabled targets.

`redirect_cache_ttl_seconds` is clamped in `services_base.rs:264-268`, but the process cache uses compile-time `REDIRECT_CACHE_TTL_SECS` in `redirects.rs:34-39`. The persisted setting is therefore write-only. Cache invalidation is transactionally recorded and the durable cursor reconciliation is a good design, but the configured TTL must either be applied per tenant/cache instance or removed from the public settings contract.

### 3. Persisted settings are not an effective runtime contract

`services_base.rs:255-337` accepts and normalizes a substantial settings object. Targeted usage search shows the following settings occur only in DTO/default/normalization code, with no renderer/runtime consumer:

- `robots_txt_custom_content`, `disallow_paths`, `crawl_delay`;
- `canonical_trailing_slash_mode`;
- `sitemap_max_entries_per_file`, `sitemap_changefreq`;
- `meta_title_max_length`, `meta_description_max_length`, `title_suffix` and related default branding/social fields;
- `redirect_cache_ttl_seconds`.

The mismatch is visible in the actual renderers:

- `sitemaps.rs:473-496` calls `render_robots_body(base_url, sitemap_enabled)`; the body only emits `User-agent`, `Allow`, and optionally `Sitemap` (`:783-788`). Custom content, disallow paths, and crawl delay are ignored.
- `sitemaps.rs:573-610` chunks with fixed `SITEMAP_CHUNK_SIZE`, not `settings.sitemap_max_entries_per_file`; change frequency is not rendered.
- Redirect cache TTL is fixed as described above.
- Metadata/title limits and organization/Twitter defaults are normalized but do not flow into the generated document path.

This is a compatibility problem, not just a missing UI: an operator can receive a successful settings write and a later read showing the normalized value while public output remains unchanged. Either implement each field end-to-end and test it at the public endpoint, or remove/deprecate it from the persisted API and migration contract.

### 4. Resource bounding is incomplete despite bounded worker chunks

The worker has useful inner bounds (`BULK_IO_CHUNK_SIZE`, metadata batches, and per-operation limits), but the expensive work happens before those bounds:

- `bulk_io_bounded_execution.rs:35-87` calls `collect_bulk_rows_for_filter`, builds a complete `target_ids` vector, and serializes it into `input_payload` before the job is inserted.
- `bulk_io_bounded_execution.rs:89-146` scans the entire CSV and stores `input.csv_utf8` in the queued payload. The older compatibility path has the same full parse behavior at `bulk_legacy.rs:1044-1075`.
- `bulk_read_model.rs:54-138` asks providers for all summaries, loads all matching metadata in batches, then `list_bulk_items_batched` paginates the final in-memory vector at `:141-188`.
- The product provider itself loops through all published products and returns all summaries/candidates (`rustok-product/src/seo_targets.rs:97-184`).

Consequences include request-time latency and memory spikes, oversized JSON rows, large transaction/log amplification, and easy resource exhaustion by repeatedly submitting broad authorized operations. A real bound must exist at the ingress (maximum CSV bytes/rows and selection cardinality), in the database/provider query (cursor/limit), in the payload (filter plus cursor rather than all IDs), and in the artifact/storage layer. “Process 50 rows per worker tick” is not sufficient.

The same architecture affects sitemap generation: `sitemaps.rs:615-655` and `sitemap_background.rs:248-286` collect all provider candidates, sort and deduplicate the entire URL set, and only then persist chunks. `sitemaps.rs:573-604` uses fixed 500-entry files and materializes all file records/index URLs. A cursor-based provider contract or a bounded external/object-store writer is needed for large catalogs.

### 5. Unbounded operational reads and missing retention

- `sitemaps.rs:431-450` loads all jobs for the tenant, then applies `take(limit)` in Rust.
- `bulk_legacy.rs:1095-1115` does the same for bulk jobs.
- `bulk_legacy.rs:1907-1934` loads complete artifact rows (including text content in the entity query) when building metadata maps.
- Sitemap files and bulk artifacts store text in database columns (`m20260419_000003_create_seo_tables.rs:174-187`; `m20260420_000004_create_seo_bulk_tables.rs:180-235`) with no size/retention policy in the migrations.

The GraphQL limits protect the response size but not the database query or object materialization. Apply SQL `LIMIT`/cursor predicates before `.all()`, select metadata columns separately from content, and expose artifact content only from an explicitly bounded download path.

There is no lifecycle job or documented TTL for completed/failed sitemap jobs, files, bulk jobs, artifacts, diagnostics, or event delivery history. Add tenant-aware retention, safe deletion ordering, and metrics/alerts for storage growth. Retention must preserve whatever audit/history period the product promises.

### 6. Worker lifecycle and queue isolation

The server starts one worker based on `settings.runtime.background_workers.seo_bulk_enabled` (`apps/server/src/services/app_lifecycle.rs:177-231`). That loop calls `execute_next_bulk_job`, whose bounded compatibility implementation advances bulk, sitemap, and index-repair queues in the same poll (`crates/modules/rustok-seo/src/services/bulk_io_bounded_compat.rs:2-31`). Consequently, a setting named for bulk is also the liveness switch for sitemap and index repair. Split switches/handles or rename the aggregate setting and make the operational dependency explicit.

Index repair queueing has a race:

- `index_repair_background.rs:54-65` checks for an active job;
- `:67-86` inserts a new job in a separate operation;
- concurrent requests can both observe no active row and insert duplicates.

The migration has tenant/status/time indexes but no uniqueness invariant for one active job per tenant/target scope. Use a database partial unique index where supported, or an atomic insert/upsert/transactional claim design.

The worker’s active-running query at `index_repair_background.rs:98-102` has no tenant filter. A fresh running job for any tenant causes the global worker to return without processing another tenant’s queued job. Sitemap and bulk workers also serialize the queue globally, so one large tenant can create cross-tenant latency. If global serialization is intentional, document and monitor it; otherwise claim work with tenant-aware fairness and an atomic state transition.

Authorization separation is positive: worker entry points require an opaque `SeoWorkerAuthorization`, and application/operator calls are distinct. That proof must remain tied to the individual queue switch after the lifecycle split.

### 7. Public sitemap delivery is inconsistent

The index controller checks the setting before serving (`controllers/mod.rs:219-247`). The child-file controller at `controllers/mod.rs:250-265` calls `sitemap_file` directly and does not repeat the setting check. Since old files are retained, disabling sitemap can still leave child URLs available if their names are known. Decide whether disable means “all files disappear immediately”; if so, enforce the check on every public file endpoint and invalidate/remove old output according to retention policy.

The generated index links use the configured public origin (`sitemaps.rs:592-604`). This is correct for deployments where the API and storefront have different origins, but the Next runtime rejects any URL whose origin differs from `NEXT_PUBLIC_API_URL` (`apps/next-frontend/src/shared/seo/runtime.ts:355-364`, `:390-416`). The runtime then uses static fallback. Configure an explicit trusted public-origin allowlist or fetch child documents through a server-side proxy; do not equate “same API origin” with “safe sitemap origin.”

### 8. Storefront runtime parity is incomplete

Leptos applies a resolved redirect to an HTTP response (`apps/storefront/src/lib.rs:413-420`, `:527-535`) and supports the generic SEO page context route. Next’s only dynamic page is `apps/next-frontend/src/app/[locale]/page.tsx`: it resolves the hard-coded `HOME_ROUTE = "/"` (`:19-25`, `:60`) and uses the context only for metadata/structured data. It never checks `seoResolution.context.route.redirect` and never calls Next’s `redirect()`. Thus a redirect can work on the native storefront and render a normal page or stale metadata in Next.

Canonical/hreflang fallback has a locale duplication risk. `metadata.ts:185-201` derives `canonicalPath` and calls `localizedPath(item, canonicalPath)` for every locale. The backend route context can already return a locale-prefixed canonical path. `localizedPath` always prepends (`site.ts:13-22`), yielding paths such as `/en/en/catalog`.

The native adapter’s locale source is also inconsistent: `apps/storefront/src/shared/context/seo_page_context_native_server_adapter.rs:62-77` reads a setting key instead of the tenant field used by the SEO service and Next transport. This can make native fallback locale selection differ from backend routing.

Define one normalized route model (origin, locale, path, query, and redirect status) and make both runtimes consume it. Add parity tests for exact/wildcard redirects, 301/302/307/308, canonical redirects, unknown routes, locale fallback, and cross-origin sitemap deployments.

### 9. Persistence isolation and referential integrity

`m20260419_000003_create_seo_tables.rs:20-35`, `:86-93`, `:134-148`, and `:174-187` create core tables with `tenant_id` columns but no foreign key to the tenant table. Child sitemap files do have a foreign key to their job (`:198-203`), but the job itself is not tenant-constrained at the database level. Bulk tables similarly enforce job-to-item/artifact relationships but do not enforce tenant ownership (`m20260420_000004_create_seo_bulk_tables.rs:20-101`, `:138-162`, `:191-231`).

Application filters generally include tenant IDs, which is necessary but not a substitute for database integrity. Tenant deletion can leave orphaned rows; an ID reuse/import mistake can associate an SEO row with a non-existent tenant; and cascades cannot guarantee cleanup. Add tenant FKs/cascades or a documented shared-schema ownership mechanism, then add migration tests for tenant deletion and cross-tenant mismatch rejection.

### 10. Product category SEO is persisted but not in the generic SEO runtime contract

The product module has deliberate category SEO storage and translation ownership: `m20260829_000017_add_product_category_seo_translations.rs:18-32` creates `catalog_category_seo_translations` with a tenant/category composite FK and cascade, and `seo_translation.rs` exposes a separate `category_seo` translation target. This is good ownership separation.

The generic runtime registry, however, has only `builtin_slug::PRODUCT` for product catalog SEO (`rustok-seo-targets/src/lib.rs:368-383`), and `ProductSeoTargetProvider` returns only `builtin_slug::PRODUCT` (`rustok-product/src/seo_targets.rs:20-38`). Product module registration installs only that provider (`rustok-product/src/lib.rs:144-155`). No category provider/route resolver/bulk provider/sitemap candidate integration was found.

Therefore category SEO values can be translated and persisted without being discoverable by generic page context, redirects/canonical resolution, bulk SEO operations, or sitemaps. This is a P2 contract gap unless category pages are explicitly out of scope. The product owner must either register a category target with routing/sitemap/bulk capabilities or document that category SEO is consumed only by a separate catalog renderer and prove that renderer’s metadata path.

### 11. Admin surface and ownership

The central Next admin page describes itself as “SEO index tracking, repair, and replay” (`apps/next-admin/src/app/dashboard/seo/page.tsx:18-25`). Its panel contains repair-only and repair-plus-historical-replay actions, not settings, redirect, sitemap generation/history, bulk, or diagnostics forms (`apps/next-admin/src/features/seo/components/seo-operator-panel.tsx`). The Next API client has some read paths for sitemap/bulk jobs, but that does not constitute a management UI.

The architecture decision on SEO UI ownership assigns entity authoring to content modules. The shared Leptos support panel is mounted by product/blog/forum owners, so absence from the central Next page is not by itself a defect. The remaining gap is contract verification: each owner must expose all intended SEO fields and operations, including category-specific ownership, while the central operator page should expose cross-module operations that are intentionally central. Document the matrix rather than relying on file discovery.

## What is working / not classified as a finding

- Tenant context and permission gates exist on the main GraphQL/application service paths; public delivery is separated from operator generation.
- Redirect changes and sitemap generation publish transactional domain/outbox events, and redirect cache reconciliation uses a durable cursor rather than relying only on process memory.
- Sitemap submission URLs have dedicated scheme/host/userinfo/internal-network validation; this is separate from the redirect/canonical relative-path bug and should be preserved.
- The provider registry/capability boundary is preferable to hard-coding every content module into the SEO crate.
- Leptos storefront redirect handling and owner-module admin support demonstrate the intended contract; the issue is parity/completeness at the other boundaries.

## Remediation order

1. **Block the URL bypass:** reject protocol-relative relative values, normalize before persistence, review canonical/template/bulk validators, and add regression tests through REST/GraphQL and both storefronts.
2. **Make public behavior match settings:** implement or remove every normalized setting; especially robots output, sitemap chunk/changefreq, canonical slash mode, title limits, and cache TTL.
3. **Redesign resource boundaries:** add provider cursor/limit APIs, queue filters/cursors rather than all IDs, enforce CSV byte/row limits, SQL-limit job reads, separate artifact metadata/content, and add retention.
4. **Fix worker isolation/liveness:** split queue switches, atomically deduplicate jobs, claim fairly per tenant, and add concurrency tests.
5. **Close runtime parity:** apply Next redirects, normalize locale prefixes, and support configured public sitemap origins safely.
6. **Repair persistence contracts:** add tenant FK/cascade policy and tests; define the product-category SEO target contract with the owning module.
7. **Publish an ownership matrix:** central operator vs owner-module UI, with each capability and its runtime consumer named.

## Verification limitations

- `cargo test -p rustok-seo --lib` could not run because `cargo` is absent in the environment. Static source tracing and existing test definitions were inspected, but tests are not evidence of passing behavior.
- The Next SEO fixture verifier is stale in this checkout: it references `load_delivery_by_idempotency_key`, which is not present in the current event service. Its failure should not be used as a product defect without updating the fixture contract.
- A static verifier that reads only `services/mod.rs` can miss providers included from adjacent modules; provider-absence claims above were checked against the actual product provider and registration code.
