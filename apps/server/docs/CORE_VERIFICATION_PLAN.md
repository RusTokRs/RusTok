# Периодический план верификации ядра RusToK

**Дата создания:** 2026-03-12
**Частота проверки:** при каждом существенном PR / по расписанию

> Этот документ — чеклист для периодической проверки ядра платформы.
> Цель: убедиться, что ядро сохраняет целостность архитектуры, AI-агенты
> не внедрили дублирующий самопис, и все контракты работают корректно.

---

## 1. Core Agnosticism — ядро не знает о доменных модулях

> [!CAUTION]
> Самая частая ошибка агентов — добавить domain-specific код прямо в server.

### 1.1 Hard-coded imports в ядре

```bash
# В apps/server/src/ НЕ ДОЛЖНО быть прямых use из доменных модулей
# (кроме ModuleRegistry / trait imports)
grep -rn "use rustok_content" apps/server/src/ --include="*.rs"
grep -rn "use rustok_commerce" apps/server/src/ --include="*.rs"
grep -rn "use rustok_blog" apps/server/src/ --include="*.rs"
grep -rn "use rustok_forum" apps/server/src/ --include="*.rs"
grep -rn "use rustok_pages" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** Нет результатов, кроме:
- `graphql/schema.rs` — KNOWN ISSUE до Фазы 4 (dynamic registration).
- `graphql/{blog,commerce,forum,pages}/` — допустимо ТОЛЬКО если модуль-scope GraphQL.
- `app.rs` — регистрация модулей через `ModuleRegistry::register()`.

### 1.2 `schema.rs` — проверка на domain coupling

```bash
grep -n "Query\|Mutation" apps/server/src/graphql/schema.rs
```

**Ожидаемый результат (текущий/known):** product `ContentQuery`/`ContentMutation` уже выведены из runtime; в schema остаются только доменные query/mutation root'ы.

### 1.3 `rustok-core` — не содержит domain logic

```bash
# core НЕ ДОЛЖЕН знать о конкретных модулях
grep -rn "content\|commerce\|blog\|forum\|pages" crates/rustok-core/src/ --include="*.rs"
```

**Ожидаемый результат:** Нет совпадений (допустимы имена в doc comments / module examples).

---

## 2. Кеширование — собственная реализация, не Loco Cache

### 2.1 Никто не добавил `loco_rs::cache`

```bash
grep -rn "loco_rs::cache\|loco_rs::prelude::cache\|CacheDriver" apps/server/src/ crates/ --include="*.rs"
```

**Ожидаемый результат:** Нет совпадений. Используется ТОЛЬКО `rustok_core::CacheBackend`.

### 2.2 CacheBackend trait не изменён без ADR

Проверить файл `crates/rustok-core/src/context.rs` — trait `CacheBackend` должен содержать:
- `health()`, `get()`, `set()`, `set_with_ttl()`, `invalidate()`, `stats()`

### 2.3 FallbackCacheBackend жив

```bash
grep -rn "FallbackCacheBackend" crates/rustok-core/src/cache.rs
```

**Ожидаемый результат:** Struct + impl блок существуют.

### 2.4 Circuit breaker на Redis backend

```bash
grep -rn "CircuitBreaker" crates/rustok-core/src/cache.rs
```

**Ожидаемый результат:** Используется в `RedisCacheBackend`.

### 2.5 Anti-stampede coalescing работает

```bash
grep -rn "in_flight\|get_or_load_with_coalescing" apps/server/src/middleware/tenant.rs
```

**Ожидаемый результат:** Оба присутствуют в `TenantCacheInfrastructure`.

---

## 3. Event Bus — Outbox, не Loco Queue

### 3.1 Никто не добавил Loco Queue для событий

```bash
grep -rn "loco_rs::bgworker\|loco_rs::queue\|QueueProvider" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** Нет совпадений (кроме Hooks::connect_workers signature).

