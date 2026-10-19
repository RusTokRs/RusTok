# SEO module — product-logic chain audit

**Date:** 2026-10-18
**Branch:** `arena/10b62f45-rustok`
**Base commit:** `d0e18e3056167beeb85065a3f913510a542dd008`
**Scope:** `rustok-seo`, `rustok-seo-targets`, owner providers (pages/product/blog/forum/taxonomy), GraphQL/REST, background workers, Leptos and Next storefronts, admin surfaces.
**Method:** end-to-end tracing of product chains through sources (write path → persistence → cache/events → render → public delivery → UI), cross-checked with `docs/audits/seo-module-engineering-audit-2026-09-30.md`.
**Verification limitation:** `cargo` is not available in this environment — all claims are static (source cross-checks); Rust tests were not executed. Durable evidence: `target/verification/seo-full-suite/summary.txt` records exit 127 (`cargo is not available on PATH`). Static verifiers were executed (see §6).

**Remediation status legend:** `FIXED` — root-cause fix applied on this branch (2026-10-18); `PARTIAL` — core fixed, remainder tracked; `NEEDS ADR` — implementation blocked on an ADR per the repository contract; `OPEN` — not yet fixed.

---

## 1. Product chain map

| # | Chain | Key paths |
|---|---|---|
| C1 | Page resolution (route → redirect → canonical → provider → merge) | `services/routing.rs` (`resolve_page_context_inner`), `services/targets.rs` |
| C2 | Metadata authorization (read/upsert/revisions/AI) | `services/meta.rs`, `graphql/mod.rs` |
| C3 | Redirects (write → cache → match → auto-create on slug change) | `services/redirects.rs` |
| C4 | Sitemap/robots (generation → background → public delivery → Next sitemap) | `services/sitemaps.rs`, `sitemap_background.rs`, `controllers/mod.rs` |
| C5 | Bulk remediation (enqueue → modes → worker → artifacts) | `services/bulk_bounded_execution.rs`, `bulk_legacy.rs` |
| C6 | Diagnostics (readiness, hreflang, redirect chains, schemas) | `services/diagnostics_batch.rs` |
| C7 | Events → index (idempotency, retry/dead-letter, replay) | `services/events.rs` |
| C8 | Translation target (change journal, receipts) | `services/translation_target.rs` |
| C9 | Settings (normalization → runtime consumption) | `services/services_base.rs` |
| C10 | Storefront parity (Leptos/Next) and admin surfaces | `apps/next-frontend`, `rustok-seo/admin`, `apps/next-admin` |
| C11 | Gating: tenant / module enabled / submodules / permissions | `graphql/mod.rs`, `controllers/mod.rs`, `authorization.rs` |

---

## 2. Status of the 2026-09-30 engineering audit findings (re-verified)

**Confirmed fixed in code:**

- **SEO-REDIR-001** — protocol-relative `//host` URLs rejected by `normalize_target_url` and `normalize_source_pattern`; the source is normalized before persistence; regression tests exist.
- **SEO-REDIR-002** — wildcard precedence pinned by longest prefix/suffix ordering plus deterministic load order `(match_type, source_pattern, id)`; `unique(tenant_id, match_type, source_pattern)` exists (`m20260419_000003`).
- **SEO-SET-001 (partial)** — consumed: `robots_txt_custom_content`, `disallow_paths`, `crawl_delay`, all `sitemap_*` knobs, `redirect_cache_ttl_seconds`, `title_suffix`/`meta_*_max_length`/`canonical_*` settings on the render path. **Write-only remainder:** see P2-14.
- **SEO-LIFE-001** — per-queue switches `seo_bulk_enabled`, `seo_sitemap_enabled`, `seo_index_repair_enabled` in `app_lifecycle.rs`.
- **SEO-QUEUE-001** — partial unique indexes "one active job per tenant" for sitemap/index-repair (`m20260930_000011`), atomic conditional-update claim for bulk, tenant-fair stale-job handling.
- **SEO-SITEMAP-003** — public child-file delivery checks service-level settings plus `ensure_public_seo_enabled` in the controller.
- **SEO-BULK-003** — CSV limits (10 MB / 100 000 rows) and `MAX_BULK_TARGETS` selection cap.
- **SEO-OPS-001** — job lists read with SQL `LIMIT`; 90-day retention (`SEO_HISTORY_RETENTION_DAYS`) for bulk/sitemap/index-repair history.
- **SEO-TENANT-001** — tenant foreign keys (`m20260930_000012`).
- **SEO-RUNTIME-002** — Next `localizedPath` no longer produces `/en/en/...`.
- **SEO-RUNTIME-001** — Next applies `redirect` from the SEO context on all 16 storefront route families through the `resolveSeoPageContextForRoute` choke point; see P2-13.
- Redirect cache: durable cursor + count reconciliation across replicas — implemented and documented in `crates/modules/rustok-seo/docs/implementation-plan.md`.

