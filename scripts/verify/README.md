# RusTok — Verification Scripts

Автоматизированные проверки платформы, встроенные в общий verification workflow. Точка входа для ручного orchestration-прогона: [PLATFORM_VERIFICATION_PLAN.md](../../docs/verification/PLATFORM_VERIFICATION_PLAN.md).

## Быстрый старт

```bash
# Запустить ВСЕ проверки (краткий вывод)
./scripts/verify/verify-all.sh

# Запустить ВСЕ проверки (полный вывод)
./scripts/verify/verify-all.sh -v

# Запустить одну категорию
./scripts/verify/verify-all.sh tenant-isolation
./scripts/verify/verify-all.sh api-quality
./scripts/verify/verify-all.sh deployment-profiles

# Запустить скрипт напрямую (всегда полный вывод)
./scripts/verify/verify-tenant-isolation.sh
./scripts/verify/verify-deployment-profiles.sh
./scripts/verify/verify-migration-smoke.sh
node scripts/verify/verify-flex-multilingual-contract.mjs
node scripts/verify/verify-module-lifecycle-bypass-usage.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-registry.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fallback-profiles.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs pages
```

## Когда запускать

| Ситуация | Команда |
|----------|---------|
| Перед коммитом | `./scripts/verify/verify-all.sh` |
| После рефакторинга модуля | `./scripts/verify/verify-all.sh -v` |
| Ревью PR | `./scripts/verify/verify-all.sh -v` |
| Добавили новый endpoint | `./scripts/verify/verify-all.sh api-quality` |
| Добавили новый event | `./scripts/verify/verify-all.sh events` |
| Проверка anti-bypass drift | `./scripts/verify/verify-all.sh anti-bypass` |
| Добавили миграцию | `./scripts/verify/verify-all.sh tenant-isolation` + `./scripts/verify/verify-migration-smoke.sh`; в CI тот же smoke закреплён отдельным job `migration-smoke` |
| Подозрение на дыру в RBAC | `./scripts/verify/verify-all.sh rbac-coverage` |
| Аудит безопасности | `./scripts/verify/verify-security.sh` |
| Проверка deployment profile matrix | `./scripts/verify/verify-all.sh deployment-profiles` |
| Проверка drift в Flex multilingual contract | `node scripts/verify/verify-flex-multilingual-contract.mjs` |
| Проверка runtime-context/cache-key invariants | `node scripts/verify/verify-runtime-context-invariants.mjs` |
| Проверка inventory admin native/write boundary | `node scripts/verify/verify-inventory-admin-boundary.mjs` |
| Проверка AI admin FFA boundary | `node scripts/verify/verify-ai-admin-boundary.mjs` |
| Проверка tenant admin FFA boundary | `node scripts/verify/verify-tenant-admin-boundary.mjs` |
| Проверка запрета lifecycle bypass helper в production | `node scripts/verify/verify-module-lifecycle-bypass-usage.mjs` |
| Проверка parity provider/consumer для page-builder контракта | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs` |
| Проверка machine-readable registry page-builder против manifests | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-registry.mjs` |
| Проверка required fallback/toggle профилей page-builder | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fallback-profiles.mjs` |
| Проверка консистентности значений в toggle профилях page-builder | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs` |
| Полный baseline gate page-builder FBA перед Wave 0/Wave 1 | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs` |
| Проверка readiness consumer-модуля (`pages/forum`) | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs <slug>` |

Альтернативно те же проверки доступны через `npm run`:

```bash
npm run verify:page-builder:contract-parity
npm run verify:page-builder:fallback-profiles
npm run verify:page-builder:toggle-profiles
npm run verify:page-builder:fba:baseline
npm run verify:page-builder:consumer:pages
npm run verify:page-builder:consumer:forum
```

## Описание скриптов

### `verify-migration-smoke.sh`
**Wave 4 migration-safety smoke** — PostgreSQL apply-from-zero для server migrator.

Что делает:
- создаёт временную PostgreSQL database через `RUSTOK_MIGRATION_SMOKE_ADMIN_URL` внутри Rust integration test, без зависимости от локального `psql`;
- запускает ignored integration test `postgres_zero_migration_smoke_applies_from_empty_database`;
- применяет `migration::Migrator` с нуля и проверяет, что pending migrations не осталось;
- при `RUSTOK_MIGRATION_SMOKE_INCREMENTAL=1` применяет миграции по одной, чтобы отдельно проверить incremental apply path; shell script и Rust test одинаково принимают только `0`/`1`, поэтому direct test runs не обходят эту валидацию;
- проверяет наличие representative platform/module tables (`tenants`, `product_variants`, `prices`, `inventory_items`, `channels`, `oauth_apps`, `blog_post_tags`, `forum_topic_tags`, `taxonomy_terms`);
- удаляет временную database из Rust test, если `RUSTOK_MIGRATION_SMOKE_KEEP_DB=1` не установлен.

