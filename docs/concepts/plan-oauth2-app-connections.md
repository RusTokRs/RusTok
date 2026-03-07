# Plan: OAuth2 App Connections (App-подключения)

## Context

RusTok стремится к модели подключения приложений "как у Shopify/Virto": любая админка, фронт, мобилка или внешняя интеграция подключаются через регистрацию Application + выдачу токенов со scope, без "вечных ключей" в `.env`.

Сейчас auth-система user-centric: JWT (HS256, симметричный) + сессии + RBAC (resource:action). Нет OAuth2 Authorization Server, нет сущностей Application/Client/Installation, нет scope-based доступа для приложений. Feature flag `oauth_enabled: false` уже существует в конфиге.

**Решения пользователя**: все 8 этапов за один проход, сразу RS256, consent page — server-rendered HTML с auto-approve для first-party apps.

**Спецификация**: `docs/concepts/app-подключения` (610 строк детальных рекомендаций).

**Связанные планы**: `docs/modules/module-rebuild-plan.md`, `docs/modules/marketplace-plan.md`.

### Выбор библиотек (обоснование)

Проведён анализ Rust-экосистемы OAuth2 Authorization Server библиотек. Итоги:

| Крейт | Назначение | Вердикт |
|-------|-----------|---------|
| `oxide-auth` + `oxide-auth-axum` | AS framework | **Отклонён**: последний релиз июнь 2024, совместимость с axum 0.8.x не гарантирована, не покрывает OIDC/JWT/JWKS/PKCE — всё равно нужен кастом |
| `auth-framework` | Full AS | **Отклонён**: новый, непроверенный, низкое community adoption |
| `oauth2` (oauth2-rs) | OAuth2 Client | Не подходит — только клиентская сторона |
| `openidconnect` | OIDC Client/RP | Не подходит — RP-библиотека, полезна только для OIDC типов |

**Принятые решения по зависимостям:**

| Компонент | Библиотека | Обоснование |
|-----------|-----------|-------------|
| JWT RS256 sign/verify | **`jsonwebtoken` 10.3.0** (уже в workspace) | Без OpenSSL, строго типизированные claims, автовалидация exp/nbf/iss/aud, самый популярный JWT-крейт |
| JWKS endpoint | **`jsonwebtoken::jwk` типы + ручная сериализация** | ~50 строк, не оправдывает отдельный крейт |
| JWK generation из RSA | **`jwk_kit`** (опционально) или ручное извлечение n/e из RSA public key | Рассмотреть при реализации Этапа 0 |
| OAuth2 AS flows | **Кастом** (authorize, token, revoke endpoints) | ~500-700 строк; AS-библиотеки в Rust либо устарели, либо непроверены. Объём кода компактный, полный контроль |
| PKCE (RFC 7636) | **Кастом** (sha2 + base64url) | ~30 строк, тривиальная реализация |
| Resource server middleware | **`tower-oauth2-resource-server`** (рассмотреть) | Tower middleware для валидации JWT по JWKS, проверки iss/exp/aud/nbf/scopes на protected routes. Альтернатива: кастомный middleware в extractors (Этап 3) |
| Session management | **Существующий механизм** (axum sessions) | Уже работает; `tower-sessions` — альтернатива если понадобится миграция |
| RSA key generation | **`rsa` crate** | Генерация RSA 2048-bit keypair при первом запуске |

**Почему кастомный AS, а не библиотека:**
1. Бо́льшая часть логики (installations, multi-tenant, Principal, audit) — кастомная в любом случае
2. OAuth2 endpoints (authorize, token, revoke) — это конечный, хорошо специфицированный набор (~500-700 строк)
3. Полный контроль над интеграцией с SeaORM, RBAC, tenant isolation
4. Нет риска зависимости от unmaintained upstream

### Терминология: "App" vs "Module"

В RusTok эти понятия **не пересекаются**:

| Термин | Что это | Пример | Где живёт в UI |
|--------|---------|--------|----------------|
| **Application** (App) | Внешний клиент, подключённый к API через OAuth2 | ERP-система, мобильное приложение, сторонний SPA, CLI | `/dashboard/apps` — "Приложения" |
| **Module** | Серверный плагин (Rust crate), компилируемый в бинарник | Blog, Commerce, Forum, SEO | `/dashboard/modules` — "Модули" |

- **Application** = _кто_ обращается к API (клиент с client_id + scopes)
- **Module** = _что_ есть на сервере (функциональность платформы)

Приложение может использовать API, которое предоставляется модулями. Модуль может быть включён/отключен независимо от подключённых приложений. OAuth-подсистема сама является модулем (`rustok-oauth`, slug: "oauth").

---

## Этап 0: Новый крейт `rustok-oauth` + инфраструктура ключей RS256

**Цель**: Создать отдельный крейт для OAuth, реализовать асимметричные ключи (RS256), JWKS endpoint.

### Новые файлы

**`crates/rustok-oauth/Cargo.toml`** — зависимости: rustok-core, jsonwebtoken (уже 10.3.0 в workspace — поддерживает RS256), rsa (генерация RSA keypair), sha2, hex, argon2, rand, chrono, serde, serde_json, uuid, async-trait, tracing, base64, thiserror.

**`crates/rustok-oauth/src/lib.rs`** — экспорт публичного API крейта

**`crates/rustok-oauth/src/keys.rs`** — управление ключами:
- `KeyPair` struct: RSA private key + kid (key ID)
- `KeyStore`: загрузка PEM из файла (`data/keys/`), генерация при отсутствии
- Поддержка нескольких ключей для ротации (kid в JWKS)
- При первом запуске: сгенерировать RSA 2048-bit пару

