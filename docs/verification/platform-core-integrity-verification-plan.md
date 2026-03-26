# План rolling-верификации целостности ядра платформы

- **Статус:** Актуализированный rolling-чеклист
- **Режим:** Повторяемая точечная верификация
- **Частота:** После любых изменений в ядре, admin-панелях, core модулях, i18n или конфигурации module registry
- **Цель:** Убедиться, что server + обе admin-панели + core crates образуют самодостаточное ядро, которое работает полностью независимо от опциональных доменных модулей, предоставляет полноценный интерфейс и поддерживает многоязычность
- **Companion-план:** [Главный план верификации платформы](./PLATFORM_VERIFICATION_PLAN.md)

**Принцип работы с планом:**
При прогоне — **устранять найденные проблемы сразу**, в той же сессии. Найденная проблема не закрывается до момента исправления. Нерешённые блокеры фиксируются в артефакте прогона как open blocker. После исправления — перепроверить соответствующую фазу. Реестр проблем ведётся в [platform-verification-issues-registry.md](./platform-verification-issues-registry.md).

**Легенда режимов:**
- `🔧 Static` — выполнимо без запущенной инфраструктуры (cargo build/check/test, npm build/lint, git grep)
- `🌐 Runtime` — требует запущенных PostgreSQL + server (и опционально Iggy)

---

## 0. Предварительные условия

> Если среда не поддерживает запуск инфраструктуры — `🌐 Runtime`-проверки пропускаются, выполняются только `🔧 Static`.

- `🔧` `docker compose config` проходит без ошибок.
- `🔧` `.env` или `.env.dev` содержит корректные переменные для соединения с DB и Iggy.
- `🌐` PostgreSQL запущен и доступен (порт 5432) — `docker compose up -d db`
- `🌐` Iggy event broker запущен (TCP порт 8090) — `docker compose up -d iggy` — **если сконфигурирован Iggy-транспорт** (`rustok-iggy` / `rustok-iggy-connector`). `rustok-outbox` transport-agnostic: при использовании другого транспорта или DB-режима Iggy не обязателен.

---

## 1. Состав ядра платформы

Этот план верифицирует только следующие компоненты как единое целое:

### 1.1 Приложения

- **Server:** `apps/server`
- **Admin панель #1:** `apps/admin` (Leptos CSR)
- **Admin панель #2:** `apps/next-admin` (Next.js 16 + React 19)

### 1.2 Core crates

- `rustok-core` — инфраструктурные контракты, типы ошибок, cache abstractions
- `rustok-auth` — жизненный цикл аутентификации, JWT, OAuth2 AS
- `rustok-rbac` — ролевая модель доступа, typed permissions
- `rustok-cache` — абстракция кэша (in-memory / Redis)
- `rustok-tenant` — multi-tenancy: резолюция, изоляция, cache
- `rustok-events` — domain event definitions и contracts
- `rustok-outbox` — transactional outbox, transport-agnostic event relay; конкретный транспорт (Iggy и др.) подключается опционально через `rustok-iggy` / `rustok-iggy-connector`
- `rustok-search` — поисковый движок, PgSearch / индекс
- `rustok-index` — CQRS read models, денормализация
- `rustok-telemetry` — OpenTelemetry, tracing, Prometheus
- `rustok-api` — shared host/API layer: TenantContext, AuthContext
- `rustok-email` — email service abstraction

### 1.3 Граница

Следующие компоненты **не входят** в область этого плана:

- Опциональные доменные модули: `rustok-content`, `rustok-commerce`, `rustok-blog`, `rustok-forum`, `rustok-pages`, `rustok-media`, `rustok-workflow`
- Capability-слои: `flex`, `alloy`, `alloy-scripting`, `rustok-mcp`
- Их UI, тесты и интеграции верифицируются в профильных планах

---

## 2. Инварианты ядра