Пример:

```bash
RUSTOK_MIGRATION_SMOKE_ADMIN_URL=postgres://postgres:postgres@localhost:5432/postgres \
  ./scripts/verify/verify-migration-smoke.sh

RUSTOK_MIGRATION_SMOKE_INCREMENTAL=1 \
RUSTOK_MIGRATION_SMOKE_ADMIN_URL=postgres://postgres:postgres@localhost:5432/postgres \
  ./scripts/verify/verify-migration-smoke.sh
```

---

### `verify-runtime-context-invariants.mjs`
**Wave 6 runtime-context guardrail** — быстрый source-level gate для уже исправленных P0/P1 invariants без полной Rust-компиляции.

Что проверяет:
- `ChannelCacheKey` содержит OAuth/client и locale dimensions;
- `RequestFacts` берёт `oauth_app_id` из `AuthContextExtension`, а `locale` — из `ResolvedRequestLocale.effective_locale`;
- source-order middleware в `compose_application_router` сохраняет фактический порядок выполнения Axum `locale -> auth_context -> channel`;
- tenant locale cache metrics экспортируют counter names с `_total` и gauge `rustok_tenant_locale_cache_entries`;
- `modules.toml` и central registry evidence сохраняют `pages -> [content, page_builder]`.

Пример:

```bash
node scripts/verify/verify-runtime-context-invariants.mjs
./scripts/verify/verify-all.sh runtime-context-invariants
```

---

### `verify-inventory-admin-boundary.mjs`
**Wave 5/Wave 6 inventory guardrail** — быстрый source-level gate для inventory-owned admin read/write boundary без полной Rust-компиляции.

Что проверяет:
- `InventoryQuantityWriteResult` строит `inStock` из committed quantity и backorder policy;
- native `set_variant_quantity`/`adjust_variant_quantity` используют internal mutation update result и не делают отдельный pre-read variant policy;
- removed GraphQL fallback stays removed: no `src/transport.rs`, `leptos-graphql`, `CommerceGraphqlInventoryReadAdapter`, GraphQL runtime markers, token/tenant-slug fallback inputs or `mod transport`;
- admin API read facades fetch-bootstrap/products/product and write facades set/adjust/reserve/release/check-availability go through inventory-owned native facades without GraphQL fallback;
- native server-function endpoints for inventory read/write/validation surfaces remain declared;
- commerce storefront/public-channel callers use inventory-owned availability/projection facades instead of direct loaders/backorder branching;
- admin UI/locales describe the native inventory facade and docs mark current admin stock operations as native/API covered.

Пример:

```bash
node scripts/verify/verify-inventory-admin-boundary.mjs
./scripts/verify/verify-all.sh inventory-admin-boundary
node scripts/verify/verify-inventory-admin-boundary.test.mjs
```

---


### `verify-ai-admin-boundary.mjs` / `verify-tenant-admin-boundary.mjs`
**FFA admin guardrails** — быстрые source-level checks для module-owned admin UI splits без полной Rust-компиляции.

Что проверяют:
- crate root wires `core`, `transport` and explicit `ui/leptos.rs` adapters;
- Leptos adapters consume module-owned transport facades instead of pre-FFA `api::` calls;
- `core.rs` stays Leptos/server-function/runtime free;
- native server-function endpoints stay inside `transport/native_server_adapter.rs`;
- old flat `api.rs` facades do not return for completed slices.

Пример:

```bash
npm run verify:ai:admin-boundary
npm run verify:tenant:admin-boundary
npm run verify:ffa:ui:migration
```

---

### `verify-tenant-isolation.sh`
**Фаза 19.1 + 5** — Multi-tenancy safety

Что ищет:
- `.all(&db)` без `.filter(tenant_id)` — загрузка данных чужого tenant
- `find_by_id` без tenant_id проверки — доступ к чужому ресурсу по ID
- `DELETE` без tenant_id filter — удаление данных чужого tenant
- Миграции: каждая domain-таблица имеет `tenant_id` column
- SeaORM entities: `pub tenant_id` в Model struct
- Raw SQL строки (SQL injection risk)
- Hard DELETE без soft-delete (архивации)