**`crates/rustok-oauth/src/jwks.rs`** — формирование JWKS JSON:
- Конвертация публичной части ключа в JWK (RFC 7517)
- `keys[]` с kid, kty="RSA", alg="RS256", n, e
- Кэширование JSON с TTL 5 минут

**`crates/rustok-oauth/src/token.rs`** — encode/decode JWT:
- `OAuthClaims` struct (RFC 9068 — JWT Profile for OAuth2):
  ```rust
  pub struct OAuthClaims {
      pub iss: String,           // issuer URL
      pub sub: String,           // user_id или installation_id
      pub aud: Vec<String>,      // ["graphql", "rest"]
      pub exp: usize,
      pub iat: usize,
      pub jti: String,           // unique token ID
      pub client_id: String,     // OAuth client_id
      pub tenant_id: Uuid,
      pub scope: String,         // "products:read orders:read"
      pub subject_type: String,  // "user" | "app"
  }
  ```
- `encode_access_token(key: &KeyPair, claims: &OAuthClaims) -> String`
- `decode_access_token(jwks: &[JwkPublic], token: &str) -> OAuthClaims`
- Header: `typ: "at+jwt"`, `kid: "..."`, `alg: "RS256"`

**`crates/rustok-oauth/src/config.rs`** — типы конфигурации:
- `OAuthConfig { algorithm, access_ttl_secs, refresh_ttl_secs, code_ttl_secs, issuer, key_path }`

**`crates/rustok-oauth/src/pkce.rs`** — PKCE (RFC 7636):
- `verify_pkce_s256(code_verifier: &str, code_challenge: &str) -> bool`
- `BASE64URL(SHA256(code_verifier)) == code_challenge`

### Файлы для изменения

- **`Cargo.toml`** (workspace) — добавить `rustok-oauth` в workspace.members и workspace.dependencies
- **`apps/server/Cargo.toml`** — добавить зависимость rustok-oauth
- **`apps/server/src/common/settings.rs`** — добавить `OAuthSettings` struct:
  ```yaml
  # в development.yaml:
  settings.rustok.oauth:
    algorithm: RS256
    access_ttl_secs: 900        # 15 мин
    refresh_ttl_secs: 2592000   # 30 дней
    code_ttl_secs: 300          # 5 мин
    key_path: data/keys
  ```
- **`apps/server/config/development.yaml`** — добавить секцию oauth

---

## Этап 1: Миграции БД — 7 новых таблиц

**Цель**: Создать все таблицы для OAuth подсистемы.

Все файлы в `apps/server/migration/src/`:

### `m20260303_000001_create_applications.rs`
```
applications: id uuid PK, name text, slug text UNIQUE, description text NULL,
  publisher_tenant_id uuid NULL FK→tenants, created_at, updated_at
```

### `m20260303_000002_create_oauth_clients.rs`
```
oauth_clients: id uuid PK, application_id uuid FK→applications CASCADE,
  client_id text UNIQUE (публичный идентификатор),
  client_type text (public|confidential),
  client_secret_hash text NULL (argon2, только для confidential),
  redirect_uris jsonb, allowed_grants jsonb, allowed_scopes jsonb,
  pkce_required bool DEFAULT true, created_at, rotated_at NULL
Индексы: idx_oauth_clients_client_id UNIQUE, idx_oauth_clients_app
```

### `m20260303_000003_create_installations.rs`
```
installations: id uuid PK, tenant_id uuid FK→tenants, application_id uuid FK→applications,
  status text DEFAULT 'active' (active|suspended|revoked),
  granted_scopes jsonb, installed_by_user_id uuid FK→users NULL,
  installed_at, revoked_at NULL
UNIQUE(tenant_id, application_id)
```

### `m20260303_000004_create_authorization_codes.rs`
```
authorization_codes: id uuid PK, code_hash text UNIQUE,
  client_id uuid FK→oauth_clients, installation_id uuid FK→installations,
  user_id uuid FK→users, tenant_id uuid FK→tenants,
  redirect_uri text, scope text,
  code_challenge text, code_challenge_method text DEFAULT 'S256',
  state text NULL, expires_at timestamptz, consumed_at NULL, created_at
```

### `m20260303_000005_create_oauth_refresh_tokens.rs`
```
oauth_refresh_tokens: id uuid PK, token_hash text UNIQUE,
  tenant_id uuid FK→tenants, client_id uuid FK→oauth_clients,
  subject_type text (user|installation), subject_id uuid,
  scope text, expires_at, revoked_at NULL,
  rotated_from_id uuid FK→self NULL, created_at
Индексы: idx_oauth_rt_hash UNIQUE, idx_oauth_rt_subject, idx_oauth_rt_client
```

### `m20260303_000006_create_access_token_jti.rs`
```
access_token_jti: jti uuid PK, tenant_id uuid, client_id uuid FK→oauth_clients,
  subject_type text, subject_id uuid, revoked_at NULL, expires_at
Индексы: idx_jti_expires (для cleanup)
```

### `m20260303_000007_create_platform_audit_log.rs`

**Единый audit log** для всей платформы — OAuth, модули, и другие домены. Поле `category` разделяет домены.

