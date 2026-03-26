# AI Context для RusToK

Обязательный стартовый контекст для AI-сессий.

## Порядок чтения

1. `docs/index.md`
2. `docs/AI_CONTEXT.md`
3. `README.md` и `CRATE_API.md` целевого компонента
4. При event-изменениях: `crates/rustok-outbox/docs/README.md` и `docs/architecture/event-flow-contract.md`

## Терминология

### Platform modules

Для platform modules существует только два статуса:

- `Core`
- `Optional`

Источник истины по составу модулей — `modules.toml`.

### Crates

`crate` — техническая упаковка в Cargo. Не каждый crate в `crates/` автоматически является platform module.

В `crates/` лежат:

- module-crates
- shared libraries
- infrastructure/support crates

### Важное правило

Не смешивай:

- **статус модуля** (`Core` / `Optional`)
- **способ wiring** (`ModuleRegistry`, bootstrap, codegen, host wiring)
- **форму упаковки** (`crate`)

`rustok-outbox` — `Core` module. То, что event runtime использует его напрямую, не делает его отдельным типом компонента.

## Текущий platform baseline

### Core modules

- `auth`
- `cache`
- `email`
- `index`
- `outbox`
- `tenant`
- `rbac`

### Optional modules

- `content`
- `commerce`
- `blog`
- `forum`
- `pages`
- `media`
- `workflow`

## Общие инварианты

- Platform modules должны оставаться согласованными между `modules.toml`, `build_registry()` и manifest validation.
- Для write-flow с межмодульными событиями используется transactional outbox.
- Tenant isolation и RBAC обязательны в сервисном слое.
- События и обработчики должны оставаться совместимыми по `DomainEvent` / `EventEnvelope`.

## Замены loco-подсистем — обязательно к прочтению

Часть встроенных подсистем loco заменена собственными модулями. **Не дублируй их параллельными реализациями.**

| Loco-подсистема | Заменена на | Что делать | Что НЕ делать |
|---|---|---|---|
| `ctx.config.auth` / JWT middleware | `rustok-auth` (`crates/rustok-auth`) | Использовать `auth_config_from_ctx(ctx)` → `encode_access_token` / `decode_access_token` из `apps/server/src/auth.rs` | Не использовать `loco_rs::prelude::auth::JWT` напрямую; не реализовывать собственный JWT вне `rustok-auth` |
| `ctx.config.cache` / Loco cache config | `rustok-cache` (`crates/rustok-cache`) | Получать `CacheService` из `ctx.shared_store.get::<CacheService>()` — он инициализируется в `bootstrap_app_runtime` | Не читать `REDIS_URL` вручную в модулях; не создавать `redis::Client` напрямую; не игнорировать `ctx.config.cache` ради самостоятельного подключения |
| Loco Mailer (`ctx.mailer`) / SMTP | `rustok-email` + `apps/server/src/services/email.rs` | Использовать `email_service_from_ctx(ctx, locale)` — возвращает `Box<dyn PasswordResetEmailSender>`; провайдер выбирается через `settings.rustok.email.provider` | Не вызывать `ctx.mailer` напрямую в обработчиках; не создавать `AsyncSmtpTransport` вне email-сервиса; не выносить email в отдельный platform module |
| Loco Storage abstraction | `rustok-storage` (`crates/rustok-storage`) | Получать `StorageService` из `ctx.shared_store.get::<StorageService>()`; загружать файлы через него | Не создавать adhoc upload backends в контроллерах; не добавлять параллельные storage paths мимо `rustok-storage` |
| Loco Queue / Workers | `rustok-outbox` — не прямая замена, а самостоятельный слой для transactional event delivery. Loco Queue (Sidekiq) и Outbox решают разные задачи. | Для доменных событий с гарантией атомарности: `publish_in_tx` через `TransactionalEventBus`. Для фоновых/maintenance задач: loco Tasks. | Не дублировать event delivery-path через Loco Queue; не создавать `rustok-jobs` поверх outbox — они решают разные задачи. ADR: `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md` |
| Loco Channels (WebSocket) | Кастомный Axum WebSocket в `apps/server` | Использовать существующие WS-handlers | Не использовать `loco_rs::controller::channels` — несовместимо с кастомным auth-handshake |

**Что по-прежнему берётся из loco напрямую:**
- `Hooks` trait — lifecycle приложения (`app.rs`)
- `AppContext` — runtime context, передаётся повсюду
- `Config` + YAML конфиги (`development.yaml`, `test.yaml`)
- SeaORM stack — ORM, migrations, entities
- Tasks (`cargo loco task`) — фоновые/CLI задачи
- Initializers — startup hooks (telemetry)

Полная матрица: [`apps/server/docs/LOCO_FEATURE_SUPPORT.md`](../apps/server/docs/LOCO_FEATURE_SUPPORT.md)

---

## Важные crate'ы

### `crates/rustok-core`

Платформенные контракты: `RusToKModule`, `ModuleRegistry`, permissions, events, health, metrics.

### `crates/rustok-events`

Канонический слой event-контрактов поверх platform event model.

### `crates/rustok-auth`

`Core` module аутентификации: JWT (HS256), Argon2 хеширование паролей, refresh tokens, password reset, invite, email verification tokens. **Заменяет** `loco_rs::prelude::auth::JWT`. Подключается через `apps/server/src/auth.rs` (bridge: `AuthError → loco_rs::Error`).

### `crates/rustok-cache`

`Core` module управления кэшем: Redis-клиент (одна точка подключения), in-memory fallback (Moka), `CacheService::health()` с PING-проверкой. **Заменяет** `ctx.config.cache`. Инициализируется в `bootstrap_app_runtime`, доступен через `ctx.shared_store.get::<CacheService>()`.

### `crates/rustok-email`

`Core` module email-рассылок: SMTP через lettre, Tera-шаблоны, `PasswordResetEmailSender` trait. **Заменяет** Loco Mailer как primary transport. Фабрика `email_service_from_ctx(ctx, locale)` в `apps/server/src/services/email.rs` выбирает провайдер (`smtp | loco | none`). SMTP-транспорт кэшируется в `shared_store` через `SharedSmtpEmailService`.

### `crates/rustok-storage`

Infrastructure crate хранилищ: `StorageBackend` trait, `LocalStorage`, `StorageService`. **Заменяет** Loco Storage abstraction. Инициализируется в `bootstrap_app_runtime` (feature `mod-media`), доступен через `ctx.shared_store.get::<StorageService>()`. S3 backend задекларирован в Cargo.toml features, но не реализован.

### `crates/rustok-outbox`

`Core` module transactional outbox: `TransactionalEventBus`, `OutboxTransport`, `OutboxRelay`, `SysEventsMigration`. **Заменяет** Loco Queue / Workers. ADR: `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md`.

### `crates/rustok-tenant`

`Core` module multi-tenant lifecycle и module enablement.

### `crates/rustok-rbac`

`Core` module authorization, roles, policies и permission resolution.

### `crates/rustok-content` / `commerce` / `blog` / `forum` / `pages` / `media` / `workflow`

Optional domain modules и их transport/UI surfaces.

## Do / Don't

### Do

- Используй только реально существующие API из кода и docs.
- Для доменных write-flow с событиями применяй `publish_in_tx`, когда нужен атомарный publish.
- Проверяй, что docs отражают текущий код, а не старые архитектурные предположения.

### Don't

- Не придумывай третий тип модулей кроме `Core` и `Optional`.
- Не подменяй архитектурный статус модуля способом runtime wiring.
- Не обходи outbox в production event-flow.