**Severity:** CRITICAL. Нарушение = утечка данных между tenant-ами.

---

### `verify-unsafe-code.sh`
**Фаза 19.1 + 19.3** — Runtime safety

Что ищет:
- `.unwrap()` — паника при None/Err
- `.expect()` — паника с сообщением (review each)
- `panic!()` — явная паника
- `todo!()` / `unimplemented!()` — недописанный код
- `std::thread::sleep` — блокировка tokio runtime
- `std::fs::` — блокирующий I/O в async
- `block_on()` — deadlock в async context
- `println!` / `eprintln!` — должно быть tracing::
- `unreachable!()` — оправдан ли?
- `static` / `lazy_static!` / `once_cell::Lazy` — should use AppContext
- `unwrap_or("default")` для секретов — unsafe fallback

**Severity:** HIGH. Паника крашит весь tokio runtime.

---

### `verify-rbac-coverage.sh`
**Фаза 19.2** — Authorization coverage

Что ищет:
- REST handlers без RBAC extractors (`Require*`, `Permission`)
- GraphQL mutations без permission checks
- GraphQL queries без auth context
- Auth middleware зарегистрирован в router

**Severity:** CRITICAL. Отсутствие RBAC = privilege escalation.

---

### `verify-api-quality.sh`
**Фаза 19.12–19.14** — API correctness

Что ищет:

**GraphQL:**
- N+1 queries — direct DB access в resolvers (должен быть DataLoader)
- `MergedObject` — модульная schema (не монолитная)
- String errors — должны быть error extensions
- `TenantContext` — в каждом resolver
- Пагинация в list queries

**REST:**
- `#[utoipa::path]` — OpenAPI annotation на каждый endpoint
- HTTP status codes: 201 для POST, 204 для DELETE
- Input validation через `validator::Validate`
- Rate limiting на auth endpoints
- CORS middleware

**Parity:**
- Auth операции доступны и через REST, и через GraphQL
- Единый `AuthLifecycleService` (не дублированная логика)
- Бизнес-логика не в controllers/resolvers

**Severity:** HIGH. N+1 = ×50 latency. Missing OpenAPI = нет документации.

---

### `verify-events.sh`
**Фаза 6 + 19.1** — Event system integrity

Что ищет:
- `publish()` без `_in_tx` — данные сохранятся, событие потеряется
- `tenant_id` в каждом DomainEvent struct
- Event handlers зарегистрированы
- Outbox pattern реализован
- DLQ (Dead Letter Queue) существует
- Event versioning
- Idempotency guards в handlers
- Transport config (не "memory" в production)
- `#[derive(Serialize, Deserialize)]` на event structs

**Severity:** CRITICAL. publish без _in_tx = потеря событий при rollback.

---

### `verify-code-quality.sh`
**Фаза 19.4–19.11** — Code health

Что ищет:

**Security:**
- PII в логах (password, email, token в tracing)
- Hardcoded secrets в коде
- `.env` файлы в git
- Entities возвращаются напрямую в API (должны быть Response DTOs)

**Metrics:**
- Файлы > 500 строк
- Функции > 60 строк (top 10)
- Функции с > 5 аргументами

**Dependencies:**
- `rustok-core` не зависит от domain crates
- Domain crates не зависят друг от друга
- `rustok-test-utils` только в `[dev-dependencies]`

**Error handling:**
- `thiserror` в domain crates (не `anyhow`)
- String-based status checks (должны быть enum)

**Observability:**
- `#[instrument]` decorator на service methods
- Structured logging fields (не string interpolation)

**Type safety:**
- Newtype IDs (`TenantId`, `UserId`), не bare `Uuid`

**Severity:** HIGH. PII в логах = GDPR violation.

---

### `verify-security.sh`
**Фаза 18** — Security audit

Что ищет:
- Argon2 для password hashing (не MD5/SHA256/bcrypt)
- Security headers (CSP, X-Frame-Options, HSTS) в middleware
- SSRF protection (allowlist для внешних HTTP запросов)
- `zeroize` для sensitive data в памяти
- JWT secret через env var (без fallback defaults)
- Token invalidation при смене пароля

**Severity:** CRITICAL. Weak hashing = compromise всех паролей.

---

### `verify-architecture.sh`
**Фаза 1 + 5** — Architectural compliance

