# `rustok-seo` Documentation

`rustok-seo` — optional module of the platform for headless SEO runtime and cross-cutting SEO control-plane. The module owns tenant-scoped SEO metadata, template-generated SEO, bulk remediation, redirects, sitemap/robots generation, diagnostics and storefront-facing `SeoPageContext`.

Entity-specific SEO authoring does not live in `rustok-seo-admin`: pages, products, blog and forum embed SEO panels in their own module-owned admin surfaces via `rustok-seo-admin-support`.

## Purpose

The purpose of the module is to give the platform a unified typed SEO runtime: owner modules publish target records and safe fields for templates, and `rustok-seo` centrally builds effective metadata, bulk remediation, diagnostics and the public/storefront read-side.

## Scope

- canonical read contract `SeoPageContext = route + document`, where `route` handles locale/canonical/redirect/hreflang and `document` handles typed head metadata;
- metadata precedence: explicit SEO > template-generated SEO > domain/entity fallback;
- tenant-scoped `template_defaults` and per-target `template_overrides`;
- bulk editor and remediation jobs over `seo_bulk_jobs`, `seo_bulk_job_items`, `seo_bulk_job_artifacts`;
- manual redirects, sitemap jobs/files and `robots.txt`;
- runtime adapter seam for sitemap submission with per-endpoint statuses and bounded partial-failure summary;
- diagnostics read model: readiness score, issue list, issue aggregates and source counts, including image descriptor quality issue codes `missing_image_alt` and `missing_image_size` for SEO-critical targets;
- read-only cross-link suggestions (`seoCrossLinkSuggestions` / `/api/seo/cross-link-suggestions`) without automatic HTML mutation;
- REST handlers use narrow `SeoHttpRuntime` with explicit DB/event bus/runtime extensions handles; the route-state adapter is the only host composition boundary;
- public application calls enter through `SeoApplicationServices` and select a focused settings, metadata, routing, redirect, sitemap, bulk, or operations service; the broad transactional runtime remains crate-private;
- REST control-plane parity endpoints for diagnostics/sitemaps/bulk jobs: `/api/seo/diagnostics`, `/api/seo/sitemaps/status`, `/api/seo/sitemaps/jobs`, `/api/seo/sitemaps/jobs/{job_id}`, `/api/seo/bulk/jobs`, `/api/seo/bulk/jobs/{job_id}`;
- REST error envelope on control-plane endpoints is unified with GraphQL codes (`errors[].extensions.code`: `BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, `INTERNAL_ERROR`) for deterministic client-side mapping;
- shared capability registry via `rustok-seo-targets`;
- support crates `rustok-seo-render` and `rustok-seo-admin-support`;
- execution wave Phase D: typed SEO events/outbox/index seam, REST parity completion, admin/host integration parity, verification matrix and runbooks.

## Template-generated SEO

Owner modules do not render SEO templates themselves. They only supply typed `SeoLoadedTargetRecord.template_fields` through `rustok-seo-targets`; the map allows SEO-safe fields such as `title`, `description`, `route`, `locale`, slug/handle/id.

`rustok-seo` centrally renders:

- `title`;
- `meta_description`;
- `canonical_url`;
- `keywords`;
- `robots`;
- Open Graph title/description;
- Twitter title/description.

`SeoPageContext.document.effective_state` and `seoMeta.effective_state` show the source of each effective value: `explicit`, `generated` or `fallback`. This is needed so that the admin UI does not mix user-authored overrides with synthesis from templates.

## Rich snippets and typed schema blocks

`SeoDocument.structured_data_blocks` is the canonical runtime layer for JSON-LD. Storage still accepts `seo_meta.structured_data` as a JSON payload, but the read-side does not return it as an untyped blob:

- `schema_kind` — canonical enum for supported schema.org shapes (`product`, `offer`, `aggregate_rating`, `breadcrumb_list`, `item_list`, `organization`, `local_business`, `web_site`, `search_action`, `article`, `blog_posting`, `faq_page`, `how_to`, `discussion_forum_posting` and others);
- `schema_type` — the original `@type` from JSON-LD;
- `kind` — legacy string alias for current headless consumers;
- `source` — `explicit`, `generated` or `fallback`, synchronized with effective SEO state;
- `payload` — JSON-LD payload rendered as `<script type="application/ld+json">`.

If the payload contains `@graph`, the runtime expands the graph into separate schema blocks and inherits `@context`. Diagnostics consider a schema as missing if typed blocks were not produced, and separately mark blocks without a recognized schema.org type as `unknown_schema_type`.

Explicit write paths (`upsertSeoMeta`, Leptos server functions and bulk apply) validate new `structured_data` values as JSON-LD. The payload must be an object, array or `@graph` with at least one non-empty `@type`; future schema.org types are allowed as `other`, but untyped JSON/scalars are rejected.

Built-in owner providers (`pages/product/blog/forum`) generate fallback structured data via `rustok-seo-targets::schema` builders. This preserves module ownership but prevents each provider from hand-rolling its own raw `json!` shape.

## Media image descriptor boundary (C3)

Image metadata boundary is established between `rustok-media` and `rustok-seo`:

- `rustok-media` publishes a typed contract `MediaImageDescriptor` (`url`, `alt`, `width`, `height`, `mime_type` + derived helpers like `has_alt`, `has_size`, `pixel_count`, `aspect_ratio`, `file_extension`);
- owner SEO providers (`pages/product/blog/forum`) populate OG/Twitter/schema fallback and image template fields via these descriptors;
- `rustok-seo` does not read raw media blobs and works only with descriptor payloads in `SeoTargetOpenGraphRecord.images`.

## Bulk remediation

Bulk apply is no longer a simple overwrite job. Each apply job must select a mode:

- `preview_only` — only builds a preview artifact with effective SEO, without writing `meta`;
- `apply_missing_only` — materializes missing/generated/fallback SEO into explicit records, but does not overwrite existing explicit SEO;
- `overwrite_generated_only` — writes only targets whose current source is `generated`;
- `force_overwrite_explicit` — allowed operator override of explicit SEO, requires a real patch delta.

CSV export/import remain scoped to a single `SeoTargetSlug` and a single locale. Artifacts are downloaded through a tenant/RBAC-checked SEO endpoint, without filesystem leakage.

## Sitemap submission semantics (C1)

Sitemap submit orchestration remains an internal runtime concern and does not change the public shape `SeoSitemapStatusRecord`, but now maintains telemetry-friendly aggregation:

- per-endpoint status is recorded (`success`, `failure`, `timeout`, `invalid_endpoint`);
- partial failure is considered **job success + submission failure summary**: sitemap files are already generated, but the job `last_error` stores a bounded aggregate message;
- deterministic truncation policy uses `max_errors` and `max_timeout_details`, ordering is always stable by endpoint;
- invalid endpoints are skipped at the adapter layer and counted as failure without HTTP submit.

## Diagnostics

`seoDiagnostics` and the admin diagnostics pane build a tenant-level summary from the target registry:

- missing title / description;
- duplicate canonical URL;
- noindex + canonical conflicts;
- canonical URLs pointing to redirect targets, chains or loops;
- missing hreflang alternates and missing `x-default`;
- missing typed schema blocks and unknown schema.org types;
- missing sitemap candidates;
- fallback-only targets where policy expects template or explicit SEO;
- `cross_link_gap` for targets without read-only cross-link suggestions with remediation entrypoint via `seoCrossLinkSuggestions`/`/api/seo/cross-link-suggestions`;
- `missing_image_alt` and `missing_image_size` for SEO-critical targets where OG/Twitter images lack full descriptor metadata.

Readiness score is derived from the issue set. The summary also returns counts by issue code and target kind, so the admin UI can build filters and remediation entrypoints without local error classification. Diagnostics do not replace owner-module editors but provide an entrypoint for remediation.

## Integration

- `apps/storefront` consumes `SeoPageContext.route + document` via `rustok-seo-render` for SSR `<title>`, meta description, canonical, robots, hreflang, Open Graph, Twitter, verification tags, pagination links and JSON-LD.
- `apps/next-frontend` uses a shared runtime SEO adapter over the Next Metadata API: `SeoPageContext` arrives via REST primary + GraphQL selected-path transport, `robots.ts`/`sitemap.ts` read the runtime source, and host-local static metadata remains only as an emergency fallback path.
- `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin` and `rustok-forum/admin` are the canonical owner surfaces for entity SEO authoring.
- The host runtime must pass `ModuleRuntimeExtensions` with `SeoTargetRegistry` to all SEO entrypoints; a built-in registry is only acceptable in tests/helpers.

## Phase D roadmap (productionization)

The current roadmap is captured in `docs/implementation-plan.md`: base batches `D1..D6` are closed, and `D7..D9` are grouped into larger Milestones `A..E`.

- `D1` closed: contract freeze, compatibility policy (`v1 additive only`) and rollout flags.
- `D2-D3`: typed SEO events, outbox emission/idempotency, SEO->index consumer seam — closed.
- `D4-D5`: GraphQL/REST parity completion and migrations/backfill/replay policy — closed (including index tracking/replay endpoints `/api/seo/index/tracking`, `/api/seo/index/repair-replay`, GraphQL `seoIndexDeliveryStatus` + `runSeoIndexRepairReplay`).
- `D6` closed: owner-side remediation widgets (`rustok-seo-admin-support`), shared widget state contract and host-locale wiring in `pages/product/blog/forum` + Next admin operator parity.
- `A-C` (D7): closed — runtime data plumbing, Next cutover, route ownership guardrails and cross-host fixture parity.
- `D-E` (D8/D9): open only on live runtime/CI closeout; compile-free baseline expanded to semantic error parity, live evidence capture template, concrete live artifact templates, incident evidence template and owner closeout criteria.

## D8/D9 readiness evidence

- Compile-free D8/D9 seed lives in `apps/next-frontend/contracts/seo/runtime-parity-fixtures.json` and is checked by `npm --prefix apps/next-frontend run verify:seo-runtime-fixtures`.
- The fixture now covers fallback behavior, route ownership, non-home metadata smoke assertions, long-tail diff allowlist, docs sync matrix, owner sign-off checklist, live evidence closeout criteria, semantic error parity, incident evidence templates, owner closeout criteria, concrete per-file live artifact templates, and static source assertions for Next runtime/metadata/transport, Rust renderer, Next Admin index transport, and Leptos storefront SEO runtime wiring.
- Live runtime evidence remains required before final D8/D9 closeout: backend GraphQL/REST parity, SEO index delivery counters, Next robots/sitemap/metadata runtime smoke, Leptos `storefront/seo-page-context` smoke, media descriptor fallback smoke and signed owner review notes.

## Verification

- `cargo xtask module validate seo`
- `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
- `cargo test -p rustok-seo --lib sitemaps`
- `cargo check -p rustok-seo-admin --features ssr --config profile.dev.debug=0`
- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- `cargo check -p rustok-outbox --tests --config profile.dev.debug=0`
- `cargo check -p rustok-index --tests --config profile.dev.debug=0`
- `cargo check -p rustok-admin --lib --config profile.dev.debug=0`
- `cargo check -p rustok-storefront --config profile.dev.debug=0`
- `cargo check -p rustok-server --lib --config profile.dev.debug=0`
- `npm --prefix apps/next-admin run lint && npm --prefix apps/next-admin run typecheck`
- `npm --prefix apps/next-frontend run lint && npm --prefix apps/next-frontend run typecheck`

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Runbook replay/repair](./replay-repair-runbook.md)
- [Operational runbook](./operations-runbook.md)
- [`rustok-seo-render` documentation](../render/docs/README.md)
- [`rustok-seo-admin-support` documentation](../../rustok-seo-admin-support/docs/README.md)
- [Admin package](../admin/README.md)
- [Storefront contract](../../../docs/UI/storefront.md)
- [i18n architecture](../../../docs/architecture/i18n.md)
