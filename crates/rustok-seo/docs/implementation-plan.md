# План реализации `rustok-seo`

Статус: SEO Suite v1 уже собран как один optional module; текущий план фиксирует bootstrap, storefront/runtime integration и следующий отложенный scope.

## Область работ

- удерживать `rustok-seo` как единый tenant-aware SEO runtime вместо набора разрозненных модулей;
- синхронизировать metadata precedence, redirects, sitemap/robots и storefront SEO contract между server и фронтендами;
- держать ownership UI согласованным с платформенной модульностью: entity SEO authoring должен жить
  в owner-module UI, а `rustok-seo-admin` — только в cross-cutting SEO infrastructure surfaces;
- не допускать destructive migration legacy domain SEO fields, пока explicit SEO layer работает в dual-read режиме.

## Текущее состояние

- module bootstrap уже проведён: `modules.toml`, `apps/server`, миграции, permissions и local docs подключены;
- explicit SEO runtime уже использует `meta` / `meta_translations` плюс новые таблицы `seo_redirects`, `seo_revisions`, `seo_sitemap_jobs`, `seo_sitemap_files`;
- locale columns в `meta_translations`, `content_canonical_urls`, `content_url_aliases` уже расширены до `VARCHAR(32)`;
- locale widening rollback зафиксирован как forward-only: миграции не должны сужать SEO locale columns назад и рисковать обрезанием `pt-BR` / `zh-Hant` / других BCP47-like тегов;
- storefront SEO read-side уже переведён на постоянный canonical contract `SeoPageContext = route + document`, без versioned DTO-вариантов;
- Rust-side SSR head rendering уже вынесен в companion support crate `rustok-seo-render`, чтобы host не дублировал tag serialization;
- internal service monolith уже разрезан на подсистемы `meta`, `routing`, `redirects`, `sitemaps`, `robots` и helper-слой target loading внутри `services/`;
- `rustok-seo-admin` уже разрезан на стандартный module-owned UI layout `lib/component/model/api/i18n/sections`, так что route shell, form/view-model слой и tab-specific view panels отделены друг от друга;
- SEO admin UI уже собран как route-driven shell c отдельными section components, а central metadata editor из него уже удалён после cutover entity SEO authoring в owner-module UI;
- SEO admin route теперь использует только host-owned query key `tab` для control-plane navigation и больше не держит entity selection contract внутри `rustok-seo-admin`;
- `apps/storefront` уже потребляет этот contract для SSR head generation, redirect preflight и locale normalization через shared `rustok_core::normalize_locale_tag`;
- `apps/next-frontend` уже держит shared metadata builder поверх built-in Metadata API, `robots.ts` и `sitemap.ts` foundation без искусственного расширения route coverage.

## Этапы

### 1. Core runtime

- [x] провести module bootstrap и manifest-driven wiring;
- [x] зафиксировать `SeoTargetKind = page | product | blog_post | forum_category | forum_topic`;
- [x] реализовать metadata precedence: explicit SEO -> domain fallback -> entity fallback;
- [x] расширить locale storage до `VARCHAR(32)` для SEO-related tables.

### 2. Public surfaces

- [x] поднять GraphQL contract для metadata, redirects и sitemap lifecycle;
- [x] добавить HTTP endpoints `/robots.txt`, `/sitemap.xml`, `/sitemaps/{name}`;
- [x] добавить headless REST read path `GET /api/seo/page-context?route=...` поверх canonical request locale resolution;
- [x] сделать forum topic resolution channel-aware на REST/GraphQL/Leptos SSR read paths через host-provided request channel slug;
- [x] добавить Leptos `#[server]` functions для module-owned admin flows;
- [x] заменить плоский storefront SEO DTO на постоянный nested contract `route + document` с typed robots/OG/Twitter/verification/JSON-LD blocks;
- [x] разрезать internal `SeoService` на подсистемы `meta`, `routing`, `redirects`, `sitemaps`, `robots` без смены public API;
- [x] разрезать `rustok-seo-admin` на стандартный package layout `lib/component/model/api/i18n` без смены manifest wiring и public export;
- [x] перевести `seo` admin route на URL-owned control-plane navigation через typed `tab`;
- [x] довести SEO-owned infrastructure UI до production-grade editor ergonomics;
- [x] добавить shared SEO UI support/widgets crate `rustok-seo-admin-support` для owner-module editor surfaces;
- [x] перенести entity SEO authoring в `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`;
- [x] добавить owner-side SEO capability slots в `rustok-forum/admin` без central editor fallback;
- [x] убрать central metadata editor из `rustok-seo-admin` и оставить infrastructure-only control-plane surface;
- [x] расширить `rustok-seo-admin` до robots / defaults / diagnostics panes.

### 3. Host integration

- [x] встроить SEO SSR preflight в `apps/storefront`;
- [x] вынести Rust-side head renderer в `rustok-seo-render` и перевести `apps/storefront` на shared renderer;
- [x] дать `apps/next-frontend` shared metadata foundation, `robots.ts` и `sitemap.ts`;
- [x] выровнять host locale normalization с platform i18n contract (`VARCHAR(32)` / `normalize_locale_tag`);
- [x] убрать package-local locale override из `rustok-seo-admin-support` и перевести owner-side SEO panels на host locale without editable locale field;
- [ ] расширять Next route coverage только вместе с появлением реальных storefront routes;
- [x] добавить targeted parity tests для GraphQL и native `#[server]` paths.

### 4. Follow-up scope

- [ ] bulk editor;
- [ ] AI assist / generation;
- [ ] analytics и search-console connectors;
- [x] включить forum targets в `rustok-seo` и заменить capability slot в `rustok-forum/admin` на реальный entity editor;
- [ ] оценить переход от закрытого `SeoTargetKind` enum к registry-backed target capability, если SEO authoring понадобится новым модулям без core change в `rustok-seo`.

## Проверка

- `cargo xtask module validate seo`
- `cargo check -p rustok-seo`
- `cargo check -p rustok-seo-render`
- `cargo check -p rustok-seo-admin-support`
- `cargo check -p rustok-pages-admin -p rustok-blog-admin -p rustok-product-admin -p rustok-forum-admin`
- `cargo check -p rustok-admin`
- `cargo check -p rustok-storefront`
- `cargo check -p rustok-server`
- `npm.cmd --prefix apps/next-frontend run typecheck`

## Правила обновления

1. При изменении SEO runtime contract сначала обновлять этот файл.
2. При изменении public/storefront surfaces синхронизировать `README.md` и `docs/README.md`.
3. При изменении module wiring, permissions или UI classification синхронизировать `rustok-module.toml`, `modules.toml` и central docs.
4. При изменении multilingual fallback semantics синхронизировать SEO docs с `docs/architecture/i18n.md` и storefront host docs.