Что ищет:
- Module dependencies: `dependencies()` trait совпадает с `modules.toml`
- Loco Hooks: все routes через `Hooks::routes()`, не напрямую
- Module registry: все модули зарегистрированы через `build_registry()`
- Core-модули не toggleable (`ModuleKind::Core`)
- MCP tools используют `McpToolResponse` (не raw JSON)
- Controller return types: `loco_rs::Result` (не custom)
- Dependency guard (`cargo metadata` + allow/deny):
  - backend apps (в текущей конфигурации: `rustok-server`) → только `rustok-*` crate-зависимости (кроме явных infra-исключений)
  - deny новых междоменных `rustok-* -> rustok-*` связей вне allow-list

---

### `verify-page-builder-contract-parity.mjs`
**Page Builder FBA baseline** — Provider/consumer version parity

Что проверяет:
- `builder_contract_version` между `rustok-page-builder` (provider) и `rustok-pages` (consumer);
- `consumer_min_version` в provider-манифесте и условие `consumer.builder_contract_version >= provider.consumer_min_version`;
- `contract_version` в consumer-манифесте относительно версии provider.

**Severity:** HIGH. Drift версий контракта блокирует безопасный rollout между Wave 0/Wave 1.

---


### `verify-page-builder-contract-registry.mjs`
**Page Builder FBA baseline** — Machine-readable registry anti-drift

Что проверяет:
- `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` существует и имеет `schema_version = 1`;
- provider metadata (`contract`, `builder_contract_version`, `consumer_min_version`, capabilities) совпадает с `rustok-page-builder/rustok-module.toml`;
- выбранный consumer (`pages` или `forum`) совпадает с registry по `contract_version`, `builder_contract_version`, `consumer_min_version` и capabilities;
- consumer version не ниже provider `consumer_min_version`.

**Severity:** HIGH. Registry drift блокирует Wave 0/Wave 1 promotion, потому что contract freeze становится непроверяемым.

### `verify-page-builder-fallback-profiles.mjs`
**Page Builder FBA baseline** — Required fallback/toggle structure

Что проверяет:
- наличие секций `fba.builder_consumer.degraded_modes` и `fba.builder_consumer.toggle_profiles`;
- обязательные ключи degraded modes и профилей (`all_on/publish_off/preview_off/builder_off`);
- наличие обязательных toggle-флагов и typed degraded-mode для publish-disable path.

**Severity:** HIGH. Отсутствие fallback-структуры ведёт к неуправляемой деградации при отключении capability.

---

### `verify-page-builder-toggle-profiles-consistency.mjs`
**Page Builder FBA baseline** — Toggle profile value consistency

Что проверяет:
- что в каждом профиле (`all_on/publish_off/preview_off/builder_off`) флаги имеют ожидаемые boolean-комбинации;
- что dry-run rollout semantics остаются детерминированными.

**Severity:** HIGH. Неконсистентные профили делают tenant-toggle rollout непредсказуемым.

---

### `verify-page-builder-fba-baseline.mjs`
**Page Builder FBA baseline** — Aggregate gate

Что делает:
- последовательно запускает:
  1) `verify-page-builder-contract-parity.mjs`,
  2) `verify-page-builder-contract-registry.mjs <module-slug>`,
  3) `verify-page-builder-consumer-readiness.mjs <module-slug>` (по умолчанию `pages` в агрегаторе),
  4) `verify-page-builder-fallback-profiles.mjs <module-slug>`,
  5) `verify-page-builder-toggle-profiles-consistency.mjs <module-slug>`,
  6) `verify-page-builder-terminology.mjs`.
- возвращает non-zero exit code при падении любого шага.

**Severity:** GATE. Это канонический baseline-check перед promotion в следующий rollout wave.

---

### `verify-page-builder-consumer-readiness.mjs`
**Page Builder FBA baseline** — Consumer readiness check

Что проверяет:
- наличие `rustok-module.toml` и `docs/implementation-plan.md` для модуля-consumer;
- наличие marker-ов dependency/consumer contract (`page_builder`/`builder_consumer`, `contract_version`, `builder_contract_version`);
- наличие `Execution checkpoint` и FBA/page-builder readiness notes в implementation-plan;
- для `pages`: manifest/docs rollout policy markers для `control_plane_builder_wave_audit`, before/after snapshots, keep/rollback decision, owner sign-off, SLO rollback triggers, pilot smoke `preview -> properties -> publish(dry)` и rollback target <= 10 минут без redeploy.

