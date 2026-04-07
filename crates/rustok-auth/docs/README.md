# Документация `rustok-auth`

`rustok-auth` — core-модуль аутентификации платформы. Он держит JWT lifecycle, password hashing, refresh/reset/invite/email-verification токены и собственные миграции, а в runtime публикует RBAC surface `users:*`.

## Что находится внутри модуля

- `AuthModule` и runtime metadata для `ModuleRegistry`
- конфигурация токенов и алгоритмов подписи
- credential helpers для hash/verify password и refresh token flow
- claims и encode/decode helpers для access, invite, reset и verification token paths
- auth-owned migrations

## Границы и взаимодействия

- `rustok-auth` зависит только от `rustok-core` и общих библиотек, а не от `rustok-rbac`.
- `apps/server` использует модуль как источник auth primitives для REST, GraphQL и lifecycle flow.
- Email delivery, tenant policy и transport adapters остаются responsibility host-слоя или соседних модулей (`rustok-email`, `apps/server`).
- Модуль не публикует собственный UI и фиксируется как `ui_classification = "capability_only"`.

## Проверка

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- `cargo test -p rustok-server registry_modules_publish_expected_rbac_surface --lib`

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
