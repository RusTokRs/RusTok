# Документация `rustok-seo`

`rustok-seo` — optional module платформы, который собирает explicit SEO metadata, redirect runtime, sitemap/robots generation и storefront SEO read contract в одном месте.

## Назначение

- публиковать tenant-aware SEO contract для `page`, `product`, `blog_post`, `forum_category` и `forum_topic`;
- держать SEO-owned infrastructure surface для redirects, sitemap/robots и global SEO policy;
- давать owner-модулям shared SEO capability contract и переиспользуемые UI/widget hooks для
  встраивания SEO в их собственные editor surfaces;
- отдавать host storefront-ам общий `SeoPageContext` для SSR metadata generation без дублирования SEO-логики по фронтендам.

## Зона ответственности

- dual-read metadata precedence: explicit `meta/meta_translations` -> domain SEO fields -> entity fallback;
- reuse существующего routing substrate `content_canonical_urls` / `content_url_aliases`;
- manual redirects, sitemap jobs/files и `robots.txt`;
- canonical storefront read contract `SeoPageContext = route + document`, где route-часть отвечает за locale/canonical/redirect/hreflang, а document-часть — за typed head metadata;
- typed document sections для дальнейшего additive growth: `robots`, Open Graph, Twitter, verification, pagination, generic `meta_tags` / `link_tags` и список JSON-LD blocks;
- companion support crate `rustok-seo-render` для Rust-host SSR head rendering без переноса SEO runtime логики в host;
- companion support crate `rustok-seo-admin-support` для owner-module admin panels, transport helper-ов
  и reusable SEO widgets без переноса ownership экрана в `rustok-seo-admin`; support crate при этом
  потребляет host-provided effective locale, canonicalizes BCP47-like tags и не держит свой locale override в panel UI;
- internal service layout уже разрезан по подсистемам `meta`, `routing`, `redirects`, `sitemaps`, `robots` с отдельным helper-слоем target loading, чтобы модуль не оставался одним monolithic service file;
- module-owned admin UI package `rustok-seo-admin` тоже уже приведён к стандартному layout `lib/component/model/api/i18n/sections`, так что route/query shell, form state и tab-specific view panels больше не живут в одном файле;
- canonical ownership теперь зафиксирован так: entity SEO authoring живёт в owner-модулях
  (`pages`, `product`, `blog`, `forum` и будущих content-модулях), а `rustok-seo-admin`
  должен держать только SEO-owned infrastructure UI;
- central metadata editor уже удалён из `rustok-seo-admin`; текущий пакет держит redirects, sitemaps, robots preview, tenant defaults и diagnostics control-plane surface;
- admin route `seo` теперь использует только typed `tab` query key для control-plane navigation и не владеет больше entity selection contract;
- GraphQL contract: `seoPageContext`, `seoMeta`, `upsertSeoMeta`, `publishSeoRevision`, `rollbackSeoRevision`, `seoRedirects`, `upsertSeoRedirect`, `generateSeoSitemaps`, `seoSitemapStatus`;
- headless REST read contract теперь включает `GET /api/seo/page-context?route=...`, который использует canonical server locale resolution через `RequestContext`, а не отдельную SEO-local fallback chain;
- forum topic SEO resolution на REST/GraphQL/Leptos SSR path теперь также учитывает `RequestContext.channel_slug`, поэтому channel-restricted public topics не раскрываются вне совпавшего канала;
- Leptos `#[server]` functions для module-owned admin read/write flows.

## Интеграция

- использует `rustok-content` как routing substrate и canonical URL source;
- `apps/storefront` потребляет `SeoPageContext.route + document` через `rustok-seo-render` для SSR `<title>`, `meta description`, canonical, robots, hreflang, Open Graph, Twitter, verification tags, pagination links и JSON-LD;
- тот же storefront SSR path теперь пробрасывает host request channel slug в `rustok-seo`, чтобы forum SEO contract совпадал с public forum visibility policy;
- `apps/next-frontend` использует shared SEO adapter поверх built-in Next Metadata API; unsupported long-tail tags остаются в canonical contract и не переносятся в host-specific source of truth;
- `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin` и `rustok-forum/admin`
  считаются canonical owner surfaces для entity-specific SEO UI; `rustok-seo-admin`
  не должен оставаться universal editor после cutover;
- owner-side SEO panels уже встроены в `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`
  и `rustok-forum/admin`, а shared runtime уже держит forum target kinds для category/topic routes;
- existing `meta` и `meta_translations` остаются core SEO storage, а locale columns в `meta_translations`, `content_canonical_urls`, `content_url_aliases` расширяются до `VARCHAR(32)`; rollback для locale widening остаётся safe forward-only path и не пытается сужать колонки назад.
- HTML/SSR output остаётся adapter-слоем: `rustok-seo` не зависит от Leptos/Next и продолжает быть headless runtime, который можно читать через GraphQL, `#[server]` и HTTP endpoints для `SeoPageContext`, `robots` и sitemaps.

## Проверка

- `cargo xtask module validate seo`
- `cargo check -p rustok-seo`
- `cargo check -p rustok-seo-render`
- `cargo check -p rustok-admin`
- `cargo check -p rustok-storefront`
- `cargo check -p rustok-server`
- `npm.cmd --prefix apps/next-frontend run typecheck`

## Что пока не входит

- AI generation
- bulk editing
- analytics connectors
- AI/bulk/analytics follow-up без отката уже включённого forum entity SEO cutover

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Документация `rustok-seo-render`](../render/docs/README.md)
- [Документация `rustok-seo-admin-support`](../../rustok-seo-admin-support/docs/README.md)
- [Admin package](../admin/README.md)
- [Контракт storefront](../../../docs/UI/storefront.md)
- [Архитектура i18n](../../../docs/architecture/i18n.md)