**Not confirmed / still open:** SEO-SITEMAP-001 (full candidate collection in memory, bounded by `MAX_SITEMAP_URLS=1M`), SEO-ADMIN-001 (partial), SEO-TARGET-001 (transformed into P2-12), SEO-SET-001 remainder (P2-14).

---

## 3. Findings

### P1 — blocking product defects

#### P1-1 · Public sitemap/robots depended on "all owner modules enabled" while collection works per-owner — **FIXED**

**Chain:** C4 (publication) vs C1/C6 (resolution/diagnostics).

**Evidence at base:**
- `public_sitemap_modules_enabled` returned `false` when **any** sitemap-capable provider's owner module was disabled. It gated `render_robots`, `latest_sitemap_index`, `sitemap_file`, and background submission (`sitemap_background.rs`).
- All providers declare full sitemap capabilities; the registry is global; a missing `tenant_modules` row means `enabled=false`.
- Meanwhile `collect_sitemap_urls`, `resolve_registered_route_match`, diagnostics, and cross-links all **skip** disabled owners and continue.
- `public_sitemap_requires_refresh` probed **every** provider, including disabled modules, and converted their failures into public-file failures.

**Product effect:** a tenant without e.g. the forum module lost the **entire** `sitemap.xml` and the `Sitemap:` line in `robots.txt`, although collection was ready to serve the enabled sections — one disabled module silently "turned the site off" for crawlers.

**Applied fix (this branch):**
1. Gate renamed to `any_public_sitemap_owner_enabled` with any-enabled-owner semantics (fails closed only when no capable owner exists); all five service call sites plus the background submission gate updated.
2. `public_sitemap_requires_refresh` no longer probes disabled owners: a disabled owner forces the live-rebuild path (privacy-narrowing wins over the cache) because a stored file may still contain its URLs, while enabled owners are still all evaluated per the existing fail-closed rule.
3. Documented in `crates/modules/rustok-seo/README.md`.

**Acceptance still to add:** an executable test "tenant without forum module still gets a sitemap" — blocked on Rust test execution (no `cargo` in this environment).

---

#### P1-2 · Canonical/hreflang head tags point to Next-404 module routes while the sitemap adapts to different URLs — **ADR PROPOSED (implementation blocked pending acceptance)**

**Chain:** C1 (merge canonical) → C10 (Next metadata) vs C4 (sitemap).

**ADR:** `DECISIONS/2026-10-18-seo-canonical-hreflang-route-family.md` (Decision status: `Proposed`,
Implementation status: `Not started`) — one owner-declared canonical route family per target, every
public href locale-qualified, one shared adapter for canonical/hreflang/redirect/sitemap outputs,
hosts obligated to serve their declared families. Implementation must not start before the ADR
moves to `Accepted`.

**Evidence:**
- Owner canonical families are heterogeneous:
  - blog: `/modules/blog?slug={slug}` (pinned by ADR `DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md`);
  - product: `/modules/product?handle={handle}` (`rustok-product/src/seo_targets.rs`);
  - pages: `/{locale}/modules/pages?slug={slug}`;
  - forum: `/{locale}/forum/t|c/...` — public native routes;
  - taxonomy category: `/categories/{slug}`.
- Default canonical template is empty, so the render always falls back to `locale_prefixed_path(effective, state.canonical_path)`.
- Next has **only** `app/[locale]/modules/forum/`; no `/modules/blog|product|pages` routes; no rewrites for them.
- Head tags: `buildSeoMetadata` publishes `context.route.canonicalUrl` as-is; `buildAlternates` publishes `context.route.alternates` as-is.
- Sitemap: `adaptRuntimeSitemapUrl` **translates** `/modules/...` into public paths — but only for the sitemap.
- Result on Next: `rel=canonical`/`hreflang` lead to 404 while the sitemap lists different URLs for the same pages — contradictory signals.
- Additionally, blog alternates are identical for every locale (provider stores `canonical_route` for each locale) and product alternates lack locale prefixes — the hreflang cluster degenerates.

**Product effect:** on Next the primary SEO signal channel contradicts the sitemap and lands on 404; on Leptos the module routes exist, so the defect only surfaces on host changes.

