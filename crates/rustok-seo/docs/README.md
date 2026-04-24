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
- diagnostics read model: readiness score, issue list, issue aggregates и source counts;
- shared capability registry через `rustok-seo-targets`;
- support crates `rustok-seo-render` и `rustok-seo-admin-support`.

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

## Bulk remediation

Bulk apply больше не является простым overwrite job. Каждый apply job обязан выбрать режим:

- `preview_only` — только строит preview artifact с effective SEO, без записи `meta`;
- `apply_missing_only` — материализует missing/generated/fallback SEO в explicit records, но не перетирает existing explicit SEO;
- `overwrite_generated_only` — пишет только targets, чей текущий source равен `generated`;
- `force_overwrite_explicit` — разрешённый operator override explicit SEO, требует реальный patch delta.

CSV export/import остаются scoped по одному `SeoTargetSlug` и одному locale. Artifacts скачиваются через tenant/RBAC-checked SEO endpoint, без filesystem leakage.

## Diagnostics

`seoDiagnostics` и admin diagnostics pane строят tenant-level summary по target registry:

- missing title / description;
- duplicate canonical URL;
- noindex + canonical conflicts;
- canonical URLs pointing to redirect targets, chains or loops;
- missing hreflang alternates and missing `x-default`;
- missing structured data;
- missing sitemap candidates;
- fallback-only targets, где policy ожидает template или explicit SEO.

Readiness score считается производным от issue set. Summary также отдаёт counts by issue code и target kind, чтобы admin UI мог строить фильтры и remediation entrypoints без локальной классификации ошибок. Diagnostics не заменяет owner-module editors, а даёт entrypoint для remediation.

## Интеграция

- `apps/storefront` потребляет `SeoPageContext.route + document` через `rustok-seo-render` для SSR `<title>`, meta description, canonical, robots, hreflang, Open Graph, Twitter, verification tags, pagination links и JSON-LD.
- `apps/next-frontend` использует shared SEO adapter поверх Next Metadata API; unsupported long-tail tags остаются в canonical contract.
- `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin` и `rustok-forum/admin` являются canonical owner surfaces для entity SEO authoring.
- Host runtime обязан прокидывать `ModuleRuntimeExtensions` с `SeoTargetRegistry` во все SEO entrypoints; built-in registry допустим только в tests/helpers.

## Проверка

- `cargo xtask module validate seo`
- `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
- `cargo check -p rustok-seo-admin --features ssr --config profile.dev.debug=0`
- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- `cargo check -p rustok-admin --lib --config profile.dev.debug=0`
- `cargo check -p rustok-storefront --config profile.dev.debug=0`
- `cargo check -p rustok-server --lib --config profile.dev.debug=0`

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Документация `rustok-seo-render`](../render/docs/README.md)
- [Документация `rustok-seo-admin-support`](../../rustok-seo-admin-support/docs/README.md)
- [Admin package](../admin/README.md)
- [Контракт storefront](../../../docs/UI/storefront.md)
- [Архитектура i18n](../../../docs/architecture/i18n.md)
