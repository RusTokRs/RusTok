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
- `channel`
- `email`
- `index`
- `search`
- `outbox`
- `tenant`
- `rbac`

### Optional modules

- `content`
- `commerce`
- `blog`
- `comments`
- `forum`
- `pages`
- `taxonomy`
- `media`
- `workflow`

## Общие инварианты

- Platform modules должны оставаться согласованными между `modules.toml`, `build_registry()` и manifest validation.
- Для write-flow с межмодульными событиями используется transactional outbox.
- Tenant isolation и RBAC обязательны в сервисном слое.
- События и обработчики должны оставаться совместимыми по `DomainEvent` / `EventEnvelope`.
- Для Leptos host-приложений и module-owned Leptos UI пакетов внутренний data-layer по умолчанию строится через `#[server]`-функции.
- GraphQL остаётся обязательным параллельным контрактом: для headless-клиентов, Next.js UI и fallback-веток в Leptos во время миграции/частичного покрытия.
- Новый Leptos UI код не должен рождаться как GraphQL-only путь, если для сценария возможен native `#[server]`-слой.
- Для core-wave source of truth по составу модулей и coverage считается `modules.toml`, а не устаревшие списки в локальных notes.
- В текущей core-wave `auth`, `search` и `channel` считаются active dual-path работой; `cache` и `email` уже покрыты host-level native-first surfaces; `index`, `outbox`, `tenant` и `rbac` теперь тоже имеют module-owned Leptos admin surfaces с native `#[server]` bootstrap.
- Для Leptos core surfaces допустимы только два legacy fallback path: GraphQL fallback для GraphQL-backed сценариев и REST fallback для `rustok-channel-admin`, пока его thin REST client ещё поддерживается параллельно.

## Замены loco-подсистем — обязательно к прочтению

Часть встроенных подсистем loco заменена собственными модулями. **Не дублируй их параллельными реализациями.**

| Loco-подсистема | Заменена на | Что делать | Что НЕ делать |
|---|---|---|---|
| `ctx.config.auth` / JWT middleware | `rustok-auth` (`crates/rustok-auth`) | Использовать `auth_config_from_ctx(ctx)` → `encode_access_token` / `decode_access_token` из `apps/server/src/auth.rs` | Не использовать `loco_rs::prelude::auth::JWT` напрямую; не реализовывать собственный JWT вне `rustok-auth` |
| `ctx.config.cache` / Loco cache config | `rustok-cache` (`crates/rustok-cache`) | Получать `CacheService` из `ctx.shared_store.get::<CacheService>()` — он инициализируется в `bootstrap_app_runtime` | Не читать `REDIS_URL` вручную в модулях; не создавать `redis::Client` напрямую; не игнорировать `ctx.config.cache` ради самостоятельного подключения |
| Loco Mailer (`ctx.mailer`) / SMTP | `rustok-email` + `apps/server/src/services/email.rs` | Использовать `email_service_from_ctx(ctx, locale)` — возвращает локализованный `BuiltInAuthEmailSender` для built-in auth mailers; провайдер выбирается через `settings.rustok.email.provider` | Не вызывать `ctx.mailer` напрямую в обработчиках; не создавать `AsyncSmtpTransport` вне email-сервиса; не выносить email в отдельный platform module |
| Loco Storage abstraction | `rustok-storage` (`crates/rustok-storage`) | Получать `StorageService` из `ctx.shared_store.get::<StorageService>()`; загружать файлы через него | Не создавать adhoc upload backends в контроллерах; не добавлять параллельные storage paths мимо `rustok-storage` |
| Loco Queue / Workers | `rustok-outbox` — не прямая замена, а самостоятельный слой для transactional event delivery. Loco Queue (Sidekiq) и Outbox решают разные задачи. | Для доменных событий с гарантией атомарности: `publish_in_tx` через `TransactionalEventBus`. Для фоновых/maintenance задач: loco Tasks. | Не дублировать event delivery-path через Loco Queue; не создавать `rustok-jobs` поверх outbox — они решают разные задачи. ADR: `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md` |
| Loco Channels (WebSocket) | Кастомный Axum WebSocket в `apps/server` | Использовать существующие WS-handlers | Не использовать `loco_rs::controller::channels` — несовместимо с кастомным auth-handshake |

**Что по-прежнему берётся из loco напрямую:**
- `Hooks` trait — lifecycle приложения (`app.rs`)
- `AppContext` — runtime context, передаётся повсюду
- `Config` + YAML конфиги (`development.yaml`, `test.yaml`)
- SeaORM stack — ORM, migrations, entities
- Tasks (`cargo loco task`) — CLI/maintenance задачи (`cleanup`, `rebuild`, `db_baseline`, `media_cleanup`, `create_oauth_app`)
- Initializers — startup hooks (telemetry)

**Loco Queue (Sidekiq/Redis) не подключён и не нужен.** Причины:
- Фоновые воркеры запускаются как tokio-таски напрямую: outbox relay (`OutboxRelayWorkerHandle`), build worker (`BuildWorkerHandle`), index/search dispatchers, workflow cron.
- Outbox паттерн архитектурно лучше Sidekiq для доменных событий — гарантирует атомарность.
- Loco Tasks покрывают maintenance/CLI нужды.
- Для отвязки медленных операций от HTTP-запросов используется `tokio::spawn` (например, отправка email в `forgot_password`).
- Если понадобится push-based очередь с retry — рассматривать расширение outbox relay, а не подключение Sidekiq.

