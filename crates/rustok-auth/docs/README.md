# Документация `rustok-auth`

`rustok-auth` — core-модуль аутентификации платформы. Он держит JWT lifecycle,
credential hashing, refresh/reset/invite/email-verification token flows и
runtime RBAC surface `users:*`.

## Назначение

- держать auth domain logic вне `apps/server`;
- публиковать канонический runtime entry type `AuthModule`;
- давать платформе единый контракт для токенов, claims и credential helpers.

## Зона ответственности

- конфигурация auth и JWT-алгоритмов;
- encode/decode helpers для access/reset/invite/email-verification token flows;
- password hashing, verify и refresh-token helpers;
- auth-owned migrations;
- публикация permission surface `users:*` через `RusToKModule::permissions()`.

## Интеграция

- зависит только от `rustok-core` и общих библиотек, без зависимости на `rustok-rbac`;
- используется `apps/server` для REST, GraphQL, session lifecycle и user-management flow;
- не публикует собственный UI и остаётся `ui_classification = "capability_only"`;
- email delivery и transport wiring остаются responsibility host-слоя и соседних модулей.

## Проверка

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- targeted server tests для auth/RBAC contracts при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
