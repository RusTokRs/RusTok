# rustok-seo

## Purpose

`rustok-seo` provides a tenant-aware SEO runtime for RusToK. It owns explicit SEO metadata overrides, template-generated SEO, bulk remediation, manual redirects, sitemap generation, robots.txt rendering, diagnostics, and a storefront-facing page-context contract for SSR metadata generation across GraphQL, REST, and Leptos server-function surfaces.

## Responsibilities

- resolve SEO page context for `page`, `product`, `blog_post`, `forum_category`, and `forum_topic`
- consume registry-backed SEO target providers through `rustok-seo-targets` and the shared runtime extensions store
- keep public forum topic SEO resolution channel-aware when a topic is restricted by forum channel access
- resolve effective metadata through explicit SEO > template-generated SEO > domain/entity fallback
- run bulk remediation with safe apply modes for preview, missing-only materialization, generated-only overwrite, and explicit-force overwrite; bulk apply gates **per field** — safe modes never overwrite operator-authored values, materialization never freezes canonical/structured data without an explicit patch, and rows with nothing left to change finish as `skipped` (reported via `skipped_count`), never `failed`
- expose diagnostics with readiness scoring, issue aggregates, hreflang gap detection, canonical redirect chain/loop detection, `cross_link_gap` hints, and image descriptor quality checks (`missing_image_alt`, `missing_image_size`) for SEO-critical targets; diagnostics page contexts are built by the shared render formula (`services::routing::merge_page_context`) so reported canonical/title/alternates/schema always equal public output. Provider state loads go through one batched call per scope (`load_target_states_batch` → `SeoTargetProvider::load_targets`; product ships a four-query lean SEO projection shared by state/bulk/sitemap scans), hreflang treats one href shared by several locales as `duplicate_hreflang_href`, and readiness counts defects only — hygienic Info signals stay visible in issue counts without dragging the score
- normalize JSON-LD into typed `SeoStructuredDataBlock` records with schema kind, source state, and `@graph` expansion
- manage manual redirects and canonical overrides
- apply `route.redirect` decisions as real HTTP redirects on **every** storefront route family: the
  Leptos host answers `redirect_response(target, status)` from the resolved context, and the Next
  host applies the same decision in the single `resolveSeoPageContextForRoute` choke point
  (301/308 permanent, anything else temporary), so no page can render a redirect decision as a
  noindex page anymore (audit P2-13); `scripts/verify/verify-seo-next-redirect-parity.mjs` pins
  the choke point, the 16-family matrix, and the Leptos guard