Полная матрица: [`apps/server/docs/LOCO_FEATURE_SUPPORT.md`](../apps/server/docs/LOCO_FEATURE_SUPPORT.md)

---

## Важные crate'ы

### `crates/rustok-core`

Платформенные контракты: `RusToKModule`, `ModuleRegistry`, permissions, events, health, metrics.

### `crates/rustok-events`

Канонический слой event-контрактов поверх platform event model.

### `crates/rustok-auth`

`Core` module аутентификации: JWT (HS256 и RS256), Argon2 хеширование паролей, refresh tokens, password reset, invite, email verification tokens. **Заменяет** `loco_rs::prelude::auth::JWT`. Подключается через `apps/server/src/auth.rs` (bridge: `AuthError → loco_rs::Error`).

Алгоритм выбирается через `AuthConfig::algorithm: JwtAlgorithm`:
- `JwtAlgorithm::HS256` (default) — симметричный, `AuthConfig::secret`
- `JwtAlgorithm::RS256` — асимметричный, `AuthConfig::with_rs256(private_pem, public_pem)`

Server runtime reads auth overrides only through `settings.rustok.auth` in
`apps/server/src/auth.rs`: `algorithm`, `rsa_private_key_env`,
`rsa_public_key_env`, `rsa_private_key_pem`, and `rsa_public_key_pem`.
`HS256` remains the default. `RS256` requires both RSA keys and must fail
config assembly instead of silently downgrading to `HS256`.

### `crates/rustok-cache`

`Core` module управления кэшем: Redis-клиент (одна точка подключения), in-memory fallback (Moka), `CacheService::health()` с PING-проверкой. **Заменяет** `ctx.config.cache`. Инициализируется в `bootstrap_app_runtime`, доступен через `ctx.shared_store.get::<CacheService>()`.

Redis URL задаётся через (в порядке приоритета):
1. `settings.rustok.cache.redis_url` в YAML
2. env `RUSTOK_REDIS_URL`
3. env `REDIS_URL`

### `crates/rustok-email`

`Core` module email-рассылок: SMTP через lettre, Tera-шаблоны. **Заменяет** Loco Mailer как primary transport. Фабрика `email_service_from_ctx(ctx, locale)` в `apps/server/src/services/email.rs` выбирает провайдер (`smtp | loco | none`). SMTP-транспорт кэшируется в `shared_store` через `SharedSmtpEmailService`.

Два публичных trait:
- `BuiltInAuthEmailSender` в `apps/server/src/services/email.rs` — локализованный runtime-контракт для built-in auth email flows (password reset + email verification)
- `TransactionalEmailSender` — общий контракт для любых transactional email по template ID (`"{module}/{action}"`, напр. `"commerce/order_confirmed"`). Модули регистрируют шаблоны через `EmailTemplateProvider`; `SmtpEmailSender::with_provider()` подключает провайдер.

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

## Известные ложные ошибки компиляции

При выполнении `cargo check -p rustok-server` без собранного фронтенда появляются ошибки:

```
error: #[derive(RustEmbed)] folder 'apps/admin/dist' does not exist
error[E0599]: no function or associated item named `get` found for struct `AdminAssets`
```

**Это не баги** — это ожидаемое поведение feature `embed-admin-assets`. Фича требует предварительного билда `apps/admin/dist` (`trunk build` или `npm run build`). В CI/dev-окружениях без фронтенд-артефактов фича отключена по умолчанию, и `/admin/*` возвращает `503`. Ошибки в `app_router.rs` при проверке кода без артефактов — норма, игнорировать.

Проверять только ошибки в изменённых файлах; ошибки в `services/app_router.rs` / `AdminAssets` не связаны с логикой сервера.

## Do / Don't

### Do

- Используй только реально существующие API из кода и docs.
- Для доменных write-flow с событиями применяй `publish_in_tx`, когда нужен атомарный publish.
- Проверяй, что docs отражают текущий код, а не старые архитектурные предположения.
- Для Leptos UI сначала проектируй локальный API-слой `view -> local api -> #[server]`, а GraphQL оставляй как параллельный transport/fallback.
- Для optional-wave module-owned Leptos admin surfaces текущий baseline такой: `rustok-media-admin` работает по модели native `#[server]` first с GraphQL fallback для `list/detail/translations/delete/usage` и с сохранённым REST-first upload path; `rustok-comments-admin` работает по модели native-only `#[server]`, потому что legacy GraphQL/REST transport surface у `comments` не существовал.
- `rustok-content` остаётся shared helper/orchestration boundary без собственного operator-facing UI.
- Commerce split crates (`cart`, `customer`, `product`, `profiles`, `region`, `pricing`, `inventory`, `order`, `payment`, `fulfillment`) не получают отдельные admin UI в этой волне и продолжают жить под aggregate surface `rustok-commerce-admin`.

### Don't

- Не придумывай третий тип модулей кроме `Core` и `Optional`.
- Не подменяй архитектурный статус модуля способом runtime wiring.
- Не обходи outbox в production event-flow.
- Не удаляй GraphQL resolver/path только потому, что рядом появился native `#[server]` путь.