```
platform_audit_log: id uuid PK, tenant_id uuid,
  category text NOT NULL (auth|modules|system|commerce|...),
  actor_type text (user|client|system), actor_id uuid NULL,
  event_type text, target_type text NULL, target_id uuid NULL,
  client_id uuid FK→oauth_clients NULL,
  ip_address text NULL, user_agent text NULL, details jsonb, created_at
Индексы: idx_audit_tenant_time, idx_audit_category, idx_audit_event_type
```

Модульный план (`docs/modules/module-rebuild-plan.md`) переиспользует ту же таблицу:
category="modules", event_type: `module.installed`, `module.enabled`, `build.started`, etc.

### Файлы для изменения
- **`apps/server/migration/src/lib.rs`** — 7 новых `mod` + регистрация в `Migrator::migrations()`

### SeaORM entities — новые файлы
В `apps/server/src/models/_entities/`:
- `applications.rs`, `oauth_clients.rs`, `installations.rs`, `authorization_codes.rs`, `oauth_refresh_tokens.rs`, `access_token_jti.rs`, `platform_audit_log.rs`

В `apps/server/src/models/`:
- `applications.rs` — бизнес-методы, валидация slug
- `oauth_clients.rs` — `find_by_client_id()`, генерация client_id
- `installations.rs` — `find_active_by_tenant_app()`, scope-проверки

Обновить `models/_entities/mod.rs` и `models/mod.rs` — экспорт.

---

## Этап 2: Сервисный слой OAuth2

**Цель**: Вся бизнес-логика OAuth2 в сервисных модулях.

### Новые файлы

**`apps/server/src/services/oauth/mod.rs`** — подмодули:

**`services/oauth/application_service.rs`**:
- `create_application(db, name, slug, publisher_tenant_id)`, `list_applications()`, `get_application()`

**`services/oauth/client_service.rs`**:
- `create_client(db, app_id, client_type, redirect_uris, grants, scopes) -> (OAuthClient, Option<secret_plaintext>)`
  - confidential: генерировать secret, хранить argon2 hash, вернуть plaintext один раз
  - public: secret = NULL, pkce_required = true
- `rotate_client_secret(db, client_id) -> secret_plaintext`
- `validate_client_credentials(db, client_id, secret) -> Result<OAuthClient>`
- Переиспользовать: `auth::hash_password` для хеширования, `auth::verify_password` для проверки

**`services/oauth/installation_service.rs`**:
- `install_app(db, tenant_id, app_id, scopes, user_id) -> Installation`
- `revoke_installation(db, installation_id)` — каскадный revoke всех токенов
- `list_installations(db, tenant_id)`

**`services/oauth/token_service.rs`** — ядро OAuth2 AS:

*Authorization Code + PKCE*:
- `create_authorization_code(db, client, user_id, tenant_id, redirect_uri, scope, code_challenge) -> code_plaintext`
  - Переиспользовать: `auth::generate_refresh_token()` для генерации random bytes
  - Переиспользовать: `auth::hash_refresh_token()` для SHA-256 хеширования
  - TTL: 5 минут (из config)
- `exchange_authorization_code(db, code, client_id, redirect_uri, code_verifier) -> OAuthTokens`
  - Проверки: code не expired/consumed, redirect_uri exact match, PKCE S256 verify
  - Пометить consumed, выпустить access + refresh

*Client Credentials*:
- `issue_client_credentials_token(db, client, scope, tenant_id) -> OAuthTokens`
  - Только для confidential clients
  - Без refresh token (client переаутентифицируется)

*Refresh*:
- `refresh_oauth_token(db, refresh_token, client_id) -> OAuthTokens`
  - Rotation: revoke старый → create новый
  - Replay detection: если старый уже revoked → revoke всю цепочку (rotated_from_id)

*Revocation (RFC 7009)*:
- `revoke_token(db, token, token_type_hint)`

**`services/oauth/audit_service.rs`** — запись в `platform_audit_log`:
- `log_event(db, tenant_id, actor_type, actor_id, event_type, ip, user_agent, details)`
- Использовать event bus для асинхронной записи

### Файлы для изменения
- **`apps/server/src/services/mod.rs`** — `pub mod oauth;`

---

## Этап 3: Unified Principal Model

**Цель**: Заменить user-only `AuthContext` на `Principal` enum, поддерживающий и пользователей, и приложения.

### Новые файлы

**`crates/rustok-oauth/src/principal.rs`**:
```rust
pub enum Principal {
    User { user_id: Uuid, session_id: Uuid, tenant_id: Uuid, permissions: Vec<Permission> },
    App { installation_id: Uuid, client_id: String, tenant_id: Uuid, permissions: Vec<Permission> },
}

impl Principal {
    pub fn tenant_id(&self) -> Uuid { ... }
    pub fn permissions(&self) -> &[Permission] { ... }
    pub fn has_permission(&self, perm: &Permission) -> bool { ... }
    pub fn is_user(&self) -> bool { ... }
    pub fn user_id(&self) -> Option<Uuid> { ... }
}
```

**`crates/rustok-oauth/src/scope.rs`** — маппинг scopes ↔ permissions:
- `scopes_to_permissions(scope_string: &str) -> Vec<Permission>`
  - Парсит "products:read orders:list" → Vec<Permission> через `Permission::from_str()`
  - Переиспользовать: существующий `Permission::from_str()` из `crates/rustok-core/src/permissions.rs:171`

### Файлы для изменения

**`apps/server/src/context/auth.rs`** — расширить `AuthContext`:
- Добавить `principal: Principal` (или заменить отдельные поля на enum)
- Helper-методы: `tenant_id()`, `permissions()`, `user_id()`, `is_app()`
- Обеспечить обратную совместимость: старый код продолжает работать через delegation

