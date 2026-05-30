# rustok-seo

## Purpose

`rustok-seo` provides a tenant-aware SEO runtime for RusToK. It owns explicit SEO metadata overrides, template-generated SEO, bulk remediation, manual redirects, sitemap generation, robots.txt rendering, diagnostics, and a storefront-facing page-context contract for SSR metadata generation across GraphQL, REST, and Leptos server-function surfaces.

## Responsibilities

- resolve SEO page context for `page`, `product`, `blog_post`, `forum_category`, and `forum_topic`
- consume registry-backed SEO target providers through `rustok-seo-targets` and the shared runtime extensions store
- keep public forum topic SEO resolution channel-aware when a topic is restricted by forum channel access
- resolve effective metadata through explicit SEO > template-generated SEO > domain/entity fallback
- run bulk remediation with safe apply modes for preview, missing-only materialization, generated-only overwrite, and explicit-force overwrite
- expose diagnostics with readiness scoring, issue aggregates, hreflang gap detection, canonical redirect chain/loop detection, `cross_link_gap` hints, and image descriptor quality checks (`missing_image_alt`, `missing_image_size`) for SEO-critical targets
- normalize JSON-LD into typed `SeoStructuredDataBlock` records with schema kind, source state, and `@graph` expansion
- manage manual redirects and canonical overrides
- generate sitemap files, serve `robots.txt`, and submit sitemap indexes via runtime adapters with per-endpoint aggregation and bounded partial-failure summaries
- prepare Phase D productionization seams for typed SEO events, outbox emission/idempotency, and SEO→index integration without breaking existing public contracts
- expose a headless REST read path for `SeoPageContext` at `/api/seo/page-context`, reusing canonical request locale/channel context
- expose registry-backed SEO target descriptors through GraphQL `seoTargets` and REST `/api/seo/targets`
  with the same tenant/module-enabled gate and `seo:manage` admin permission contract
- expose read-only cross-link suggestions through GraphQL `seoCrossLinkSuggestions` and REST `/api/seo/cross-link-suggestions`
  with tenant/module checks and `seo:read|seo:manage` parity
- expose control-plane REST parity surfaces for diagnostics/sitemaps/bulk jobs:
  `/api/seo/diagnostics`, `/api/seo/sitemaps/status`, `/api/seo/sitemaps/jobs`,
  `/api/seo/sitemaps/jobs/{job_id}`, `/api/seo/bulk/jobs`, `/api/seo/bulk/jobs/{job_id}`
- provide shared SEO capability contracts that owner modules can embed into their own admin UI
- expose admin and storefront read/write surfaces through GraphQL, HTTP, and Leptos server functions
- require host runtimes to inject `ModuleRuntimeExtensions` for SEO target registry lookup; built-in registries are test-only helpers, not production fallback behavior

## Entry points

- runtime module: `rustok_seo::SeoModule`
- GraphQL: `rustok_seo::graphql::{SeoQuery, SeoMutation}`
- HTTP routes: `rustok_seo::controllers::routes`
- cross-cutting admin UI: `crates/rustok-seo/admin`
- Rust renderer support: `crates/rustok-seo/render`

## Interactions

- reads canonical routing substrate from `rustok-content`
- reads page/blog/product/forum content from `rustok-pages`, `rustok-blog`, `rustok-product`, and `rustok-forum`
- consumes `rustok-media::MediaImageDescriptor` as the typed image boundary for OG/Twitter/schema fallback
- consumes tenant/module settings from `rustok-tenant`
- is mounted by `apps/server`, consumed by `apps/storefront`, and shared with `apps/next-frontend`
- reuses host-provided `RequestContext.channel_slug` on REST/GraphQL/Leptos SSR paths so restricted forum topics only resolve SEO in the matching public channel
- pairs with `rustok-seo-render` for Rust-host SSR head rendering without moving SEO resolution out of the module
- consumes `rustok-seo-targets` as the extensibility seam for owner-module target registration
- is expected to integrate with owner-module admin surfaces in `rustok-pages`, `rustok-product`,
  `rustok-blog`, and `rustok-forum`; `rustok-seo/admin` is reserved for cross-cutting SEO
  infrastructure rather than long-term ownership of entity editors

## Current execution wave (Phase D)

Phase D is planned as a productionization and integration-parity wave:

- typed SEO domain events + outbox delivery foundations (live `seo_event_deliveries` tracking with outbox envelope ids and duplicate-emission guard)
- SEO-to-index consumer seam with bounded retry/dead-letter behavior
- GraphQL/REST control-plane parity completion (additive `v1` only; REST diagnostics/sitemaps/bulk job parity endpoints are live)
- expanded admin and storefront/Next host integrations
- verification matrix and operational runbooks

See `docs/implementation-plan.md` for the batch-by-batch checklist (`D1..D9`).