- generate sitemap files, serve `robots.txt`, and submit sitemap indexes via runtime adapters with per-endpoint aggregation and bounded partial-failure summaries; public sitemap responses use `no-store`. If a target owner can narrow visibility between operator jobs (Forum settings and audience policy), the public index/pages always rebuild from current owner candidates without writing a new operator job. Monitor this per-request read cost on large catalogs. Forum's public bulk SEO pages also derive cursors from audience-visible categories/topics rather than forwarding raw target cursors; selective policies can make these reads scan more raw candidates than a single page.
- gate public sitemap/robots availability on **any** enabled sitemap-capable owner module: disabled owners are skipped by candidate collection and refresh probes instead of hiding every owner's routes (previously one disabled module 404'd the whole public surface)
- compose AI smart fill inside the generation repository (`services::ai_seo::ai_smart_fill_meta`): only textual fields are refreshed; explicit robots directives, canonical URL, structured data, and the locale's explicit `og:image` are preserved, and generation snapshots (generated/frozen canonical, hardcoded robots) are never materialized as explicit overrides. Smart fill is a write and therefore requires stacked `seo:update` **and** `seo:generate` grants — generation alone can never persist
- apply canonical-rename auto redirects (deactivate the new URL's live capture set, create `old→new`, flatten inbound chains) in one transaction with per-row domain events; reject cycle-closing active manual redirects at the write boundary while always allowing deactivation; suppress stored self-loops and two-hop cycles at runtime with a warning instead of failing the route with 5xx
- render metadata with per-field precedence: OpenGraph title/description fall back
  `explicit → generated → state` independently in public render **and** diagnostics (a lone explicit
  `og:image` no longer bypasses the generated template), and `title_suffix` reserves its separator and
  suffix before title truncation so a long generated title can never drop the suffix
- prepare Phase D productionization seams for typed SEO events, outbox emission/idempotency, and SEO→index integration without breaking existing public contracts
- expose a headless REST read path for `SeoPageContext` at `/api/seo/page-context`, reusing canonical request locale/channel context
- keep REST handlers on narrow `SeoHttpRuntime` state with explicit DB/event bus/runtime extension handles; controller state is isolated to the Axum adapter
- expose registry-backed SEO target descriptors through GraphQL `seoTargets` and REST `/api/seo/targets`
  with the same tenant/module-enabled gate and `seo:manage` admin permission contract
- expose read-only cross-link suggestions through GraphQL `seoCrossLinkSuggestions` and REST `/api/seo/cross-link-suggestions`
  with tenant/module checks and `seo:read|seo:manage` parity
- expose control-plane REST parity surfaces for diagnostics/sitemaps/bulk jobs:
  `/api/seo/diagnostics`, `/api/seo/sitemaps/status`, `/api/seo/sitemaps/jobs`,
  `/api/seo/sitemaps/jobs/{job_id}`, `/api/seo/bulk/jobs`, `/api/seo/bulk/jobs/{job_id}`
- keep REST control-plane errors aligned with GraphQL error codes through a GraphQL-compatible
  envelope (`errors[].extensions.code`)
- provide shared SEO capability contracts that owner modules can embed into their own admin UI
- expose admin and storefront read/write surfaces through GraphQL, HTTP, and Leptos server functions
- require host runtimes to inject `ModuleRuntimeExtensions` for SEO target registry lookup; built-in registries are test-only helpers, not production fallback behavior

## Entry points

- runtime module: `rustok_seo::SeoModule`
- application composition: `rustok_seo::SeoApplicationServices`, exposing focused `settings()`, `metadata()`, `routing()`, `redirects()`, `sitemaps()`, `bulk()`, and `operations()` services
- GraphQL: `rustok_seo::graphql::{SeoQuery, SeoMutation}`
- HTTP routes: `rustok_seo::controllers::routes`
- cross-cutting admin UI: `crates/modules/rustok-seo/admin`
- Rust renderer support: `crates/modules/rustok-seo/render`

## Interactions

- reads canonical routing substrate from `rustok-content`
- reads page/blog/product/forum content from `rustok-pages`, `rustok-blog`, `rustok-product`, and `rustok-forum`
- consumes media descriptors at runtime boundaries and maps them into the independent `rustok-seo-targets::SeoTargetImageRecord` DTO for OG/Twitter/schema fallback
- consumes typed `SeoModuleSettings`, while the current runtime/admin persistence
  path still reads or writes `tenant_modules` directly and is therefore a settings
  owner-boundary migration target
- is mounted by `apps/server`, consumed by `apps/storefront`, and shared with `apps/next-frontend`
- reuses host-provided `RequestContext.channel_slug` on REST/GraphQL/Leptos SSR paths so restricted forum topics only resolve SEO in the matching public channel
- pairs with `rustok-seo-render` for Rust-host SSR head rendering without moving SEO resolution out of the module
- consumes `rustok-seo-targets` as the extensibility seam for owner-module target registration
- is expected to integrate with owner-module admin surfaces in `rustok-pages`, `rustok-product`,
  `rustok-blog`, and `rustok-forum`; `rustok-seo/admin` is reserved for cross-cutting SEO
  infrastructure rather than long-term ownership of entity editors

## Settings boundary

SEO owns the semantics and typed normalization of its tenant settings. The current
service and native admin adapter nevertheless access `tenant_modules` persistence
directly. This is current executable truth, not the canonical owner contract; the
cutover must route reads and writes through the static module settings owner with
revision, schema-digest, and activation semantics.

The manifest describes `template_defaults` using canonical `properties` and
`template_overrides` using `type = "object"`. Manifest deserialization enforces
`deny_unknown_fields` and `snake_case` key validation, ensuring settings adhere to the
canonical platform vocabulary. See [`docs/architecture/settings.md`](../../../docs/architecture/settings.md).

## Current execution wave (Phase D)

Phase D is a productionization and integration-parity wave.

Completed baseline (`D1..D6`):

- typed SEO domain events + outbox delivery foundations (live `seo_event_deliveries` tracking with outbox envelope ids and duplicate-emission guard)
- SEO-to-index consumer seam with bounded retry/dead-letter behavior
- GraphQL/REST control-plane parity completion (additive `v1` only; REST diagnostics/sitemaps/bulk job parity endpoints are live and share GraphQL-compatible error codes)
- owner-module admin integrations and Next admin operator parity

Current wave (`D7..D9`) is grouped into large end-to-end milestones (`A..E`):

- `A`: runtime SEO data plumbing (REST primary + GraphQL selected-path adapter semantics for Next storefront)
- `B`: runtime migration cutover for Next `robots`/`sitemap`/metadata surfaces
- `C`: route ownership matrix + cross-host fixture parity guardrails
- `D`: verification matrix execution
- `E`: docs/runbooks/readiness closeout

See `docs/implementation-plan.md` for the authoritative milestone checklist and status.

## Known limitations / pending implementation

Tracked from [`docs/audits/seo-module-product-logic-audit-2026-10-18.md`](../../../docs/audits/seo-module-product-logic-audit-2026-10-18.md)
(source of truth for open findings):

- **Route-family contract (audit P1-2) — ADR proposed, implementation blocked.**
  `DECISIONS/2026-10-18-seo-canonical-hreflang-route-family.md` (`Proposed`) defines one
  owner-declared canonical route family per target, locale-qualified hrefs, and a single
  SEO output adapter; head/sitemap unification waits for the ADR to be accepted.
- **Locale-aware path prefixing (audit P2-9) — deferred with design note.** Stripping a stored
  foreign locale prefix (`/ru/about` under `en`) needs the tenant locale policy
  (`rustok_tenant::TenantLocalePolicyPort`) inside SEO; syntactic locale-tag detection is unsound
  (slug-shaped segments parse as BCP-47 language tags). See the audit report design note.
- **Manual redirect target chains (audit P2-10 remainder).** Cycle creation is rejected and
  stored cycles are suppressed at runtime, but a manual redirect whose target is itself a
  registry-owned alias still costs a second hop; flattening it requires target-side alias
  resolution design (operator destination intent must win when the target is a live page).
- **Next module-route hosting pending** — backend `modules/*` routes still 404 on the Next host
  without rewrites; that hosting half belongs to the route-family ADR
  (`DECISIONS/2026-10-18-seo-canonical-hreflang-route-family.md`, `Proposed`). SEO redirect parity
  itself is implemented on all 16 storefront route families (audit P2-13).
- **Authoring-record preview drift (audit P2-5 remainder) — deferred with design note.**
  Diagnostics page contexts now share the render formula (`merge_page_context`), but the editable
  `SeoMetaRecord` projection (`seo_meta` and the diagnostics meta record) still shows raw stored
  values; the fix is additive display-only `effective_*` fields computed by a shared canonical
  resolver, because editable fields must stay raw for admin round-trips. See the audit report.
- **Diagnostics N+1 reads (P2-11), category SEO dual-store (P2-12),
  write-only settings without admin consumers (P2-14),
  admin surface split (P2-15)** — open; evidence and file:line
  references live in the audit report.
- **Rust compile/test evidence is host-dependent:** the canonical scoped commands are
  `cargo check -p rustok-seo` / `cargo test -p rustok-seo` (see `docs/verification/README.md`).
  Static verification for this module runs via `npm run verify:seo:admin-boundary`,
  `npm run verify:seo:fba`, and `scripts/verify/verify-seo-*.mjs`.