**`apps/server/src/extractors/auth.rs`** — dual JWT decode в `resolve_current_user`:
1. Извлечь Bearer token
2. Проверить наличие `kid` в JWT header:
   - Есть `kid` → decode как RS256 через JWKS → OAuthClaims → Principal
   - Нет `kid` → decode как HS256 (legacy) → Claims → Principal::User
3. `CurrentUser` и `OptionalCurrentUser` остаются, но внутри работают через Principal
4. Добавить `CurrentPrincipal` / `OptionalCurrentPrincipal` extractors

**Альтернатива**: рассмотреть `tower-oauth2-resource-server` — Tower middleware для валидации JWT по JWKS endpoint с проверкой iss, exp, aud, nbf, scopes. Может упростить реализацию resource server стороны (валидацию OAuth токенов на protected routes), заменив часть кастомного кода в extractors. Решение принять при реализации — если объём кастомного кода в extractors окажется тривиальным (~50-100 строк), отдельный крейт может быть излишним.

**`apps/server/src/controllers/graphql.rs`** — адаптация:
- `AuthContext` передаётся как обычно, но теперь содержит Principal
- GraphQL resolvers используют `ctx.data::<AuthContext>()?.permissions()` — единый интерфейс

---

## Этап 4: OAuth2 REST Endpoints

**Цель**: Authorization Server endpoints.

### Новые файлы

**`apps/server/src/controllers/oauth/mod.rs`** — роутер + подмодули

**`controllers/oauth/authorize.rs`** — `GET /oauth/authorize`:
- Параметры: response_type=code, client_id, redirect_uri, scope, state, code_challenge, code_challenge_method
- Валидация: client_id exists, redirect_uri exact match (RFC 9700), PKCE обязателен для public clients
- Если user авторизован → check consent → generate auth code → redirect
- Если нет → redirect на login с return URL

**`controllers/oauth/consent.rs`** — consent page:
- **First-party apps** (`publisher_tenant_id` совпадает): auto-approve, без показа consent
- **Third-party apps**: server-rendered HTML страница со списком запрошенных scopes + кнопки "Разрешить"/"Отказать"
- После approve → redirect к authorize для генерации code

**`controllers/oauth/token.rs`** — `POST /oauth/token`:
- Content-Type: application/x-www-form-urlencoded (по стандарту)
- `grant_type=authorization_code` → exchange code + PKCE verify → tokens
- `grant_type=refresh_token` → rotation → new tokens
- `grant_type=client_credentials` → client auth + scope → access_token (без refresh)
- Response: `{ access_token, token_type: "Bearer", expires_in, refresh_token?, scope }`
- Headers: `Cache-Control: no-store`, `Pragma: no-cache`

**`controllers/oauth/revoke.rs`** — `POST /oauth/revoke` (RFC 7009):
- Параметры: token, token_type_hint
- Всегда 200 (по стандарту, даже если токен не найден)

**`controllers/oauth/jwks.rs`** — `GET /.well-known/jwks.json`

**`controllers/oauth/discovery.rs`** — `GET /.well-known/openid-configuration`:
- Возвращает: issuer, authorization_endpoint, token_endpoint, revocation_endpoint, jwks_uri, response_types_supported, grant_types_supported, scopes_supported

### Файлы для изменения

- **`apps/server/src/controllers/mod.rs`** — `pub mod oauth;`
- **`apps/server/src/app.rs`** — условная регистрация роутов при `oauth_enabled: true`:
  ```
  /oauth/authorize, /oauth/token, /oauth/revoke
  /.well-known/jwks.json, /.well-known/openid-configuration
  ```
- **Rate limiter**: `/oauth/token` → 20 req/min per client_id (как auth endpoints)

---

## Этап 5: GraphQL Admin API

**Цель**: Управление приложениями, клиентами, установками через GraphQL.

### Новые файлы

**`apps/server/src/graphql/oauth/mod.rs`** — `OAuthQuery` + `OAuthMutation`

**`graphql/oauth/types.rs`** — GraphQL types:
- `Application`, `OAuthClient` (без client_secret_hash!), `Installation`
- `CreateApplicationInput`, `CreateOAuthClientInput`, `InstallApplicationInput`
- `CreateOAuthClientPayload { client: OAuthClient, clientSecretOnce: Option<String> }`
- `RotateSecretPayload { clientId: String, newClientSecret: String }`

**`graphql/oauth/mutations.rs`**:
- `createApplication(input)` — SuperAdmin only
- `createOAuthClient(applicationId, input)` — вернуть secret один раз
- `installApplication(input)` — Admin tenant-а
- `rotateClientSecret(clientId)` — вернуть новый secret один раз
- `revokeInstallation(installationId)` — revoke + cascade tokens

**`graphql/oauth/queries.rs`**:
- `applications`, `application(id)`, `installations(tenantId)`, `oauthClients(applicationId)`, `authAuditLog(filter, pagination)`

### Файлы для изменения

- **`crates/rustok-core/src/permissions.rs`** — добавить `Resource::Applications`, `Resource::OAuthClients`, `Resource::Installations`, `Resource::AuditLog` (общий для платформы, не только OAuth) + Permission constants
- **`apps/server/src/graphql/schema.rs`** — добавить `OAuthQuery` и `OAuthMutation` в `MergedObject`
- **`apps/server/src/graphql/mod.rs`** — `pub mod oauth;`

---

## Этап 6: Seed Data — first-party apps

**Цель**: При dev-старте создавать Application + клиентов для UI и CLI.