Поддерживаемые slug:
- `pages`
- `forum`

**Severity:** MEDIUM. Скрипт проверяет structural readiness перед включением модуля в rollout wave.
  - deny nested imports внутренних модулей без явного разрешения

**Severity:** CRITICAL. Модуль вне registry = не проходит health check.

---

### `verify-deployment-profiles.sh`
Smoke-check поддерживаемых build surfaces:

- `monolith` — default feature set + startup smoke
- `server+admin` — `--no-default-features --features redis-cache,embed-admin`
- `headless-api` — `--no-default-features --features redis-cache`
- `registry-only` — runtime host mode `RUSTOK_RUNTIME_HOST_MODE=registry_only` поверх минимального headless feature-profile (`--no-default-features --features redis-cache`)

Скрипт запускает по каждой конфигурации `cargo check` и профильный smoke-test router/startup. Для
`registry-only` дополнительно проверяются env override `RUSTOK_RUNTIME_HOST_MODE=registry_only`,
суженный runtime surface и reduced OpenAPI, чтобы deployment contract для read-only catalog host не
расползался между docs и фактическим runtime.
Дополнительно для `registry-only` матрица уже держит `GET /v1/catalog/{slug}` detail-path,
cache-contract через `ETag` / `If-None-Match` и negative smoke на write-route-ы
`POST /v2/catalog/publish`, `POST /v2/catalog/publish/{request_id}/validate`,
`POST /v2/catalog/publish/{request_id}/stages`,
`POST /v2/catalog/publish/{request_id}/request-changes`,
`POST /v2/catalog/publish/{request_id}/hold`,
`POST /v2/catalog/publish/{request_id}/resume`,
`POST /v2/catalog/runner/claim`, `POST /v2/catalog/owner-transfer` и
`POST /v2/catalog/yank`.

Для уже развёрнутого dedicated host тот же скрипт теперь умеет optional external smoke:

```bash
RUSTOK_REGISTRY_BASE_URL=https://modules.rustok.dev \
RUSTOK_REGISTRY_SMOKE_SLUG=blog \
RUSTOK_REGISTRY_EVIDENCE_DIR=./tmp/modules-rustok-dev-smoke \
./scripts/verify/verify-deployment-profiles.sh
```

PowerShell-вариант поддерживает тот же contract через env vars
`RUSTOK_REGISTRY_BASE_URL`, optional `RUSTOK_REGISTRY_SMOKE_SLUG` и optional
`RUSTOK_REGISTRY_EVIDENCE_DIR`. Если evidence dir задан, external smoke сохраняет туда
`runtime-*`, `catalog-*`, `openapi-*` snapshots и `registry-smoke-metadata.txt`, а negative
smoke покрывает тот же expanded V2 surface (`publish`, `validate`, `stages`,
`request-changes`, `hold`, `resume`, `runner/claim`, `owner-transfer`, `yank`).

Для Windows / PowerShell используйте `./scripts/verify/verify-deployment-profiles.ps1`: он
покрывает ту же матрицу профилей развёртывания, когда `bash` недоступен в локальном окружении.

**Severity:** HIGH. Поломка profile matrix = build contract задокументирован, но не воспроизводим.

---


### `verify-anti-bypass.sh`
**Фаза 19.15** — Anti-bypass audit

Что ищет (кандидаты для ручного review):
- Повтор валидации доменных правил в `apps/server` и frontend-adapter слое
- Ручная публикация событий в app-слое вместо модульного сервиса
- Прямые запросы к доменным таблицам мимо crate API
- Контрольные сигнатуры orchestration-only (вызовы domain service)

Режимы:
- `--manual-review` — расширенный вывод кандидатов
- `--strict` — найденные кандидаты считаются ошибкой (use in CI gate при необходимости)

Важно: anti-bypass аудит не требует «бездумно всё выносить в модули». Разбор кандидатов делается вручную с учётом допустимого platform/core слоя и frontend-library слоя.

**Severity:** MEDIUM→HIGH. Цель — системно ловить drift и фиксировать migration-task с корректным target-слоем: доменная логика → `crates/rustok-<domain>`, platform/core orchestration → `apps/server` + `crates/rustok-core`, frontend дублирование → самописные frontend-библиотеки.

---
### `verify-flex-multilingual-contract.mjs`
Focused repo-side guardrail for the live Flex multilingual contract.