**Required remediation (blocked on ADR — do not implement before the ADR):**
1. ADR: define which URL family is the **public canonical** (recommendation: native locale-qualified routes as forum already does; module routes become retried redirects to the public ones). Update `DECISIONS/2026-10-08-...` at this contract boundary.
2. Either owners emit the public canonical in `canonical_route`/`alternates`, or one shared route-family adapter applies to **all** outputs (canonical, hreflang, redirect target, sitemap).
3. Fix blog/product alternates per-locale (independent bug).
4. Extend `scripts/verify/verify-seo-fba.mjs` and runtime-parity fixtures with "canonical/hreflang/sitemap of one target must resolve on both storefronts".

---

#### P1-3 · AI smart fill clobbered noindex/nofollow, froze generated snapshots as explicit, and was writable with generation permission alone — **FIXED**

**Chain:** C2 (authorization) + C1 (render) + C7 (events).

**Evidence at base:**
- `ai_smart_fill_seo_meta` built `SeoMetaInput` from `generate_seo_metadata` and persisted it via `upsert_meta`.
- `ai_seo.rs` hardcodes `robots = "index, follow"` → `noindex`/`nofollow` in the input were always `false`; any explicit noindex was **silently erased** by an AI fill.
- The existing record was read through `seo_meta()`, a **merged** view: writing it back froze provider fallback schema/OG images as explicit; `canonical_url = target_state.canonical_path` was persisted as an explicit canonical, freezing a stale path that then conflicted with the auto-created redirect on the next rename.
- Permissions: `require_seo_permission(&[UPDATE, GENERATE])` is **any-of** (`has_any_effective_permission`) — `SEO_GENERATE` alone was sufficient to write explicit meta, while the plain `upsert_seo_meta` requires `SEO_UPDATE` and bulk requires `UPDATE+MANAGE`.

**Product effect:** "AI fill" silently dropped noindex, froze schema/image copies, could pin a stale canonical, and allowed writes through a weaker grant than any other write path.

**Applied fix (this branch):**
1. Composition moved into the generation repository: `services::ai_seo::ai_smart_fill_meta` is now the only path that persists generated output; it reads raw state via `load_explicit_meta` (not the merged read-model) and builds the input with a pure, unit-tested `build_smart_fill_input`.
2. Preservation rules: stored robots directives, explicit canonical URL, and structured data are preserved unchanged; an absent explicit canonical stays **absent** (the generated/frozen snapshot is never materialized); the exact locale's explicit `og:image` is preserved (the generator has no OG image source); textual fields (`title/description/keywords/og_title/og_description`) are refreshed from generation.
3. GraphQL mutation is now a thin authorize-and-forward layer with **two separate permission checks** (`SEO_UPDATE` and `SEO_GENERATE`), because matching is any-of within a single call — stacked grants are required to write.
4. Unit tests added in `ai_seo.rs`: explicit state survives; snapshots are never materialized without explicit state; other-locale `og:image` is not copied into the target locale row.

**Acceptance still to add:** an integration test "GENERATE without UPDATE cannot write" — blocked on Rust test execution.

---

### P2 — substantial inconsistencies

#### P2-4 · `auto_redirect_on_canonical_url_changed` mutated redirect rows without a transaction or events — **FIXED** (P3-19 fixed with it)

**Chain:** C3 → C7 → cross-replica cache.

**Evidence at base:** step 1 (deactivate redirects sourced at the new canonical) and step 3 (flatten chains `A→B` → `A→C`) ran as raw `active.update(&self.db)` with `let _ =` error swallowing, **without** `publish_redirect_transition_in_tx` and **without** a transaction; only step 2 (`upsert_redirect`) wrote `seo_event_delivery (source_kind='redirect')` — the only channel replicas observe; a fake `TenantContext` was constructed from settings (P3-19).

**Product effect:** with empty `old_urls` or failed validations, replicas kept serving the **old** redirect until `redirect_cache_ttl_seconds` expired (abandoned loops/bounces after renames), and multi-row updates could apply partially.

**Applied fix (this branch):**
1. The whole rename runs in **one transaction**: capture-set deactivation → per-old-URL create → inbound flatten; every changed row publishes its transition via `publish_redirect_transition_in_tx` inside the same transaction; cache invalidation happens once after commit; any DB/event failure aborts the rename (the handler propagates and the event can be retried — the flow is idempotent).
2. Capture-set deactivation now covers **wildcard** rows capturing the new canonical (`redirect_pattern_captures`, a pure predicate mirroring `RedirectLookup::matches`), not only exact sources — a live canonical can no longer be bounced away by `/*`.
3. The fake `TenantContext` is gone (P3-19): the flow works from `tenant_id` plus normalized values only.
4. Shared row writer extracted as `persist_redirect_in_tx`, used by both manual upsert and auto flow.