### Seed data

```
Application: "RusTok Platform", slug: "rustok-platform", publisher: system tenant

OAuth Clients — UI (все public, pkce_required: true, grant: authorization_code):
  - client_id: "rustok-admin-leptos",      redirect: ["http://localhost:3001/auth/callback"]
  - client_id: "rustok-storefront-leptos", redirect: ["http://localhost:3101/auth/callback"]
  # Next.js клиенты не включены в seed — подключаются вручную через
  # GraphQL createOAuthClient при выборе Next.js в качестве фронтенда.
  # Пример redirect URIs: http://localhost:3000/auth/callback (admin),
  #                        http://localhost:3100/auth/callback (storefront)

OAuth Clients — CLI (public, pkce_required: true, grant: authorization_code):
  - client_id: "rustok-cli",               redirect: ["http://127.0.0.1:*/callback"]
    ← используется `rustok auth login` из CLI для авторов модулей
      (см. docs/modules/marketplace-plan.md, секция 7.2)
    ← redirect на localhost с динамическим портом (стандартный паттерн для CLI OAuth)

OAuth Clients — server-to-server (confidential, grant: client_credentials):
  - client_id: "rustok-internal-worker"
    ← для фоновых задач (build pipeline, cleanup, etc.)
    ← secret генерируется при первом запуске, выводится в логи

Installation: все клиенты в default tenant, full scopes, auto-approved (first-party)
```

### Связь с маркетплейсом модулей

`rustok-cli` — это тот же клиент, через который авторы модулей аутентифицируются
для `rustok module publish` (см. `docs/modules/marketplace-plan.md`, секция 7).
При запуске `rustok auth login` CLI:
1. Стартует локальный HTTP-сервер на random порту
2. Открывает браузер → `/oauth/authorize?client_id=rustok-cli&redirect_uri=http://127.0.0.1:{port}/callback&...`
3. Пользователь авторизуется → callback → CLI получает tokens
4. Токены сохраняются в `~/.config/rustok/credentials.json`

### Файлы для изменения
- **`apps/server/src/initializers/`** или **seed** — создание OAuth seed при первом запуске (если таблица applications пуста)

---

## Этап 7: Audit, Events, Cleanup

**Цель**: Полный audit trail, domain events, фоновая очистка.

### Новые файлы

**`apps/server/src/tasks/oauth_cleanup.rs`** — background task:
- Удалить expired authorization_codes (старше code_ttl + буфер)
- Удалить expired access_token_jti записи
- Удалить expired+revoked oauth_refresh_tokens (retain 30 дней для аудита)

**`apps/server/src/tasks/key_rotation.rs`** — ротация ключей:
- Проверить возраст active key → если старше порога → сгенерировать новый, старый в JWKS ещё 2× max token TTL

**`crates/rustok-oauth/src/module.rs`** — `RusToKModule` для OAuth:
```rust
impl RusToKModule for OAuthModule {
    fn slug(&self) -> &'static str { "oauth" }
    fn name(&self) -> &'static str { "OAuth2 App Connections" }
    fn kind(&self) -> ModuleKind { ModuleKind::Optional }
    fn dependencies(&self) -> &[&'static str] { &["rbac"] }
    fn permissions(&self) -> Vec<Permission> { /* Applications CRUD, Installations, AuditLog */ }
}
```

**`crates/rustok-oauth/rustok-module.toml`** — манифест по стандарту маркетплейса (см. `docs/modules/marketplace-plan.md`):
```toml
[module]
slug = "oauth"
name = "OAuth2 App Connections"
version = "0.1.0"
description = "OAuth2 Authorization Server with PKCE, client credentials, and app installations"
authors = ["RusTok Team"]
license = "MIT"

[compatibility]
rustok_min = "0.5.0"
rust_edition = "2024"

[dependencies]
rbac = ">= 0.1.0"

[crate]
name = "rustok-oauth"
entry_type = "OAuthModule"

[provides]
migrations = true
permissions = [
    "applications:create",
    "applications:read",
    "applications:manage",
    "oauth_clients:create",
    "oauth_clients:read",
    "oauth_clients:manage",
    "installations:create",
    "installations:read",
    "installations:manage",
    "audit_log:read",
]
events_emitted = [
    "oauth.token.issued",
    "oauth.token.revoked",
    "oauth.code.exchanged",
    "oauth.app.installed",
    "oauth.app.revoked",
    "oauth.secret.rotated",
]
graphql_types = ["Application", "OAuthClient", "Installation", "AuthAuditLogEntry"]
graphql_queries = ["applications", "application", "installations", "oauthClients", "authAuditLog"]
graphql_mutations = ["createApplication", "createOAuthClient", "installApplication", "rotateClientSecret", "revokeInstallation"]

[[provides.admin_nav]]
label_key = "app.nav.apps"
href = "/apps"
icon = "grid-3x3"
section = "platform"

[locales]
supported = ["en", "ru"]
default = "en"
```

Это обеспечивает совместимость `rustok-oauth` с модульным стандартом из `docs/modules/marketplace-plan.md`. Файлы `locales/en.json` и `locales/ru.json` создать в `crates/rustok-oauth/locales/`.

### Файлы для изменения
- **`crates/rustok-core/src/events/types.rs`** — новые DomainEvent варианты: `OAuthTokenIssued`, `OAuthTokenRevoked`, `OAuthCodeExchanged`, `AppInstalled`, `AppRevoked`, `ClientSecretRotated`. Naming convention: `{Domain}{Entity}{Action}` — согласовано с `events_emitted` в `rustok-module.toml` (dot-notation: `oauth.token.issued`). DomainEvent enum использует PascalCase, audit log event_type — dot-notation
- **`apps/server/src/app.rs`** — регистрация cleanup tasks
- **`apps/server/src/modules/`** (или где регистрируется registry) — добавить `OAuthModule`