- `🔧` Core crates не импортируют опциональные доменные crates (content, commerce, blog, forum, pages, media, workflow).
- `🔧` `rustok-core` не содержит доменных таблиц — только инфраструктурные контракты.
- `🔧` Модули с `ModuleKind::Core` помечены `required = true` в `modules.toml`.
- `🔧` `registry.is_core()` запрещает отключение core модулей через tenant API.
- `🔧` `rustok-outbox` является `Core` модулем без tenant-toggle semantics.
- `🔧` В `build_registry()` отсутствуют циклические зависимости между core crates.

---

## 3. Boot без опциональных модулей

**Файлы:**
- `apps/server/src/app.rs`
- `apps/server/src/modules/mod.rs`
- `apps/server/src/modules/manifest.rs`
- `modules.toml`

- `🔧` `cargo build -p rustok-server` проходит.
- `🌐` Server стартует с включёнными только core модулями.
- `🌐` `validate_registry_vs_manifest()` вызывается при старте и проходит без ошибок.
- `🌐` Миграции (`cargo loco db migrate`) проходят без доменных модульных миграций.
- `🌐` Server завершает bootstrap без `unwrap()` паник, связанных с отсутствием domain модулей.
- `🌐` `/api/health` возвращает HTTP 200.

---

## 4. Auth в изоляции

**Файлы:**
- `crates/rustok-auth/`
- `apps/server/src/controllers/auth.rs`
- `apps/server/src/controllers/oauth.rs`
- `apps/server/src/services/auth_lifecycle.rs`

> Полный auth/RBAC-аудит (JWT-контракт, permission enforcement, hardcoded roles) — в [platform-foundation-verification-plan.md](./platform-foundation-verification-plan.md).

- `🌐` Sign up работает без опционального модуля.
- `🌐` Sign in (email + password) работает без опционального модуля.
- `🌐` Token refresh работает.
- `🌐` Logout и session invalidation работают.
- `🌐` Password reset flow работает.
- `🌐` OAuth2 Authorization Server (PKCE flow, client credentials) работает как часть ядра.

---

## 5. Multi-tenancy core

**Файлы:**
- `crates/rustok-tenant/`
- `apps/server/src/middleware/tenant.rs`

> Полный tenancy-аудит (cache, stampede, Redis invalidation) — в [platform-foundation-verification-plan.md](./platform-foundation-verification-plan.md).

- `🌐` Tenant resolution (hostname/header-based) работает при чистом старте.
- `🌐` Core модули всегда включены — попытка disable через API возвращает ошибку.
- `🔧` `tenant_modules` корректно отражает core модули как не-toggleable.

---

## 6. Обе admin-панели — функциональная полнота

Admin-панели предоставляют **полноценный интерфейс** управления ядром платформы, а не голый дашборд. Каждый пункт меню — это UI, предоставляемый конкретным core модулем.

### 6.1 Функциональные разделы

| Пункт меню | Core модуль — источник UI |
|------------|---------------------------|
| Пользователи (Users) | `rustok-auth` |
| Сессии (Sessions) | `rustok-auth` |
| Роли и разрешения (Roles & Permissions) | `rustok-rbac` |
| Tenant-ы / Организации | `rustok-tenant` |
| Управление модулями | server / module registry |
| Email-настройки | `rustok-email` |
| Кэш (Cache management) | `rustok-cache` |
| OAuth приложения | `rustok-auth` (OAuth2 AS) |
| Настройки платформы (Settings) | `rustok-core` |
| Локализация / Многоязычность | i18n layer (см. фазу 7) |

### 6.2 Leptos Admin (`apps/admin`)

- `🔧` `cargo build -p rustok-admin` проходит.
- `🌐` Приложение запускается и устанавливает соединение с server.
- `🌐` Аутентификация работает через GraphQL auth flow.
- `🌐` Dashboard загружается после успешного входа.
- `🌐` Все функциональные разделы из таблицы 6.1 присутствуют в навигации.
- `🌐` Каждый раздел, чей backend-модуль включён, отображает рабочий интерфейс.
- `🌐` Разделы без включённого backend-модуля деградируют корректно (нет краша, нет 500).
- `🔧` Module-owned routing (`/modules/:module_slug/*`) зарегистрирован для core модулей.

