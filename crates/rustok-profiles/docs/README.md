# Документация `rustok-profiles`

`rustok-profiles` — доменный модуль универсального публичного профиля
пользователя для RusToK. Он задаёт profile boundary поверх platform `users`,
не смешивая auth identity, customer и будущие seller/merchant surfaces.

## Назначение

- публиковать канонический profile runtime contract для public profile и author/member summary;
- держать storage, service layer и transport boundary профилей внутри отдельного модуля;
- давать downstream-модулям единый источник author/member presentation без прямой зависимости на `users`.

## Зона ответственности

- profile aggregate: `profiles`, `profile_translations`, `profile_tags`;
- `ProfileService`, `ProfilesReader`, `ProfileSummary` и related DTO/enum contracts;
- public handle, display name, bio, avatar/banner references, locale и visibility policy;
- GraphQL read/write surfaces для public profile lookup и self-service edit path;
- event contract `profile.updated` и backfill path для существующих пользователей.

## Интеграция

- `users` остаётся identity/security boundary и не превращается в public profile source;
- `rustok-customer` остаётся отдельным commerce-domain профилем с optional linkage на `user_id`;
- `rustok-blog` и `rustok-forum` уже используют `ProfilesReader` для author presentation;
- `rustok-taxonomy` даёт shared dictionary для `profile_tags`, но ownership привязок остаётся у модуля профилей.

## Проверка

- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- targeted tests для handle policy, locale fallback, summary batching, GraphQL self-service path и profile backfill

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Карта документации платформы](../../../docs/index.md)