---

## Этап 8: Feature Flag и финальная интеграция

### Feature flag стратегия

Используем существующий `settings.rustok.features.oauth_enabled` из `apps/server/src/common/settings.rs`:

**`oauth_enabled: false`** (default):
- OAuth роуты не регистрируются (404)
- `CurrentUser` работает только с HS256 (текущее поведение)
- GraphQL OAuth queries/mutations возвращают "Feature not enabled"
- Миграции запускаются (таблицы пусты)
- KeyStore не инициализируется

**`oauth_enabled: true`**:
- OAuth роуты зарегистрированы
- `CurrentPrincipal` поддерживает HS256 и RS256
- KeyStore инициализирован, JWKS endpoint активен
- Все OAuth GraphQL операции доступны

**Переходный период** (оба механизма активны):
- `/api/auth/login` продолжает выдавать HS256 токены
- `/oauth/token` выдаёт RS256 токены
- Оба типа токенов принимаются везде через dual-decode в extractors

---

## Этап 9: Admin UI — Страницы регистрации и управления приложениями

**Цель**: Страницы "Apps" (как у Shopify) в Leptos-админке. Полный CRUD: список приложений, установка в tenant, создание OAuth-клиентов, просмотр установок. Next.js-версия откладывается — реализуется при необходимости по аналогии.

### Разделение с Modules UI

"Apps" и "Modules" — **отдельные секции** sidebar, разные доменные сущности:

```
Sidebar:
├── Overview
│   └── Dashboard
├── Management
│   └── Users
├── Platform                    ← новая секция
│   ├── Modules  (/modules)    ← серверные плагины (toggle, install, marketplace)
│   └── Apps     (/apps)       ← внешние OAuth-клиенты (API consumers)
├── Account
│   ├── Profile
│   └── Security
```

**Modules** (`/modules`) — уже реализовано, расширяется вкладками Installed/Catalog/Updates/Build History (см. `docs/modules/module-rebuild-plan.md`).

**Apps** (`/apps`) — новое, управление OAuth Applications и их подключениями к API.

Обе секции сгруппированы в "Platform", но это разные страницы с разными data sources и GraphQL queries. Пользователь видит их как два инструмента: "какие модули включены" vs "какие приложения подключены".

### 9.1 Leptos Admin (`apps/admin/`)

**Паттерн для следования**: `apps/admin/src/pages/modules.rs` + `apps/admin/src/features/modules/`

**Новые файлы — страницы**:
- `apps/admin/src/pages/apps.rs` — список установленных приложений
- `apps/admin/src/pages/app_details.rs` — детали приложения + OAuth-клиенты + установки
- `apps/admin/src/pages/app_create.rs` — регистрация нового приложения

**Новые файлы — feature**:
- `apps/admin/src/features/apps/mod.rs`
- `apps/admin/src/features/apps/api.rs` — GraphQL вызовы через gql_client (fetch_applications, install_app, create_app, create_client, rotate_secret, revoke_installation)
- `apps/admin/src/features/apps/components/mod.rs`
- `apps/admin/src/features/apps/components/apps_list.rs` — таблица/карточки (Leptos component, `#[component] pub fn AppsList`)
- `apps/admin/src/features/apps/components/app_card.rs` — карточка приложения со статусом и scopes
- `apps/admin/src/features/apps/components/create_app_form.rs` — форма регистрации (с Leptos реактивными сигналами)
- `apps/admin/src/features/apps/components/client_secret_dialog.rs` — диалог одноразового показа секрета
- `apps/admin/src/features/apps/components/scope_selector.rs` — мультиселект scopes

**Файлы для изменения**:
- `apps/admin/src/shared/config/nav.rs` — добавить секцию "Apps" в `NAV_SECTIONS`:
  ```rust
  NavSection {
      label: "Platform",
      items: &[
          NavItem { label_key: "app.nav.modules", href: "/modules", icon: "package" },
          NavItem { label_key: "app.nav.apps", href: "/apps", icon: "grid-3x3" },
      ],
  },
  ```
- `apps/admin/src/app/router.rs` — добавить роуты:
  ```rust
  <Route path=path!("/apps") view=Apps />
  <Route path=path!("/apps/create") view=AppCreate />
  <Route path=path!("/apps/:id") view=AppDetails />
  ```
- `apps/admin/src/pages/mod.rs` — экспорт новых страниц
- `apps/admin/src/features/mod.rs` — `pub mod apps;`

### 9.2 UI/UX — как у Shopify

**Страница "Apps" (список)** — карточки установленных приложений:
- Название + иконка + описание
- Статус: Active / Suspended / Revoked (цветной badge)
- Granted scopes (свёрнутый список)
- Кнопки: "Manage", "Uninstall"
- Кнопка "Install app" → переход в каталог

**Страница "App Detail"** — после клика на приложение:
- Информация: publisher, дата установки, установивший пользователь
- Granted scopes (полный список, можно изменить)
- OAuth Clients: список клиентов (client_id, type, redirect URIs)
  - Кнопка "Rotate Secret" → диалог с новым секретом
  - Кнопка "Create Client" → форма
- Audit Log: последние события по этому приложению
- Кнопка "Revoke/Uninstall" (с подтверждением)

