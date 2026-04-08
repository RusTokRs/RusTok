# План реализации `rustok-blog`

Статус: blog-owned storage и transport surfaces уже зафиксированы; модуль
удерживается в режиме post-split hardening и product semantics rollout.

## Область работ

- удерживать `rustok-blog` как самостоятельный blog domain module;
- синхронизировать post/category/tag/comment contracts, UI packages и local docs;
- развивать channel-aware и taxonomy-aware semantics без возврата к shared content storage.

## Текущее состояние

- blog posts, translations, categories и typed tag relations уже живут в module-owned storage;
- GraphQL/REST adapters и Leptos admin/storefront surfaces уже живут внутри модуля;
- comments runtime contract приходит из `rustok-comments`, а author presentation — из `rustok-profiles`;
- public read-path уже поддерживает module-level и publication-level channel visibility.

## Этапы

### 1. Contract stability

- [x] закрыть storage split и blog-owned transport boundary;
- [x] перенести tag vocabulary на shared `rustok-taxonomy`, сохранив blog-owned attachments;
- [x] встроить channel-aware public visibility contract;
- [ ] удерживать sync между runtime contracts, UI packages и module metadata.

### 2. Product hardening

- [ ] довести rate limiting и performance baseline для public/write paths;
- [ ] довести search/index integration без размывания blog domain boundary;
- [ ] удерживать category/tag/comment semantics покрытыми targeted integration tests.

### 3. Operability

- [ ] развивать observability для post lifecycle, visibility filtering и moderation flows;
- [ ] документировать новые public/runtime guarantees одновременно с изменением сервисов;
- [ ] держать локальные docs, README и manifest metadata синхронизированными.

## Проверка

- `cargo xtask module validate blog`
- `cargo xtask module test blog`
- targeted tests для lifecycle, taxonomy sync, channel visibility и UI-facing read contracts

## Правила обновления

1. При изменении blog runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении dependency graph, UI wiring или metadata синхронизировать `rustok-module.toml`.
4. При изменении channel/tag semantics обновлять также связанные module docs и central references.