**Acceptance still to add:** cross-replica test "slug rename → replica B does not serve the old redirect without waiting for TTL" — blocked on Rust test execution.

---

#### P2-5 · Diagnostics and admin preview compute "effective" values with a different formula than the public render — **PARTIAL (page-context half fixed)**

**Chain:** C6 vs C1, C2 vs C1.

**Evidence at base:** `build_diagnostics_page_context` duplicated `merge_page_context` but omitted: (1) `canonical_host_allowed`, (2) `apply_canonical_trailing_slash` / `apply_canonical_protocol`, (3) the `hreflang_enabled`/`submodule_hreflang_enabled` gate for alternates, (4) `rich_snippets_enabled` when counting schema blocks — plus (not in the original list) `apply_metadata_settings` (title suffix/limits) and `apply_open_graph_settings` (site name/default OG image). The authoring read-model `seo_meta` additionally omits suffix/limits, canonical normalization, and returns `canonical_url: None` where the render always emits one.

**Applied fix — page-context half (this branch):**
1. `merge_page_context` is now `pub(super)` and documented as **the single formula** for render, diagnostics, and previews; diagnostics calls it directly (`self.merge_page_context(...)` in the batch loop) instead of its own copy.
2. The entire parallel `build_diagnostics_page_context` (284 lines) was deleted (zero-legacy); dead imports removed.
3. Verification contract evolved with the code: `verify-seo-diagnostics-batch-reads.mjs` now requires the shared `self.merge_page_context(` projection in the batch file **and** `pub(super) async fn merge_page_context(` in routing, keeping all batching/forbidden-read guards.
4. Behavior consequence: duplicate-canonical detection and redirect-chain checks now see host/slash-normalized canonicals exactly as rendered; schema/hreflang counts respect submodule gates; titles carry suffix/limits.

**Remaining — authoring-record half (OPEN):** `seo_meta` (meta.rs) and `build_diagnostics_meta_record` (diagnostics) still build the authoring `SeoMetaRecord` projection with their own formula. Design note: the editable fields must stay **raw** (admin round-trips `canonical_url` back through `upsert_meta`; storing the render-normalized value would pin a locale into the target-level column), so the fix is additive display-only `effective_*` fields (e.g. `effective_canonical_url`) computed by a shared pure canonical resolver next to the raw ones — DTO addition, no GraphQL schema snapshot exists to update (`verify-api-surface-contract` is marker-based, additive-safe). The two record builders should then share that resolver.

---

#### P2-6 · Bulk modes gate per-row by record `source` while precedence is declared per-field; "safe" modes mark rows as failed — **FIXED**

**Chain:** C5.

**Evidence:** `apply_missing_only` rejects a row when `source == "explicit"` and `overwrite_generated_only` when `source != generated"`, but `source="explicit"` is set when **any** explicit row exists even if every field is still generated — with `source=Any` (default) "safe" modes send hundreds of rows to `failed` instead of `skipped`. Materialization modes write the **merged** state back to explicit (freezing fallback schema/generated canonical — the P1-3 disease at scale). A provider returning `Json(Null)` for `structured_data` would fail validation on **every** row of any writing mode.

**Applied fix (this branch):** `apply_bulk_patch_to_target` now plans every field independently through a pure `plan_bulk_field(mode, source, raw_present, class, patch_mode, field)` decision (`Preserve` / `Materialize` / `Applied`) driven by the **raw stored value** (`load_explicit_meta`), not by the row-level `source` group: safe modes (`apply_missing_only`, `overwrite_generated_only`, `apply_missing_schema_only`) never overwrite operator-authored fields — a Set/Clear targeting an explicit field preserves just that field while the rest of the row still receives the patch — and rows where nothing would change return a tri-state `BulkApplyOutcome::Skipped` instead of an error. Keep-decisions materialize only template fields (title/description/keywords/og_* and robots defaults); `canonical_url` and `structured_data` stay raw-absent without an explicit patch, so materialization can no longer freeze derived routes (P1-3 at scale). Invalid legacy payloads (`Json(Null)`) count as absent on both the raw and effective side and heal to `NULL` on write instead of failing every row. `overwrite_generated_only` stays loud (field-named validation error) for Set/Clear on non-generated, non-explicit fields; `preview_only` keeps its row-level guard. Skips are persisted as job-item status `"skipped"` (progress derives from items) and surfaced through a new `skipped_count` column (`m20261018_000013_add_seo_bulk_job_skipped_count`), `SeoBulkJobRecord`/`SeoBulkJobStatusRecord.skipped_count`, GraphQL (`skippedCount`), the admin bulk panel (`ok= | failed= | skipped= | artifacts=`), and both `next-admin` GraphQL queries; `finish_bulk_job` takes the skipped counter and a skipped-only run finishes `completed` (skips never count as failures). Six unit tests pin the per-field table (explicit preservation, template-only materialization, applied operator intent, loud generated-only, preview guard, JSON `Null` healing).