**Страница "Register App"** (для разработчиков):
- Form: name, slug (auto-generated), description
- OAuth Client setup: client type (public/confidential), redirect URIs, allowed grants, allowed scopes (checkbox list)
- Submit → показать client_id + client_secret (один раз, dialog)

### 9.3 i18n ключи

Добавить ключи перевода:
- `app.nav.apps` = "Apps" / "Приложения"
- `apps.title` = "Applications" / "Приложения"
- `apps.subtitle` = "Manage connected applications and integrations"
- `apps.install` = "Install App"
- `apps.create` = "Register App"
- `apps.revoke.confirm` = "Are you sure? This will revoke all access tokens."
- `apps.secret.warning` = "Copy this secret now. It won't be shown again."

### 9.4 Next.js Admin — отложено

Next.js админка (`apps/next-admin/`) реализуется позже при необходимости. Паттерн аналогичен Leptos: роуты в `app/dashboard/apps/`, feature в `features/apps/`, навигация в `config/nav-config.ts`. GraphQL API (Этап 5) одинаков для обоих фронтендов, поэтому Next.js-версия — чисто UI-задача без серверных изменений.

---

## Граф зависимостей

```
Этап 0 (Крейт + RS256 ключи)  ─┐
Этап 1 (Миграции БД)  ─────────┤ ← параллельно
                                │
Этап 2 (Сервисный слой)  ──────┤ ← зависит от 1
Этап 3 (Principal Model)  ─────┤ ← зависит от 0, 1
                                │
Этап 4 (OAuth REST)  ──────────┤ ← зависит от 0, 2, 3
Этап 5 (GraphQL Admin)  ───────┤ ← зависит от 2 (параллельно с 4)
Этап 6 (Seed Data)  ───────────┤ ← зависит от 1, 2
                                │
Этап 7 (Audit + Cleanup)  ─────┤ ← зависит от 1, 2
Этап 8 (Feature Flag)  ────────┤ ← можно на любом этапе
Этап 9 (Admin UI: Apps)  ──────┘ ← зависит от 5 (GraphQL API)
```

**Параллелизм**: Этапы 0+1 параллельно. Этапы 4+5 параллельно. Этапы 6+7+8 параллельно. Этап 9 после 5 (нужен GraphQL API).

---

## Критические существующие файлы

| Файл | Роль |
|------|------|
| `apps/server/src/auth.rs` | JWT HS256, Claims — расширяем dual-mode |
| `apps/server/src/extractors/auth.rs` | CurrentUser → CurrentPrincipal |
| `apps/server/src/context/auth.rs` | AuthContext — добавляем Principal |
| `apps/server/src/controllers/graphql.rs` | Injection auth context → адаптируем |
| `apps/server/src/services/auth_lifecycle.rs` | Паттерн create_session + tokens |
| `apps/server/migration/src/lib.rs` | Регистрация миграций |
| `apps/server/config/development.yaml` | oauth_enabled flag + новые настройки |
| `apps/server/src/common/settings.rs` | FeatureSettings + новый OAuthSettings |
| `crates/rustok-core/src/permissions.rs` | Resource/Action/Permission — расширяем |
| `crates/rustok-core/src/module.rs` | RusToKModule trait |
| `crates/rustok-core/src/events/types.rs` | DomainEvent — новые варианты |
| `apps/admin/src/shared/config/nav.rs` | Sidebar navigation — добавляем "Apps" |
| `apps/admin/src/app/router.rs` | Leptos роутер — добавляем /apps роуты |
| `apps/admin/src/pages/modules.rs` | Паттерн для pages/apps.rs (Leptos) |
| `apps/admin/src/features/modules/` | Паттерн для features/apps/ (Leptos) |

---

## Верификация

### Тесты (на каждом этапе)
- **Миграции**: up/down на test DB (`rustok_test_utils::db::setup_test_db_with_migrations`)
- **Сервисы**: PKCE S256 verification, auth code exchange, token rotation, scope validation, replay detection
- **JWT**: RS256 encode/decode round-trip, dual-mode (HS256 + RS256)
- **Principal**: permission resolution для User и App
- **Integration**: полный Authorization Code + PKCE flow end-to-end
- **Client Credentials**: token → API call с scopes
- **Feature flag**: oauth_enabled=false → 404

### Команды
```bash
cargo test                    # все тесты
cargo clippy                  # без warnings
cargo run                     # dev server → OAuth endpoints
```

### Ручная верификация (curl)
1. GET /oauth/authorize?... → redirect с code
2. POST /oauth/token (authorization_code) → access + refresh
3. Authorization: Bearer <token> → GraphQL /api/graphql
4. POST /oauth/token (refresh_token) → new tokens
5. POST /oauth/revoke → 200
6. GET /.well-known/jwks.json → JWKS JSON

---

## Приложение A: Взаимодействие с маркетплейсом модулей

> Связанный документ: `docs/modules/marketplace-plan.md`

Маркетплейс модулей (`modules.rustok.dev`) — **отдельный сервис** со своей БД и API.
OAuth-подсистема основного RusTok-сервера обеспечивает аутентификацию для трёх сценариев
взаимодействия с маркетплейсом.

### A.1 Архитектура auth для маркетплейса