### 3.2 Outbox relay жив и настроен

```bash
grep -rn "spawn_outbox_relay_worker\|OutboxRelay" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** Вызывается в `app.rs` / `connect_workers`.

### 3.3 EventTransport trait не изменён

Файл `crates/rustok-core/src/events.rs` — trait `EventTransport`.

### 3.4 Transactional event bus работает через outbox

```bash
grep -rn "TransactionalEventBus" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** Используется в GraphQL schema и сервисных слоях.

---

## 4. Email — provider-based server infra

### 4.1 Email service существует

```bash
ls apps/server/src/services/email.rs
```

### 4.2 Provider switch и Loco Mailer adapter подключены централизованно

```bash
grep -rn "EmailProvider\|loco_rs::mailer\|LocoMailerAdapter\|email.provider" apps/server/src/services/email.rs apps/server/src/common/settings.rs
```

**Ожидаемый результат:** Почтовый runtime остаётся server-infra responsibility и поддерживает `smtp|loco|none`; при `provider=loco` используется `ctx.mailer`, при `provider=smtp` остаётся compatibility path через SMTP.

### 4.3 Шаблоны не захардкожены в send-path

```bash
grep -rn "include_str!\|mailers/auth/password_reset" apps/server/src/services/email.rs
```

**Ожидаемый результат:** Auth email рендерится через файловые шаблоны (`mailers/...`) и единый server email service, а не через inline HTML literals в бизнес-логике.

---

## 5. Settings — YAML vs DB

### 5.1 `RustokSettings` и DB overrides существуют одновременно

```bash
grep -rn "SettingsService\|platform_settings" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** typed settings живут в `settings.rustok.*`, а `SettingsService`/`platform_settings` обеспечивают per-tenant DB overrides и validation layer.

### 5.2 YAML не дублирует DB

Проверить `config/development.yaml` — должен содержать только bootstrap defaults.

### 5.3 Module settings не должны притворяться универсально реализованными

```bash
# GraphQL query tenantModules должен возвращать settings != {}
curl -s http://localhost:5150/graphql -H 'Content-Type: application/json' \
  -d '{"query":"{ tenantModules { moduleslug settings } }"}' | jq '.data.tenantModules[] | select(.settings == "{}")'
```

**Ожидаемый результат:** Для модулей, где settings UI/contract уже заявлены как активные, не должно быть бессмысленного пустого `{}`. Если модульные settings ещё не formalized, это должно быть явно отражено в их docs, а не маскироваться под завершённый runtime.

---

## 6. i18n — текущий request locale contract

### 6.1 Locale resolution живёт в request context и middleware

```bash
grep -rn "RequestContext\|Accept-Language\|rustok-admin-locale\|extract_requested_locale\|resolve_locale" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** Каноническая цепочка locale resolution (`query -> x-medusa-locale -> cookie -> Accept-Language(q-values) -> tenant.default_locale -> en`) собрана в request context, а middleware/GraphQL используют тот же effective locale.

### 6.2 API ошибки локализованы

```bash
# Проверить что FieldError сообщения не hardcoded
grep -rn "FieldError::new(\"" apps/server/src/graphql/ --include="*.rs" | head -20
```

**Ожидаемый результат:** Новые transport-ошибки не должны зашивать пользовательские строки напрямую, если для них уже существует i18n path/translation key.

### 6.3 Module-owned translation bundles фиксируются через manifest-level contract

```bash
grep -rn "leptos_locales_path\|next_messages_path\|supported_locales\|default_locale" crates/rustok-*/rustok-module.toml
```

**Ожидаемый результат:** Если модуль заявляет `[provides.*_ui.i18n]`, manifest должен описывать `supported_locales`, `default_locale` и bundle paths. `ManifestManager` валидирует existence/shape этого контракта, а docs не должны снова описывать слой как неопределённый trait/WIP.

### 6.4 UI locale bundles parity проверяется машинно

