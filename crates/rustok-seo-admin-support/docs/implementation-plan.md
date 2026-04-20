# План реализации `rustok-seo-admin-support`

Статус: базовый support crate уже введён и используется owner-module admin пакетами.

## Цель

- не дублировать SEO panel logic в `pages`, `product`, `blog` и будущих content-модулях;
- не превращать `rustok-seo-admin` в universal entity editor;
- держать reusable UI/tooling слой отдельно от SEO runtime и от owner-module screen ownership.

## Выполнено

- [x] создан support crate с root README и local docs;
- [x] вынесены shared GraphQL helper-ы для `seoMeta`, `upsertSeoMeta`, `publishSeoRevision`;
- [x] реализован `SeoEntityPanel` для owner-side entity editors;
- [x] реализован `SeoCapabilityNotice` для модулей, где capability slot нужен раньше, чем runtime target support;
- [x] встроены owner-side SEO panels в `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`;
- [x] убран package-local locale override: support crate читает host effective locale, canonicalizes его и не держит editable locale field в panel UI.

## Осталось

- [x] вынести typed snippet preview и diagnostics cards в отдельные reusable subcomponents;
- [x] добавить targeted tests для panel transport, scoring logic и host-locale wiring.

## Проверка

- `cargo check -p rustok-seo-admin-support`
- `cargo check -p rustok-pages-admin`
- `cargo check -p rustok-blog-admin`
- `cargo check -p rustok-product-admin`
- `cargo check -p rustok-forum-admin`