### 6.3 Next.js Admin (`apps/next-admin`)

- `🔧` `npm run build` проходит.
- `🔧` `npm run lint` проходит.
- `🔧` `npm run typecheck` проходит.
- `🌐` Приложение запускается и устанавливает соединение с server.
- `🌐` Аутентификация работает через NextAuth credentials flow.
- `🌐` Dashboard загружается после успешного входа.
- `🌐` Все функциональные разделы из таблицы 6.1 присутствуют в навигации.
- `🌐` Каждый раздел, чей backend-модуль включён, отображает рабочий интерфейс.
- `🌐` Разделы без включённого backend-модуля деградируют корректно (нет краша, нет 500).

---

## 7. Многоязычность (i18n) как часть ядра

Поддержка многоязычности — платформенная функция, а не доменный модуль.

### 7.1 Server / API

- `🌐` API возвращает локализованные сообщения об ошибках при запросе с заголовком `Accept-Language`.
- `🌐` Auth messages (ошибки валидации, email-тексты) локализованы.
- `🌐` GraphQL API поддерживает передачу locale через параметр или заголовок.

### 7.2 Leptos Admin

- `🌐` Раздел управления языками / переводами присутствует в навигации.
- `🌐` UI корректно переключается между языками (минимум: RU, EN).
- `🌐` Форматирование дат и чисел соответствует активному locale.

### 7.3 Next.js Admin

- `🔧` `next-intl` настроен и подключён (`apps/next-admin/`).
- `🌐` Роутинг с locale-префиксом работает корректно.
- `🌐` Раздел управления языками / переводами присутствует в навигации.
- `🌐` UI корректно переключается между языками (минимум: RU, EN).

---

## 8. UI core модулей (наличие и сборка)

> ⚠️ **В разработке:** UI-компоненты core модулей находятся в активной разработке и могут частично отсутствовать. Эта пометка будет снята по готовности UI.

> **Область действия:** Этот план верифицирует **только UI core модулей** (rustok-auth, rustok-rbac, rustok-tenant, rustok-email, rustok-cache, rustok-core). UI доменных опциональных модулей и capability-слоёв (flex, rustok-mcp, alloy) верифицируются в профильных планах.

### 8.1 Leptos UI компоненты core модулей

- `🔧` `rustok-auth` — наличие admin-UI Leptos (users, sessions, OAuth apps).
- `🔧` `rustok-rbac` — наличие admin-UI Leptos (roles, permissions).
- `🔧` `rustok-tenant` — наличие admin-UI Leptos (tenant management).
- `🔧` `rustok-email` — наличие admin-UI Leptos (email settings).
- `🔧` `rustok-cache` — наличие admin-UI Leptos (если предусмотрен).
- `🔧` Сборка всех найденных Leptos UI пакетов core модулей проходит (`cargo build`).

### 8.2 Next.js UI пакеты core модулей

- `🔧` Наличие Next.js пакетов для управления пользователями/ролями/tenant-ами в `apps/next-admin/packages/`.
- `🔧` Сборка пакетов проходит (`npm run build`).
- `🔧` Lint проходит (`npm run lint`).

### 8.3 Интеграция UI в admin-панели

- `🔧` Leptos Admin регистрирует UI core модулей через module-owned routing.
- `🔧` Next.js Admin импортирует пакеты core модулей корректно и без циклических зависимостей.
- `🔧` Отсутствующие (в разработке) UI не блокируют сборку и запуск admin-панелей.

---

## 9. GraphQL schema без опциональных модулей

**Файлы:**
- `apps/server/src/graphql/schema.rs`
- `apps/server/src/graphql/queries.rs`
- `apps/server/src/graphql/mutations.rs`

> Полный GraphQL-аудит — в [platform-api-surfaces-verification-plan.md](./platform-api-surfaces-verification-plan.md).