```
┌─────────────────────────────────────────────────────────────────────┐
│  modules.rustok.dev (отдельный сервис)                               │
│                                                                      │
│  ┌──────────────────────────────┐                                    │
│  │  GraphQL API                 │                                    │
│  │  marketplace(search) →       │◄──── (2) Server proxy              │
│  │  publishModule(crate) →      │◄──── (3) CLI direct                │
│  │  downloadCrate(slug,ver) →   │◄──── (4) Build pipeline            │
│  └──────────────────────────────┘                                    │
│                                                                      │
│  Auth: API key per platform instance + author tokens                 │
└──────────────────────────────────────────────────────────────────────┘
         ▲              ▲              ▲
         │(2)           │(3)           │(4)
         │              │              │
┌────────┴──────┐ ┌────┴──────┐ ┌────┴───────────────┐
│ RusTok Server │ │ CLI       │ │ Build Worker       │
│ (proxy)       │ │ (author)  │ │ (cargo build)      │
│               │ │           │ │                     │
│ OAuth AS ◄────│ │ OAuth ◄───│ │ client_credentials │
│ for admin UI  │ │ PKCE flow │ │ + marketplace key   │
└───────────────┘ └───────────┘ └─────────────────────┘
         ▲
         │(1)
┌────────┴──────┐
│ Admin UI      │
│ (Leptos/Next) │
│ OAuth PKCE    │
└───────────────┘
```

### A.2 Четыре auth-потока с маркетплейсом

#### (1) Admin UI → RusTok Server

Уже описан в этом плане (OAuth PKCE, `rustok-admin-*` clients).
Admin UI **не обращается к маркетплейсу напрямую**.

#### (2) RusTok Server → Marketplace API (прокси)

Admin UI запрашивает каталог через GraphQL основного сервера, который **проксирует** запросы к маркетплейсу:

```graphql
# На основном сервере (проксирует к modules.rustok.dev):
query { marketplace(search: "seo") { slug, name, ... } }
mutation { installModule(slug: "seo", version: "1.0") { id, status } }
```

**Auth**: Основной сервер аутентифицируется в маркетплейсе через **Platform API Key** — уникальный ключ инстанса, выдаваемый при регистрации платформы на modules.rustok.dev.

```yaml
# development.yaml
settings.rustok.marketplace:
  enabled: false                           # на MVP отключён
  registry_url: https://modules.rustok.dev
  api_key: ""                              # Platform API Key
```

Причина прокси, а не прямого доступа: безопасность (API key не утекает в браузер), кэширование, единая точка контроля.

#### (3) CLI автора → Marketplace API

Автор модуля использует `rustok module publish`. CLI аутентифицируется **напрямую к маркетплейсу**:

```
rustok auth login --registry modules.rustok.dev
→ OAuth PKCE flow к modules.rustok.dev (у маркетплейса свой OAuth AS или он делегирует)
→ Получает author token
→ Сохраняет в ~/.config/rustok/credentials.json
```

**Решение**: маркетплейс — отдельный сервис, у него может быть **свой OAuth AS** или он принимает токены от **любого RusTok-инстанса** (federated auth). Это решение за рамками текущего плана, но `rustok-cli` OAuth-клиент (определён в Этапе 6) может быть переиспользован для обоих сценариев.

`marketplace_accounts` (из marketplace-plan.md, строка 450) — аккаунты авторов **на стороне маркетплейса**, не в основной БД RusTok.

#### (4) Build Pipeline → Marketplace API

Build Worker скачивает .crate архивы из маркетплейса:

```
GET modules.rustok.dev/api/v1/crates/rustok-seo/1.0.0.crate
Authorization: Bearer <platform-api-key>
```

**Auth**: тот же Platform API Key из конфига сервера. Build worker работает от имени платформы, не от имени пользователя.

### A.3 GraphQL naming: install conflicts

Чтобы избежать путаницы между `installModule` (модуль) и `installApplication` (OAuth app):

| Mutation | Домен | Что делает |
|----------|-------|-----------|
| `installModule(slug, version)` | Modules | Скачать crate, пересобрать бинарник, deploy |
| `installApplication(input)` | OAuth | Создать Installation запись, выдать scopes |
| `toggleModule(slug, enabled)` | Modules | Включить/отключить для тенанта (мгновенно) |
| `revokeInstallation(id)` | OAuth | Отозвать OAuth-доступ приложения |

Эти мутации находятся в разных GraphQL модулях:
- `installModule` → `ModulesMutation` (из build service)
- `installApplication` → `OAuthMutation` (из rustok-oauth)

### A.4 Permissions для маркетплейса

Добавить в `crates/rustok-core/src/permissions.rs`:

```
Resource::Marketplace
  marketplace:browse     — просмотр каталога (все admin роли)
  marketplace:install    — установка/удаление модулей (admin+)
  marketplace:manage     — build history, rollback (super_admin)

Resource::Builds
  builds:view            — просмотр истории сборок
  builds:manage          — rollback, cancel
```

### A.5 Audit log: что куда пишется

| Событие | Где пишется | Таблица | category |
|---------|-------------|---------|----------|
| OAuth token issued | Основной сервер | `platform_audit_log` | `auth` |
| App installed (OAuth) | Основной сервер | `platform_audit_log` | `auth` |
| Module toggle (tenant) | Основной сервер | `platform_audit_log` | `modules` |
| Module install (platform) | Основной сервер | `platform_audit_log` | `modules` |
| Build started/completed | Основной сервер | `platform_audit_log` | `builds` |
| Module published | **Маркетплейс** | Его БД (marketplace_versions) | — |
| Module yanked | **Маркетплейс** | Его БД | — |

События на маркетплейсе **не попадают** в `platform_audit_log` основного сервера. Это нормально — разные сервисы, разные БД.