---

#### P2-7 · OpenGraph precedence is all-or-nothing while title/description fall back per-field — **FIXED**

**Evidence at base:** if **any** explicit OG field was set, `og_title`/`og_description` came only from explicit (`None` → provider state), skipping the generated template; title/description use strict per-field `explicit → generated → state`.

**Applied fix (this branch):** extracted one pure `resolve_open_graph_text` helper in `routing.rs`, used by both public render (`merge_page_context`) and diagnostics (`build_diagnostics_page_context`): per-field `explicit → generated → state` for OG title/description, explicit `og:image` preserved, group source semantics unchanged (any explicit input → Explicit). Unit tests cover the image-only, explicit-title, generated-only, and fallback cases.

---

#### P2-8 · `title_suffix` is appended before truncation at `meta_title_max_length = 60` — **FIXED**

**Evidence at base:** the suffix push happened first, then `truncate_chars(title, 60)` — a 60-char generated title lost the suffix entirely or got cut mid-suffix.

**Applied fix (this branch):** `apply_metadata_settings` now reserves `separator.chars() + suffix.chars()` **before** truncating the base, then appends the separator and suffix, then applies the final hard cap (a suffix longer than the cap itself is the only case still cut). Unit test `title_suffix_survives_truncation` pins "suffix always whole" and unchanged no-suffix truncation.

---

#### P2-9 · `locale_prefixed_path` is idempotent only for the requested locale — double prefixes like `/en/ru/about` — **OPEN (design note)**

**Evidence:** a redirect target or explicit canonical stored as `/ru/about` renders as `/en/ru/about` under `en`. Next `site.ts` strips **any** known locale prefix; the backend does not.