Что ищет:
- cleanup migration `m20260410_000001_cleanup_flex_attached_legacy_inline_metadata` подключена в canonical server migrator;
- standalone runtime не возвращается к inline localized fallback в `flex_entries.data`;
- attached runtime не возвращается к inline localized fallback в donor `metadata`;
- `crates/flex` docs продолжают фиксировать migration-based cleanup как канонический путь.

**Severity:** HIGH. Возврат к inline localized fallback снова размажет единый multilingual storage contract.

---
### `verify-storefront-module-routes.mjs`
Repo-side контракт маршрутов storefront для модульных UI-surface.

Что проверяет:
- manifest metadata storefront UI модулей синхронизирована с route wiring;
- route keys не выпадают из ожидаемого contract-surface;
- drift между module-owned route map и host wiring фиксируется как ошибка.

**Severity:** HIGH. Drift в storefront route contract ломает навигацию и интеграцию модульного UI.

---
### `verify-i18n-contract.mjs`
Repo-side guardrail для i18n contract платформы.

Что проверяет:
- ключевые i18n contract-правила остаются согласованными в исходниках/документации;
- нет регресса в canonical путях locale handling для server-owned contract.

**Severity:** HIGH. Drift i18n contract быстро приводит к несогласованным locale fallback и UI/Server mismatch.

---
### `verify-ui-i18n-parity.mjs`
Проверка паритета i18n между module-owned UI и host-runtime expectations.

Что проверяет:
- module UI wiring не расходится с host-provided locale contract;
- ключевые surface точки не обходят canonical locale provider.

**Severity:** HIGH. Нарушение parity приводит к фрагментации i18n и различию поведения между surface-ами.

---
### `verify-module-lifecycle-bypass-usage.mjs`
Guardrail против использования lifecycle bypass helper в production/runtime путях.

Что проверяет:
- helper для lifecycle bypass не просачивается в production-контуры;
- forbidden usage фиксируется как contract violation.

**Severity:** HIGH. Bypass в production нарушает lifecycle governance и publish/runtime safety.

---
### `verify-all.sh`
**Master runner** — запуск всех `verify-*.sh` и ключевых `verify-*.mjs` с итоговым отчётом.
В non-verbose режиме раннер пытается показывать compact summary, а при падении печатает
явные `error/failed/violation` строки (с fallback на tail вывода), чтобы ошибки не терялись.

```
╔══════════════════════════════════════════════╗
║   Verification Report                        ║
╚══════════════════════════════════════════════╝

  PASS Tenant Isolation
  PASS Unsafe Code Patterns
  FAIL RBAC Coverage (2 error(s))
  PASS API Quality (REST + GraphQL)
  PASS Event System
  PASS Code Quality
  PASS Security
  PASS Architecture

  Total: 15 suites | 14 passed | 1 failed
```

> Примечание: количество suites в примере иллюстративно и должно соответствовать
> текущему списку `SCRIPTS` в `verify-all.sh`.

## Интерпретация результатов

| Символ | Значение | Действие |
|--------|----------|----------|
| `✓` (зелёный) | Проверка пройдена | Ничего не нужно |
| `!` (жёлтый) | Warning — manual review | Посмотреть вручную, может быть OK |
| `✗` (красный) | Error — нарушение | Обязательно исправить |

**Exit codes:**
- `0` — все проверки пройдены
- `N` — агрегированное количество ошибок (errors, не warnings)
- `255` — ошибок больше 255 (ограничение process exit code)

## Расширение скриптов

Для добавления новой проверки:

1. Найти подходящий скрипт по категории
2. Добавить секцию с header/pass/fail/warn
3. Обновить этот README

```bash
# Шаблон новой проверки
header "N. Описание проверки"
count=$(grep -rn 'PATTERN' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "Описание успеха"
else
    fail "$count нарушение(й):"
    grep -rn 'PATTERN' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -10
fi
```

## Связанные документы

- [Platform Verification Plan](../../docs/verification/PLATFORM_VERIFICATION_PLAN.md) — master-plan для периодических прогонов
- [План верификации качества и эксплуатационной готовности](../../docs/verification/platform-quality-operations-verification-plan.md) — детальный блок тестов, observability, CI/CD, security и quality checks
- [Forbidden Actions](../../docs/standards/forbidden-actions.md) — запреты с примерами
- [Patterns vs Antipatterns](../../docs/standards/patterns-vs-antipatterns.md) — ✅/❌ сравнения
- [Known Pitfalls](../../docs/ai/KNOWN_PITFALLS.md) — частые ошибки AI-агентов
