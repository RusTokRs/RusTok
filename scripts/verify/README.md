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
| Добавили миграцию | `./scripts/verify/verify-all.sh tenant-isolation` |
| Подозрение на дыру в RBAC | `./scripts/verify/verify-all.sh rbac-coverage` |
| Аудит безопасности | `./scripts/verify/verify-security.sh` |
| Проверка deployment profile matrix | `./scripts/verify/verify-all.sh deployment-profiles` |

## Описание скриптов

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
????????????? ??? `registry-only` ??????? ??? ?????? `GET /v1/catalog/{slug}` detail-path, cache-contract ????? `ETag` / `If-None-Match` ? negative smoke ?? write-route-? `POST /v2/catalog/publish`, `POST /v2/catalog/owner-transfer` ? `POST /v2/catalog/yank`.

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
### `verify-all.sh`
**Master runner** — запуск всех скриптов с итоговым отчётом.

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

  Total: 8 suites | 7 passed | 1 failed
```

## Интерпретация результатов

| Символ | Значение | Действие |
|--------|----------|----------|
| `✓` (зелёный) | Проверка пройдена | Ничего не нужно |
| `!` (жёлтый) | Warning — manual review | Посмотреть вручную, может быть OK |
| `✗` (красный) | Error — нарушение | Обязательно исправить |

**Exit codes:**
- `0` — все проверки пройдены
- `N` — количество ошибок (errors, не warnings)

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