**Design note (deferred deliberately):** the sound fix requires the tenant's authoritative locale list inside `locale_prefixed_path`'s callers. Purely syntactic detection is unsound here: ICU/BCP-47 parsing accepts slug-shaped segments (`faq`, `modules`, `about` all parse as language subtags), so a strip-on-shape heuristic would corrupt legitimate targets. The authoritative source exists — `rustok_tenant::TenantLocalePolicyPort::read_locale_policy` (entity `tenant_locale`, projection `locales + default_locale`), already used by the server locale middleware — and `rustok-seo` already depends on `rustok-tenant` at the Cargo level with `PortContext`/port-call precedent (`targets.rs` media port). Consuming it from SEO (optional injected port with cached reads, threading `&[String]` locales through `locale_prefixed_path` at the 4 call sites, fallback to today's own-locale idempotency when the port is absent) is a boundary-consuming change and should be reviewed alongside the P1-2 ADR acceptance rather than smuggled in as a heuristic.

---

#### P2-10 · Redirect cycles were storable, 5xx'd at runtime, and alias targets cost two hops — **PARTIAL**

**Chain:** C3 → C1 → storefront.

**Evidence at base:** the write path rejected only `source == target` (`A→B` plus `B→A` was storable); a runtime self-loop threw `SeoError::validation("redirect loop detected")` → 4xx/5xx on live traffic; a manual redirect to a legacy alias redirects to the alias form which itself requires another hop.

**Applied fix (this branch):**
1. **Write-time cycle rejection:** `ensure_no_redirect_cycle` walks the stored matching graph from the proposed target with a visited set and a hop budget (16), failing with a validation error when it reaches the proposed source, revisits a URL, or exceeds the budget. Active writes only — deactivation is always allowed so operators can disable an existing loop.
2. **Runtime fail-safe:** a stored self-loop now logs a warning and serves the page context **without** the redirect decision instead of returning an error; a stored two-hop cycle (`A→B→A`) and target self-loops are suppressed the same way. Writes now reject new cycles, so runtime suppression mainly protects legacy rows.
3. Capture-set deactivation for the live canonical (see P2-4) removes the wildcard-bounce class of loops.

**Remaining (design needed):** flattening a manual redirect whose **target** is itself a registry-owned alias still costs a second hop. Blindly substituting the target's resolved canonical would override operator intent when the target is a live page with a merely custom canonical tag; the fix must distinguish "target would HTTP-redirect anyway" from "target renders with a canonical tag".

**Acceptance still to add:** store a legacy `A↔B` pair and assert the storefront never errors; test that a cycle-closing upsert returns a validation error — blocked on Rust test execution.

---

#### P2-11 · Diagnostics: cost and blindness of key checks — **OPEN**

**Evidence:** every call does a full sitemap-candidate scan, a full bulk-summary list, a second scan for cross-links, and **two** provider-state loads per target (N+1 at the provider: product = item + translations + images ×2) — tens of thousands of queries for a 10k catalog. The hreflang check compares locale **sets** only, so identical hrefs for all locales (the P1-2 blog bug) pass green. Readiness scoring counts persistent Info signals (`cross_link_gap` without suggestions, `fallback_only`), compressing every fresh site to "medium".

**Fix:** batch provider state loads (à la `list_bulk_summaries_page`); treat non-unique locale-qualified hrefs as hreflang problems; separate hygienic Info signals from errors in readiness.

---

#### P2-12 · Two sources of SEO truth for product categories — **OPEN**

**Evidence:** Product registers only the `product` target; taxonomy registers `category`; Product additionally maintains its own `catalog_category_seo_translations` store plus the `category_seo` translation target, which the generic `meta` table does not read.

**Product effect:** the same category page gets title/description from Product's store (Product render) or from generic `meta` under slug `category` (SEO page-context/redirect/bulk/diagnostics) depending on who serves it; neither side sees the other's tooling (SEO-TARGET-001 changed shape, not substance).

**Fix:** designate a single source (recommendation: generic `meta` via the owner provider, with the product store as a migrated snapshot/read-model), document the owner, cover both directions with the seam verifier.

---

#### P2-13 · Next applies SEO redirects on 3 of 16 route families — **FIXED**

**Evidence:** only `page.tsx`, `blog/[slug]`, `products/[slug]` consume `redirectDecision`; `pages/[slug]`, lists, `profiles/*`, `orders*`, `account*`, `checkout`, `notifications`, `pricing`, `search` render the redirect context as a noindex page instead of redirecting. Leptos applies redirects everywhere.

**Applied fix (this branch):** the decision now applies inside `resolveSeoPageContextForRoute` itself — the single choke point every Next server page (and `generateMetadata`) already passes through — via `applySeoRouteRedirect`: 301/308 → `permanentRedirect`, anything else → `redirect`. The three hand-rolled per-page blocks (home, `blog/[slug]`, `products/[slug]`) were removed, so no page can re-implement or forget the decision; Next's app-render `errorRecovery` converts `NEXT_REDIRECT` (including from `generateMetadata`) into the decision's status and `Location` header, matching the Leptos storefront's `redirect_response(target, status)` guard on the resolved context. New contract verifier `scripts/verify/verify-seo-next-redirect-parity.mjs` pins the choke point, forbids `route.redirect`/`redirectDecision` reads anywhere under `src/app`, requires `resolveSeoPageContextForRoute` on all 16 route families, and requires the Leptos `resolved.route.redirect.is_some()` → `redirect_response(` guard (negative-tested).

---

#### P2-14 · Write-only settings (SEO-SET-001 remainder) — **OPEN**

**Evidence:** `organization_name`, `organization_logo_url`, `breadcrumbs_enabled`, `searchbox_enabled`, `twitter_card_type`, `twitter_site_handle` have no runtime consumer: operators set them, read them back normalized, and public output (organization JSON-LD, twitter card, breadcrumbs) does not change.

**Fix:** wire into `build_document`/schema helpers or mark `deprecated/experimental` in the manifest so UI hides them; either closes the "successful settings write ≈ changed public behavior" contract.

---

#### P2-15 · Admin surfaces: two UIs with different coverage — **OPEN**

**Evidence:** Leptos `rustok-seo/admin` has the full panel set (Defaults/Redirects/Sitemaps/Robots/Bulk/Diagnostics/Index); Next admin only has the index-repair operator panel. Entity authoring lives in owner panels per `DECISIONS/2026-04-19-seo-ui-ownership-by-content-module.md`. A Next-only operator physically cannot run redirects/settings/sitemap-jobs/bulk, and no executable "function × surface × owner" matrix exists.

**Fix:** publish the ownership matrix as contract JSON + verifier (pattern of `verify-seo-admin-boundary.mjs`), then either move cross-cutting panels into Next admin or formally fix Leptos admin as the only control plane and remove residual SEO actions from Next.

---

### P3 — minor/cosmetic inconsistencies

| ID | Finding | Evidence | Fix |
|---|---|---|---|
| P3-16 | Updating a redirect by `id` with a colliding source surfaces as a raw DB unique violation instead of `validation: source already exists` | `redirects.rs` upsert | pre-check inside the transaction + map `DbErr::Exec` to `SeoError::validation` |
| P3-17 | `upsert_response_locale = translations.first()` — list order decides the response locale | `meta.rs` | pick the single/sorted locale and document |
| P3-18 | `prepare_meta_transition` checks target existence by `default_locale`, not the input's actual locale | `meta.rs` | validate by `response_locale` |
| P3-19 | `auto_redirect_on_canonical_url_changed` built a fake `TenantContext` from settings | `redirects.rs` | **FIXED** — removed with P2-4 (flow uses `tenant_id` directly) |
| P3-20 | `seo_meta` for a deleted entity returns a record with `open_graph: None` (explicit-only branch) — UI shows an empty OG as normal | `meta.rs` | flag `target_missing` so admin does not present "empty" as the norm |
| P3-21 | `missing_title` is Error even for fallback targets that have a title at the provider but not in the requested locale | `diagnostics_batch.rs` | distinguish "absent in locale (fallback)" from "absent everywhere" |
| P3-22 | Bulk export still materializes the full target list on enqueue (`collect_bulk_rows_for_filter`) — bounded by `MAX_BULK_TARGETS` but O(N) per request | `bulk_io_bounded_execution.rs` | store filter+cursor; compute counts with a capped SQL `COUNT` |

---

## 4. Prioritized remediation plan

| Step | What | Why first | Acceptance |
|---|---|---|---|
| 1 | **P1-2:** canonical/hreflang route-family contract + adapter for all outputs; fix blog/product alternates | Direct SEO-traffic damage on Next (canonical → 404, contradicts sitemap) | verifier: canonical/hreflang/sitemap of each target resolve (200) on both storefronts; fixtures updated — **ADR proposed** (`DECISIONS/2026-10-18-seo-canonical-hreflang-route-family.md`, `Proposed`; implementation blocked until `Accepted`) |
| 2 | **P1-3:** safe AI-fill | Mass noindex loss — indexing of drafts | tests: noindex preserved; canonical not materialized; GENERATE without UPDATE cannot write — **DONE (unit tests; integration tests pending `cargo`)** |
| 3 | **P1-1:** any-owner gate for public sitemap/robots + disabled-owner handling in `requires_refresh` | Tenants without forum/blog currently have no sitemap at all | test: tenant with 1 of 5 modules gets a sitemap from the enabled ones — **gate DONE (test pending `cargo`)** |
| 4 | **P2-4:** one transaction + events for all auto-redirect mutations | Cross-replica divergence after renames | cross-replica test without waiting for TTL — **DONE (test pending `cargo`)** |
| 5 | **P2-10:** write-time cycles + runtime fail-safe (+ alias 1-hop remainder) | Live traffic must never 5xx on a loop | tests: A↔B rejected on write; storefront never errors on legacy loops — **DONE except alias 1-hop (design)** |
| 6 | **P2-5:** shared effective-context builder for render/diagnostics/preview | Removes the whole "diagnostics lies" class | identical canonical/title/alternates across three consumers on the same data — **page-context half DONE** (diagnostics delegates to `merge_page_context`, verifier contract updated); **authoring-record half open** (additive `effective_*` fields, see design note) |
| 7 | **P2-6:** field-level bulk guards + skipped counter + Null→None | "Safe" modes stop alarming and stop freezing fallback | partial-explicit row tests — **DONE** |
| 8 | **P2-13:** one Next redirect hook on all pages | Host parity | route matrix: redirects applied 16/16 — **DONE** (choke point in `resolveSeoPageContextForRoute` + `verify-seo-next-redirect-parity.mjs`) |
| 9 | **P2-7/8/9:** OG per-field fallback, suffix/truncate order, foreign locale prefix | Targeted render defects | unit tests per item — **P2-7 DONE** (shared `resolve_open_graph_text` + tests), **P2-8 DONE** (reserve-before-truncate + test), **P2-9 open** (see design note: needs `TenantLocalePolicyPort`, no sound syntactic shortcut) |
| 10 | **P2-11/12/14/15, P3** | Perf/categories/settings/UI matrix | per matching contract verifiers — **OPEN** |

---

## 5. What works well (not a finding)

- One `SeoPageContext` contract across hosts; REST/GraphQL permission parity spot-checked (diagnostics `READ|MANAGE`, sitemaps `READ|GENERATE`, bulk/targets/index `MANAGE` identical in `graphql/mod.rs` and `controllers/mod.rs`).
- Transactional outbox events with idempotency keys, retry/dead-letter, durable cursor for redirect-cache invalidation, replica count/cursor reconciliation.
- Public delivery safety: SSRF checks for origin and submission endpoints, `no-store` on sitemap, noindex targets excluded from sitemaps.
- Atomic queue claims, partial unique indexes, tenant-fair stale handling, 90-day retention.
- The protocol-relative bypass (from the 09-30 audit) is closed; wildcard precedence is deterministic.
- Submodule flags (redirects/sitemaps/canonical/hreflang/rich-snippets/bulk) are consumed on render and entry paths (bulk enqueue is gated in `applications.rs`).

---

## 6. Verification performed

Executed on this branch (static, Node-based):

- `npm run verify:seo:admin-boundary` — **PASS**
- `npm run verify:seo:fba` (`verify-seo-fba.mjs` + `verify-consumer-fba-runtime-order.mjs`) — **PASS**
- `scripts/verify/verify-seo-entrypoint-authorization.mjs` — **PASS**
- `scripts/verify/verify-seo-failure-classification.mjs` — **PASS**
- `scripts/verify/verify-seo-sitemap-background-worker.mjs` — **PASS**
- `scripts/verify/verify-seo-hardening-suite.mjs` — **PASS**
- `scripts/verify/verify-forum-public-discovery-seo.mjs`, `verify-forum-canonical-route-seo-policy.mjs` — **PASS**
- `scripts/verify/verify-seo-bulk-batch-reads.mjs`, `verify-seo-bulk-cursor-pagination.mjs`, `verify-seo-diagnostics-batch-reads.mjs`, `verify-seo-index-repair-background-worker.mjs` — **PASS**
- `scripts/verify/verify-rust-constructor-arity.mjs`, `verify-module-source-layout.mjs`, `verify-storefront-module-routes.mjs` — **PASS**
- `scripts/verify/run-seo-full-suite.sh` — **exit 127**: `cargo` unavailable (durable evidence in `target/verification/seo-full-suite/summary.txt`). Rust compile/tests **not executed**; unit tests added for P1-3/P2-4/P2-7/P2-8 helpers await a Rust-capable host.
- ADR governance: `npm run verify:adrs` and `npm run verify:docs` retain their **pre-existing** failure sets (12 and 19 errors, all in unrelated 2026-10-16 ADRs and `docs/standards/continuous-review-ledger.md`); adding `DECISIONS/2026-10-18-seo-canonical-hreflang-route-family.md` + registry row produced **zero new errors** (error sets diffed identical before/after).
- The full SEO static battery (§ above) was re-run after the P2-7/P2-8 changes and the ADR: **all PASS**.
- After the P2-5 page-context delegation: `verify-seo-diagnostics-batch-reads.mjs` (contract updated to
  require the shared `merge_page_context` projection in both files), `verify-seo-hardening-suite` (7 checks),
  and the entire SEO battery re-run: **all PASS**; `verify-api-surface-contract` PASS (marker-based,
  unaffected); `verify-product-category-seo-seam` remains the known pre-existing failure.
- After P2-6 (per-field bulk gates + `skipped_count`): `verify-seo-bulk-batch-reads.mjs` marker set
  re-checked (all markers intact, including `progress.artifacts == 0` and the forbidden
  `.seo_meta(` in the IO worker), `verify-api-surface-contract` PASS (additive `skippedCount`),
  and the full SEO battery re-run: **all PASS**. Six new unit tests pin the field-plan table;
  like the other Rust tests on this branch they await a Rust-capable host (cargo unavailable).
- After P2-13 (Next redirect choke point): new `scripts/verify/verify-seo-next-redirect-parity.mjs`
  **PASS** (negative-tested: removing the choke-point invocation fails it); the four edited
  TypeScript files pass `typescript@5.9` transpile diagnostics (syntax-only — `tsc --noEmit`
  project checking needs `node_modules`, which is unavailable here); Next.js `canary`
  `app-render.tsx` was inspected via `gh api`: `renderToStream errorRecovery` classifies
  `isRedirectError` anywhere in the render pass (including `generateMetadata`) into the
  decision's status + `location` header; full SEO battery re-run: **all PASS**.

Not run / environment-dependent:

- `cargo check -p rustok-seo` / `cargo test -p rustok-seo` — canonical scoped commands per `docs/verification/README.md`; impossible here.
- `scripts/verify/verify-product-category-seo-seam.mjs` — **FAIL**, but pre-existing at base: it reads only `rustok-product`/`rustok-taxonomy` files that this branch does not touch.

Other limitations: P1-2 should be confirmed with live HTML on a Next deploy (`curl /products/<slug> | grep rel=canonical`); SQL plans/volumes on large catalogs (diagnostics/bulk) were not measured — estimates are qualitative.
