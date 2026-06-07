# Документация `rustok-seo`

`rustok-seo` — optional module платформы для headless SEO runtime и cross-cutting SEO control-plane. Модуль владеет tenant-scoped SEO metadata, template-generated SEO, bulk remediation, redirects, sitemap/robots generation, diagnostics и storefront-facing `SeoPageContext`.

Entity-specific SEO authoring не живёт в `rustok-seo-admin`: страницы, продукты, блог и форум встраивают SEO panels в собственные module-owned admin surfaces через `rustok-seo-admin-support`.

## Назначение

Назначение модуля — дать платформе единый typed SEO runtime: owner modules публикуют target records и безопасные поля для templates, а `rustok-seo` централизованно строит effective metadata, bulk remediation, diagnostics и public/storefront read-side.

## Зона ответственности

- canonical read contract `SeoPageContext = route + document`, где `route` отвечает за locale/canonical/redirect/hreflang, а `document` — за typed head metadata;
- metadata precedence: explicit SEO > template-generated SEO > domain/entity fallback;
- tenant-scoped `template_defaults` и per-target `template_overrides`;
- bulk editor и remediation jobs поверх `seo_bulk_jobs`, `seo_bulk_job_items`, `seo_bulk_job_artifacts`;
- manual redirects, sitemap jobs/files и `robots.txt`;
- runtime adapter seam для sitemap submission с per-endpoint статусами и bounded partial-failure summary;
- diagnostics read model: readiness score, issue list, issue aggregates и source counts, включая image descriptor quality issue codes `missing_image_alt` и `missing_image_size` для SEO-critical targets;
- read-only cross-link suggestions (`seoCrossLinkSuggestions` / `/api/seo/cross-link-suggestions`) без автоматической HTML mutation;
- REST control-plane parity endpoints для диагностики/карт сайта/bulk jobs: `/api/seo/diagnostics`, `/api/seo/sitemaps/status`, `/api/seo/sitemaps/jobs`, `/api/seo/sitemaps/jobs/{job_id}`, `/api/seo/bulk/jobs`, `/api/seo/bulk/jobs/{job_id}`;
- REST error envelope на control-plane endpoint-ах унифицирован с GraphQL кодами (`errors[].extensions.code`: `BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, `INTERNAL_ERROR`) для deterministic client-side mapping;
- shared capability registry через `rustok-seo-targets`;
- support crates `rustok-seo-render` и `rustok-seo-admin-support`;
- execution wave Phase D: typed SEO events/outbox/index seam, REST parity completion, admin/host integration parity, verification matrix и runbooks.

## Шаблонная генерация SEO

Owner modules не рендерят SEO templates сами. Они отдают только typed `SeoLoadedTargetRecord.template_fields` через `rustok-seo-targets`; в map допускаются SEO-safe поля вроде `title`, `description`, `route`, `locale`, slug/handle/id.

`rustok-seo` централизованно рендерит:

- `title`;
- `meta_description`;
- `canonical_url`;
- `keywords`;
- `robots`;
- Open Graph title/description;
- Twitter title/description.

`SeoPageContext.document.effective_state` и `seoMeta.effective_state` показывают source каждого effective значения: `explicit`, `generated` или `fallback`. Это нужно, чтобы admin UI не смешивал user-authored overrides и synthesis из шаблонов.

## Rich snippets и typed schema blocks

`SeoDocument.structured_data_blocks` является canonical runtime layer для JSON-LD. Storage по-прежнему принимает `seo_meta.structured_data` как JSON payload, но read-side не отдаёт его как неразмеченный blob:

- `schema_kind` — canonical enum для поддерживаемых schema.org shapes (`product`, `offer`, `aggregate_rating`, `breadcrumb_list`, `item_list`, `organization`, `local_business`, `web_site`, `search_action`, `article`, `blog_posting`, `faq_page`, `how_to`, `discussion_forum_posting` и другие);
- `schema_type` — исходный `@type` из JSON-LD;
- `kind` — legacy string alias для текущих headless consumers;
- `source` — `explicit`, `generated` или `fallback`, синхронизированный с effective SEO state;
- `payload` — JSON-LD payload, который рендерится как `<script type="application/ld+json">`.

Если payload содержит `@graph`, runtime разворачивает граф в отдельные schema blocks и наследует `@context`. Diagnostics считает schema отсутствующей, если typed blocks не получились, и отдельно помечает блоки без распознанного schema.org type как `unknown_schema_type`.

Explicit write paths (`upsertSeoMeta`, Leptos server functions и bulk apply) валидируют новые `structured_data` значения как JSON-LD. Payload должен быть object, array или `@graph` минимум с одним непустым `@type`; будущие schema.org типы допускаются как `other`, но untyped JSON/scalars отклоняются.

Built-in owner providers (`pages/product/blog/forum`) генерируют fallback structured data через `rustok-seo-targets::schema` builders. Это сохраняет module ownership, но не даёт каждому provider hand-roll-ить собственный raw `json!` shape.

## Media image descriptor boundary (C3)

Image metadata boundary зафиксирован между `rustok-media` и `rustok-seo`:

- `rustok-media` публикует typed contract `MediaImageDescriptor` (`url`, `alt`, `width`, `height`, `mime_type` + derived helpers вроде `has_alt`, `has_size`, `pixel_count`, `aspect_ratio`, `file_extension`);
- owner SEO providers (`pages/product/blog/forum`) заполняют OG/Twitter/schema fallback и image template fields через эти descriptors;
- `rustok-seo` не читает raw media blobs и работает только с descriptor payload в `SeoTargetOpenGraphRecord.images`.

## Bulk remediation

Bulk apply больше не является простым overwrite job. Каждый apply job обязан выбрать режим:

- `preview_only` — только строит preview artifact с effective SEO, без записи `meta`;
- `apply_missing_only` — материализует missing/generated/fallback SEO в explicit records, но не перетирает existing explicit SEO;
- `overwrite_generated_only` — пишет только targets, чей текущий source равен `generated`;
- `force_overwrite_explicit` — разрешённый operator override explicit SEO, требует реальный patch delta.

CSV export/import остаются scoped по одному `SeoTargetSlug` и одному locale. Artifacts скачиваются через tenant/RBAC-checked SEO endpoint, без filesystem leakage.

## Sitemap submission semantics (C1)

Sitemap submit orchestration остаётся внутренним runtime concern и не меняет public shape `SeoSitemapStatusRecord`, но теперь ведёт telemetry-friendly aggregation:

- фиксируется per-endpoint статус (`success`, `failure`, `timeout`, `invalid_endpoint`);
- partial failure считается **job success + submission failure summary**: sitemap files уже сгенерированы, но `last_error` job хранит bounded aggregate message;
- deterministic truncation policy использует `max_errors` и `max_timeout_details`, ordering всегда стабильный по endpoint;
- invalid endpoints пропускаются на adapter layer и учитываются как failure без HTTP submit.

## Diagnostics

`seoDiagnostics` и admin diagnostics pane строят tenant-level summary по target registry:

- missing title / description;
- duplicate canonical URL;
- noindex + canonical conflicts;
- canonical URLs pointing to redirect targets, chains or loops;
- missing hreflang alternates and missing `x-default`;
- missing typed schema blocks and unknown schema.org types;
- missing sitemap candidates;
- fallback-only targets, где policy ожидает template или explicit SEO;
- `cross_link_gap` для targets без read-only cross-link suggestions с remediation entrypoint через `seoCrossLinkSuggestions`/`/api/seo/cross-link-suggestions`;
- `missing_image_alt` и `missing_image_size` для SEO-critical targets, где OG/Twitter images не имеют полного descriptor metadata.

Readiness score считается производным от issue set. Summary также отдаёт counts by issue code и target kind, чтобы admin UI мог строить фильтры и remediation entrypoints без локальной классификации ошибок. Diagnostics не заменяет owner-module editors, а даёт entrypoint для remediation.

## Интеграция

- `apps/storefront` потребляет `SeoPageContext.route + document` через `rustok-seo-render` для SSR `<title>`, meta description, canonical, robots, hreflang, Open Graph, Twitter, verification tags, pagination links и JSON-LD.
- `apps/next-frontend` использует shared SEO adapter поверх Next Metadata API; unsupported long-tail tags остаются в canonical contract.
- `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin` и `rustok-forum/admin` являются canonical owner surfaces для entity SEO authoring.
- Host runtime обязан прокидывать `ModuleRuntimeExtensions` с `SeoTargetRegistry` во все SEO entrypoints; built-in registry допустим только в tests/helpers.

## Phase D roadmap (productionization)

Текущий roadmap зафиксирован в `docs/implementation-plan.md` и выполняется батчами `D1..D9`.

- `D1` закрыт: contract freeze, compatibility policy (`v1 additive only`) и rollout flags.
- `D2-D3`: typed SEO events, outbox emission/idempotency, SEO->index consumer seam — закрыто.
- `D4-D5`: GraphQL/REST parity completion и migrations/backfill/replay policy — закрыто (включая index tracking/replay endpoints `/api/seo/index/tracking`, `/api/seo/index/repair-replay`, GraphQL `seoIndexDeliveryStatus` + `runSeoIndexRepairReplay`).
- `D6-D7`: Leptos admin + Next admin control-plane parity, owner-side remediation widgets, storefront + Next runtime SEO parity — в работе.
- `D8-D9`: verification matrix, runbooks, Definition of Ready/Done для следующего execution wave — в работе.

## Проверка

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

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Runbook replay/repair](./replay-repair-runbook.md)
- [Документация `rustok-seo-render`](../render/docs/README.md)
- [Документация `rustok-seo-admin-support`](../../rustok-seo-admin-support/docs/README.md)
- [Admin package](../admin/README.md)
- [Контракт storefront](../../../docs/UI/storefront.md)
- [Архитектура i18n](../../../docs/architecture/i18n.md)
