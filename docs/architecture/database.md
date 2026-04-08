# Схема данных платформы

Этот документ даёт верхнеуровневую карту write-side и read-side схем RusToK.
Он не заменяет migrations, entities и local docs модулей.

## Источники истины

Канонический источник правды по структуре БД:

- migrations в `apps/server/migration` и module-owned migration sources
- SeaORM entities в `apps/server/src/models/_entities` и module crates
- local docs модулей для module-owned storage contracts

Этот документ нужен только как central summary layer.

## Общие инварианты

- `tenant_id` остаётся главным изоляционным boundary для platform и module data
- write-side таблицы считаются source of truth для доменного состояния
- denormalized index/read tables не считаются source of truth
- JSONB допустим для settings, config и гибких payload-ов, но не как конечная
  canonical форма для локализованного бизнес-текста

## Мультиязычный storage-контракт

Текущий целевой паттерн платформы:

- базовые business tables хранят language-agnostic state
- локализованные короткие тексты живут в `*_translations`
- тяжёлый локализованный контент может жить в `*_bodies`
- tenant locale policy управляет effective locale и fallback, но не ownership
  локализованных полей
- хранение locale следует единому normalized contract с безопасной шириной
  `VARCHAR(32)`

Если старый модуль использует смешанный storage pattern, это считается
миграционным target, а не желаемым эталоном.

## Foundation-таблицы

Foundation storage включает:

- `tenants`
- `users`
- `sessions`
- `platform_settings`
- `tenant_modules`
- `tenant_locales`
- `oauth_apps`
- `sys_events`

### Что здесь важно

- `tenants` и `tenant_locales` задают tenant и locale policy layer
- `sessions` и auth-related tables поддерживают auth/session lifecycle
- `platform_settings` и `tenant_modules` хранят platform/module settings
- `sys_events` остаётся transactional outbox table, а не generic audit dump

## RBAC-таблицы

RBAC source of truth живёт в relation tables:

- `roles`
- `permissions`
- `user_roles`
- `role_permissions`

Они поддерживают permission/runtime contract и не должны дублироваться в
альтернативных ownership tables без явной архитектурной причины.

## Хранение content-family

Текущий content baseline строится вокруг:

- `nodes`
- `node_translations`
- `bodies`

Принцип:

- `nodes` владеет language-agnostic state
- `node_translations` владеет локализованными короткими полями
- `bodies` владеет heavy localized content

## Хранение commerce-family

Commerce storage остаётся split-domain family, но верхнеуровневой базой служат:

- `products`
- `product_translations`
- `product_variants`
- `variant_translations`
- `prices`
- `product_images`
- `product_options`

И здесь действует тот же принцип: base rows language-agnostic, локализованные
поля вынесены в parallel records.

## Flex

`flex` — capability slice, но он подчиняется тому же storage contract.

Текущий live baseline включает:

- `flex_schemas`
- `flex_schema_translations`
- `flex_entries`
- `flex_attached_localized_values`

Current-state вывод:

- schema-level language-agnostic state живёт в base tables
- localized schema copy живёт в translations tables
- attached localized values вынесены в dedicated locale-aware storage
- legacy inline localized JSON считается transitional fallback, а не конечной
  формой

## Index/read-side таблицы

`rustok-index` владеет denormalized read models, например:

- `index_content`
- `index_products`

Они существуют для query/index/search paths и не должны использоваться как
авторитетный write-side storage.

## Workflow-хранение

`rustok-workflow` владеет собственным module storage:

- `workflows`
- `workflow_steps`
- `workflow_executions`
- `workflow_step_executions`
- `workflow_versions`

Это module-owned schema, а не generic platform queue layer.

## Media и storage-слой

Media metadata остаётся module-owned, а file bytes обрабатываются через shared
storage runtime.

Базовые media tables:

- `media`
- `media_translations`

Storage backend configuration при этом живёт не в per-file SQL contract, а в
typed runtime settings.

## Что не делать

- не считать summary-документ заменой migrations
- не использовать read-side tables как write-side authority
- не хранить локализованный бизнес-текст в base rows, если модуль уже идёт по
  parallel-localized-record path
- не размывать ownership module-owned tables между host и module crate

## Связанные документы

- [Архитектура модулей](./modules.md)
- [Контракт потока доменных событий](./event-flow-contract.md)
- [Архитектура i18n](./i18n.md)
- [Реестр модулей и приложений](../modules/registry.md)
