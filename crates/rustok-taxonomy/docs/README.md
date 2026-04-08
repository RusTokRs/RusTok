# Документация `rustok-taxonomy`

`rustok-taxonomy` — shared vocabulary module платформы. Он владеет dictionary
layer для terms, translations и aliases, но не забирает ownership доменных
attachment tables у blog/forum/product/profile и других модулей.

## Назначение

- публиковать канонический taxonomy dictionary contract;
- держать term identity, localized labels/slugs и scope rules внутри модуля;
- давать доменным модулям shared vocabulary без возврата к polymorphic shared product storage.

## Зона ответственности

- `taxonomy_terms`, `taxonomy_term_translations`, `taxonomy_term_aliases`;
- tenant-scoped term identity и `canonical_key`;
- scope contract для `global` и `module` terms;
- alias-aware lookup и module integration helpers;
- отсутствие ownership над relation tables вроде `blog_post_tags` или `forum_topic_tags`.

## Интеграция

- `rustok-blog`, `rustok-forum`, `rustok-product` и `rustok-profiles` используют taxonomy как shared dictionary;
- attachment ownership и public domain contracts остаются внутри owning modules;
- locale normalization и fallback должны оставаться синхронизированными с shared `rustok-content` contract;
- любые новые taxonomy consumers должны входить через explicit module-owned relation tables.

## Проверка

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- targeted tests для term CRUD, scope rules, alias lookup и consumer-module integration helpers

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