```bash
npm run verify:i18n:ui
```

**Ожидаемый результат:** `locales/*.json` и `messages/*.json` в host apps и module-owned UI packages проходят проверку на parity ключей. Manifest-level existence contract и parity contract должны жить вместе: первый ловится `ManifestManager`, второй — verifier-скриптом.

### 6.5 Password reset email использует effective locale независимо от transport/provider

```bash
grep -rn "email_service_from_ctx\|request_context.locale\|locale_from_ctx" apps/server/src/controllers apps/server/src/graphql apps/server/src/services/email.rs --include="*.rs"
```

**Ожидаемый результат:** GraphQL и REST password-reset paths прокидывают effective locale в `email_service_from_ctx(...)`, а `smtp` и `loco` providers рендерят один и тот же localized built-in template path вместо hardcoded English-only SMTP body.

---

## 7. RBAC — единый Casbin runtime

### 7.1 Server использует модульный `rustok-rbac` runtime

```bash
grep -rn "RbacService::has_permission\|RbacService::has_any_permission" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** Проверки проходят через `RbacService`/общие `rustok-rbac` helpers, а не через локальный policy engine в `apps/server`.

Проверить, что server wiring не вернул локальную authorization semantics:

```bash
grep -rn "RuntimePermissionResolver\|authorize_permission\|authorize_any_permission\|authorize_all_permissions" apps/server/src/services/ --include="*.rs"
```

**Ожидаемый результат:** `apps/server` использует resolver/adapters из `rustok-rbac`, а decision path опирается на модульный Casbin runtime.

### 7.2 Нет legacy rollout branches и дублирующих auth middleware

```bash
grep -rn "fn check_permission\|fn verify_role\|relation_only\|casbin_shadow\|mismatch\|RUSTOK_RBAC_AUTHZ_MODE" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** Нет adhoc проверок мимо `RbacService` и нет возврата к старым rollout/shadow веткам в live server runtime.

---

## 8. Tenant Resolution — инфраструктура

### 8.1 Tenant middleware работает

```bash
grep -rn "TenantCacheInfrastructure\|init_tenant_cache_infrastructure" apps/server/src/ --include="*.rs"
```

### 8.2 Redis pub/sub invalidation listener

```bash
grep -rn "TENANT_INVALIDATION_CHANNEL\|spawn_invalidation_listener" apps/server/src/ --include="*.rs"
```

### 8.3 Negative cache существует

```bash
grep -rn "negative_cache\|set_negative\|check_negative" apps/server/src/middleware/tenant.rs
```

---

## 9. Loco Integration — правильное использование Framework

### 9.1 Все маршруты через Hooks::routes / after_routes

```bash
# Не должно быть standalone Router::new() без интеграции в Loco
grep -rn "axum::Router::new()" apps/server/src/ --include="*.rs" | grep -v "test"
```

**Ожидаемый результат:** Нет (или только в модульных sub-routers, подключённых через Loco).

### 9.2 AppContext через State, не глобальные переменные