- `🔧` GraphQL schema компилируется без паники при отсутствии domain resolver-ов.
- `🌐` Queries для auth, users, tenant-ов, settings резолвятся.
- `🌐` Mutations для управления пользователями, ролями, tenant-ами работают.

---

## 10. Команды

### 10.1 Сборка

```sh
# Server
# Примечание: дефолтные features включают embed-admin (RustEmbed из apps/admin/dist).
# Если apps/admin/dist не собран, используй вариант без embed:
cargo build -p rustok-server --no-default-features \
  --features "redis-cache,mod-product,mod-pricing,mod-inventory,mod-cart,\
mod-customer,mod-order,mod-payment,mod-fulfillment,mod-commerce,mod-content,\
mod-blog,mod-forum,mod-pages,mod-alloy,mod-media,mod-workflow"
# Либо предварительно собери Leptos admin: cd apps/admin && trunk build

# Leptos admin
cargo build -p rustok-admin

# Core crates workspace check
cargo check --workspace

# Next.js Admin
cd apps/next-admin && npm run build
cd apps/next-admin && npm run lint
# Примечание: скрипт typecheck в apps/next-admin не определён; используй tsc напрямую:
# cd apps/next-admin && npx tsc --noEmit
```

### 10.2 Тесты core

```sh
cargo test -p rustok-core --lib
cargo test -p rustok-auth --lib
cargo test -p rustok-rbac --lib
cargo test -p rustok-tenant --lib
cargo test -p rustok-outbox --lib
cargo test -p rustok-server --lib
```

### 10.3 Изоляция: поиск нежелательных зависимостей

```sh
git grep -rn "rustok-content\|rustok-commerce\|rustok-blog\|rustok-forum\|rustok-pages\|rustok-media\|rustok-workflow" \
  -- crates/rustok-core/ crates/rustok-auth/ crates/rustok-rbac/ \
     crates/rustok-tenant/ crates/rustok-events/ crates/rustok-outbox/ \
     crates/rustok-index/ crates/rustok-cache/ crates/rustok-email/
```

### 10.4 Health check

```sh
curl -f http://localhost:5150/api/health
```

### 10.5 Docker

```sh
docker compose config
docker compose up -d db
```

---

## 11. Stop-the-line условия

При обнаружении любого из нижеперечисленных случаев — **остановить прогон, исправить, перепроверить фазу, только затем продолжать**. Не оставлять в артефакте как «known issue».

Считать блокирующим drift любой из следующих случаев:

- `cargo build -p rustok-server` или `cargo build -p rustok-admin` не компилируются.
- Core crate импортирует опциональный доменный crate (content, commerce, blog, forum, pages, media, workflow).
- Server не стартует при включённых только core модулях.
- `/api/health` возвращает не 200 при чистом старте.
- GraphQL schema паникует при сборке без domain resolver-ов.
- Core модуль успешно отключается через tenant API (ожидается ошибка).
- Любая admin-панель крашится при попытке открыть auth/dashboard с только core.
- В любой admin-панели отсутствует навигация по core функциям (auth, rbac, tenants, modules).

---

## 12. Артефакты

Каждый прогон должен оставлять короткий evidence bundle:

- дата
- branch / commit
- выполненные команды
- pass/fail по каждой фазе
- список UI-компонентов core модулей, которые отсутствуют (в разработке)
- список выявленных проблем
- оставшиеся блокеры

**Место хранения:** `artifacts/verification/platform-core-integrity/<yyyy-mm-dd>.md`

---

## Связанные документы

- [Главный план верификации платформы](./PLATFORM_VERIFICATION_PLAN.md)
- [План foundation-верификации](./platform-foundation-verification-plan.md) — полный auth/RBAC/tenancy/registry аудит
- [План верификации API-поверхностей](./platform-api-surfaces-verification-plan.md) — полный GraphQL/REST аудит
- [План rolling-верификации RBAC для server и runtime-модулей](./rbac-server-modules-verification-plan.md)
- [Реестр проблем платформенной верификации](./platform-verification-issues-registry.md)
- [README каталога verification](./README.md)
