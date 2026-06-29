# Документация `rustok-auth`

`rustok-auth` — core-модуль аутентификации платформы. Он держит JWT lifecycle,
credential hashing, refresh/reset/invite/email-verification token flows и
runtime RBAC surface `users:*`.

## Назначение

- держать auth domain logic вне `apps/server`;
- публиковать канонический runtime entry type `AuthModule`;
- давать платформе единый контракт для токенов, claims и credential helpers.

## Зона ответственности

- конфигурация auth, JWT-алгоритмов и host-provided override assembly/validation;
- encode/decode helpers для access/reset/invite/email-verification token flows;
- password hashing, verify и refresh-token helpers;
- auth-owned migrations;
- публикация permission surface `users:*` через `AUTH_USER_PERMISSIONS` и `RusToKModule::permissions()`.
- typed application boundaries `UserAdminMutationPort` и `OAuthAdminMutationPort` для admin mutations без зависимости module crate от host transport.

## Интеграция

- зависит только от `rustok-core` и общих библиотек, без зависимости на `rustok-rbac`;
- используется `apps/server` для REST, GraphQL, session lifecycle и user-management flow;
- `apps/server` сверяет registry wiring и GraphQL security hints с `AUTH_USER_PERMISSIONS`, чтобы host-слой не расходился с auth-owned permission surface;
- `apps/server` реализует mutation ports поверх существующих auth lifecycle/OAuth services и регистрирует providers в shared runtime extensions; GraphQL и native `#[server]` adapters должны потреблять один provider для каждого boundary;
- публикует собственный UI через подпакет `crates/rustok-auth/admin` с `ui_classification = "admin_only"`;
- email delivery и transport wiring остаются responsibility host-слоя и соседних модулей.

## Поверхность config lifecycle

Каноническая сборка `AuthConfig` выполняется через `build_auth_config` /
`build_auth_config_with_env`: host передаёт Loco/другой framework config, а
`rustok-auth` применяет defaults, `AuthSettingsOverrides`, RS256 env key
resolution и validation. `apps/server` не должен дублировать эти правила, а
только маппить `AuthError` в transport-specific error type.

## Поверхность token lifecycle

Канонический набор auth-owned token helpers:

- access tokens: `encode_access_token`, `decode_access_token`;
- OAuth access tokens: `encode_oauth_access_token`;
- password reset tokens: `encode_password_reset_token`, `decode_password_reset_token`;
- email verification tokens: `encode_email_verification_token`, `decode_email_verification_token`;
- invite tokens: `encode_invite_token`, `decode_invite_token`.

Special-purpose tokens содержат строгий claim `purpose`, используют общую JWT-валидацию
`issuer`/`audience` и нормализуют email-subject в lowercase перед выпуском.
Host-слой (`apps/server`) должен публиковать transport endpoints только через эти
helpers, чтобы invite/reset/verification flows оставались auth-owned.

## Runtime-набор permissions

Канонический набор permissions, принадлежащих auth-модулю:

- `users:create`
- `users:read`
- `users:update`
- `users:delete`
- `users:list`
- `users:manage`

При добавлении, удалении или переименовании permissions нужно менять `AUTH_USER_PERMISSIONS`, `AuthModule::permissions()`, server registry/security-тесты и этот документ в одном инкременте.

## Incident response

Primary owner для auth/JWT/RBAC инцидентов — Platform security/auth on-call. Escalation path: владелец `crates/rustok-auth`, затем владелец server API surface.

При деградации auth:

1. Проверить `/health/ready`, `email_backend` и последние auth/API ошибки без логирования секретов, reset/invite tokens или refresh tokens.
2. Сверить effective `AuthConfig`: algorithm/key pairing, issuer, audience, TTL bounds и production policy.
3. Если проблема связана с email reset/verification delivery, эскалировать также владельцу host email transport.
4. Если проблема связана с RBAC, сверить `AUTH_USER_PERMISSIONS`, server registry/security hints и фактические transport guards.
5. После rollback сохранить evidence: artifact id, config snapshot без секретов, affected flows, health snapshot и список revoked/rotated credentials, если rotation выполнялась.

## Проверка

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- targeted server tests для auth/RBAC contracts при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