```bash
grep -rn "lazy_static\|static.*OnceCell\|static.*Mutex" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** Нет runtime state мимо `AppContext.shared_store`.

### 9.3 Ошибки через loco_rs::Result

```bash
# Handlers должны возвращать loco Result, не axum IntoResponse напрямую
grep -rn "impl IntoResponse" apps/server/src/controllers/ --include="*.rs"
```

**Ожидаемый результат:** Нет custom IntoResponse в controllers.

---

## 10. Модульная система — целостность

### 10.1 ModuleRegistry содержит все зарегистрированные модули

```bash
grep -rn "ModuleRegistry::new()\|\.register(" apps/server/src/modules/ apps/server/src/services/module_lifecycle.rs
```

### 10.2 Module lifecycle hooks вызываются

```bash
grep -rn "on_enable\|on_disable" apps/server/src/services/module_lifecycle.rs
```

### 10.3 `modules.toml` манифест валиден

```bash
# Проверить что все модули из modules.toml имеют соответствующие crate
cat modules.toml
```

---

## 11. Storage — shared `rustok-storage` / `rustok-media` contract

> `rustok-storage` = shared storage backend/service layer. `rustok-media` = core domain module поверх storage. Server только инициализирует общий runtime и прокидывает его в consumers.

### 11.1 Shared storage service инициализируется в app runtime

```bash
grep -rn "StorageService\|init_storage" apps/server/src/ --include="*.rs"
```

**Ожидаемый результат:** `StorageService` создаётся один раз в runtime bootstrap и используется как shared dependency, а не как adhoc storage client по месту вызова.

### 11.2 Контроллеры не тащат storage backend напрямую

```bash
# Прямой backend wiring в controllers — антипаттерн
grep -rn "loco_rs::storage\|StorageService::from_config" apps/server/src/controllers/ --include="*.rs"
```

**Ожидаемый результат:** Backend wiring остаётся в runtime/bootstrap слое; controllers/graphql/services используют уже готовый shared storage contract.

### 11.3 Файлы организованы по дате и tenant

```bash
# Проверить что storage используется вместе с media/runtime cleanup flows
grep -rn "media_cleanup\|storage_path\|MediaService" apps/server/src/ crates/rustok-media/src/ --include="*.rs"
```

### 11.4 media_assets таблица существует

```bash
grep -rn "media_assets" apps/server/src/models/ migration/ --include="*.rs"
```

### 11.5 Нет ad-hoc upload мимо StorageAdapter

```bash
grep -rn "multipart\|tokio::fs::write\|std::fs::write" apps/server/src/controllers/ --include="*.rs"
```

**Ожидаемый результат:** Нет — все через `StorageAdapter`.

---

## 12. Observability — telemetry и health

### 12.1 Telemetry initializer

```bash
ls apps/server/src/initializers/telemetry.rs
```

### 12.2 Health endpoint работает

```bash
curl -s http://localhost:5150/api/_health | jq .
```

### 12.3 Metrics endpoint

```bash
curl -s http://localhost:5150/api/_metrics | head -20
```

---

## 13. Антипаттерны — что НЕ должно появиться

| Антипаттерн | Как обнаружить | Серьёзность |
|---|---|---|
| Loco Cache вместо CacheBackend | `grep "loco_rs::cache"` | 🔴 Критичная |
| Loco Queue вместо Outbox | `grep "loco_rs::bgworker" \| grep "QueueProvider"` | 🔴 Критичная |
| Domain imports в core crate | `grep "content\|commerce" crates/rustok-core/` | 🔴 Критичная |
| Static globals мимо AppContext | `grep "lazy_static\|OnceCell"` | 🟡 Высокая |
| Inline SQL вместо SeaORM | `grep "raw_sql\|execute_unprepared"` в новом коде | 🟡 Высокая |
| Новый HTTP client вместо Loco fetch | `grep "reqwest::Client::new()"` в ядре | 🟢 Средняя |
| Custom auth middleware мимо RbacService | Ручной `fn check_role` | 🟡 Высокая |
| Hard-coded tenant ID в бизнес-логике | `grep "00000000-0000-0000-0000"` в не-config файлах | 🟡 Высокая |

---

## Как проводить верификацию

1. **Автоматическая:** Добавить проверки из §§1–11 в CI как lint step (grep-based). Fail on match.
2. **Ручная (периодическая):** Пройти чеклист вручную раз в спринт / при крупном PR.
3. **Agent pre-commit:** AI-агенты обязаны сверяться с этим документом перед любым изменением в `apps/server/` или `crates/rustok-core/`.

> **Правило для агентов:** Если вы собираетесь добавить новую зависимость, middleware, или
> инфраструктурный сервис в `apps/server/` — **сначала** проверьте по этому документу,
> не дублирует ли это уже существующее решение.


