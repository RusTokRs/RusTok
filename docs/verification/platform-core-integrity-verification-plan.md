# План rolling-верификации целостности ядра платформы

- **Статус:** Актуализированный rolling-чеклист
- **Режим:** Повторяемая точечная верификация
- **Частота:** После любых изменений в ядре, admin-панелях, core модулях, i18n или конфигурации module registry
- **Цель:** Убедиться, что server + обе admin-панели + core crates образуют самодостаточное ядро, которое работает полностью независимо от опциональных доменных модулей, предоставляет полноценный интерфейс и поддерживает многоязычность
- **Companion-план:** [Главный план верификации платформы](./PLATFORM_VERIFICATION_PLAN.md)

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
- `rustok-outbox` — transactional outbox, event relay
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

- [ ] Core crates не импортируют опциональные доменные crates (content, commerce, blog, forum, pages, media, workflow).
- [ ] `rustok-core` не содержит доменных таблиц — только инфраструктурные контракты.
- [ ] Модули с `ModuleKind::Core` помечены `required = true` в `modules.toml`.
- [ ] `registry.is_core()` запрещает отключение core модулей через tenant API.
- [ ] `rustok-outbox` является `Core` модулем без tenant-toggle semantics.
- [ ] В `build_registry()` отсутствуют циклические зависимости между core crates.

---

## 3. Boot без опциональных модулей

**Файлы:**
- `apps/server/src/app.rs`
- `apps/server/src/modules/mod.rs`
- `apps/server/src/modules/manifest.rs`
- `modules.toml`

- [ ] `cargo build -p rustok-server` проходит.
- [ ] Server стартует с включёнными только core модулями.
- [ ] `validate_registry_vs_manifest()` вызывается при старте и проходит без ошибок.
- [ ] `/api/health` возвращает HTTP 200.
- [ ] `/api/graphql` отвечает на introspection запрос без паники.
- [ ] Миграции (`cargo loco db migrate`) проходят без доменных модульных миграций.
- [ ] Server завершает bootstrap без `unwrap()` паник, связанных с отсутствием domain модулей.

---

## 4. Auth & RBAC в изоляции

**Файлы:**
- `crates/rustok-auth/`
- `crates/rustok-rbac/`
- `apps/server/src/controllers/auth.rs`
- `apps/server/src/controllers/oauth.rs`
- `apps/server/src/graphql/auth/`
- `apps/server/src/services/auth_lifecycle.rs`
- `apps/server/src/extractors/rbac.rs`

- [ ] Sign up работает без опционального модуля.
- [ ] Sign in (email + password) работает без опционального модуля.
- [ ] Token refresh работает.
- [ ] Logout и session invalidation работают.
- [ ] Password reset flow работает.
- [ ] OAuth2 Authorization Server (PKCE flow, client credentials) работает как часть ядра.
- [ ] `SecurityContext` строится из resolved permissions, а не из role inference.
- [ ] `infer_user_role_from_permissions()` не используется как замена авторизации.
- [ ] Нет hardcoded `UserRole::Admin` / `UserRole::SuperAdmin` в authorization path.
- [ ] REST и GraphQL auth surfaces проходят через единый `AuthLifecycleService`.

---

## 5. Multi-tenancy core

**Файлы:**
- `crates/rustok-tenant/`
- `apps/server/src/middleware/tenant.rs`
- `rustok-api` — `TenantContext`

- [ ] Tenant resolution (hostname/header-based) работает при чистом старте.
- [ ] `TenantContext` импортируется из `rustok-api` консистентно во всём server.
- [ ] Core модули всегда включены — попытка disable через API возвращает ошибку.
- [ ] Tenant cache (positive/negative) и stampede protection работают.
- [ ] Redis invalidation для tenant cache работает при изменении данных tenant.
- [ ] `tenant_modules` корректно отражает core модули как не-toggleable.

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

- [ ] `cargo build -p rustok-admin` проходит.
- [ ] Приложение запускается и устанавливает соединение с server.
- [ ] Аутентификация работает через GraphQL auth flow.
- [ ] Dashboard загружается после успешного входа.
- [ ] Все функциональные разделы из таблицы 6.1 присутствуют в навигации.
- [ ] Каждый раздел, чей backend-модуль включён, отображает рабочий интерфейс.
- [ ] Разделы без включённого backend-модуля деградируют корректно (нет краша, нет 500).
- [ ] Module-owned routing (`/modules/:module_slug/*`) работает для core модулей.

### 6.3 Next.js Admin (`apps/next-admin`)

- [ ] `npm run build` проходит.
- [ ] `npm run lint` проходит.
- [ ] `npm run typecheck` проходит.
- [ ] Приложение запускается и устанавливает соединение с server.
- [ ] Аутентификация работает через NextAuth credentials flow.
- [ ] Dashboard загружается после успешного входа.
- [ ] Все функциональные разделы из таблицы 6.1 присутствуют в навигации.
- [ ] Каждый раздел, чей backend-модуль включён, отображает рабочий интерфейс.
- [ ] Разделы без включённого backend-модуля деградируют корректно (нет краша, нет 500).

---

## 7. Многоязычность (i18n) как часть ядра

Поддержка многоязычности — платформенная функция, а не доменный модуль.

### 7.1 Server / API

- [ ] API возвращает локализованные сообщения об ошибках при запросе с заголовком `Accept-Language`.
- [ ] Auth messages (ошибки валидации, email-тексты) локализованы.
- [ ] GraphQL API поддерживает передачу locale через параметр или заголовок.

### 7.2 Leptos Admin

- [ ] Раздел управления языками / переводами присутствует в навигации.
- [ ] UI корректно переключается между языками (минимум: RU, EN).
- [ ] Форматирование дат и чисел соответствует активному locale.

### 7.3 Next.js Admin

- [ ] `next-intl` настроен и подключён (`apps/next-admin/`).
- [ ] Роутинг с locale-префиксом работает корректно.
- [ ] Раздел управления языками / переводами присутствует в навигации.
- [ ] UI корректно переключается между языками (минимум: RU, EN).

---

## 8. UI core модулей (наличие и сборка)

> ⚠️ **В разработке:** UI-компоненты core модулей находятся в активной разработке и могут частично отсутствовать. Эта пометка будет снята по готовности UI.

> **Область действия:** Этот план верифицирует **только UI core модулей** (rustok-auth, rustok-rbac, rustok-tenant, rustok-email, rustok-cache, rustok-core). UI доменных опциональных модулей (content, commerce, blog, forum, pages, media, workflow) и capability-слоёв (flex, rustok-mcp, alloy) верифицируются в профильных планах.

### 8.1 Leptos UI компоненты core модулей

- [ ] `rustok-auth` — наличие admin-UI Leptos (users, sessions, OAuth apps).
- [ ] `rustok-rbac` — наличие admin-UI Leptos (roles, permissions).
- [ ] `rustok-tenant` — наличие admin-UI Leptos (tenant management).
- [ ] `rustok-email` — наличие admin-UI Leptos (email settings).
- [ ] `rustok-cache` — наличие admin-UI Leptos (если предусмотрен).
- [ ] Сборка всех найденных Leptos UI пакетов core модулей проходит (`cargo build`).

### 8.2 Next.js UI пакеты core модулей

- [ ] Наличие Next.js пакетов для управления пользователями/ролями/tenant-ами в `apps/next-admin/packages/`.
- [ ] Сборка пакетов проходит (`npm run build`).
- [ ] Lint проходит (`npm run lint`).

### 8.3 Интеграция UI в admin-панели

- [ ] Leptos Admin регистрирует UI core модулей через module-owned routing.
- [ ] Next.js Admin импортирует пакеты core модулей корректно и без циклических зависимостей.
- [ ] Отсутствующие (в разработке) UI не блокируют сборку и запуск admin-панелей.

---

## 9. GraphQL schema без опциональных модулей

**Файлы:**
- `apps/server/src/graphql/schema.rs`
- `apps/server/src/graphql/queries.rs`
- `apps/server/src/graphql/mutations.rs`

- [ ] GraphQL schema компилируется без паники при отсутствии domain resolver-ов.
- [ ] Queries для auth, users, tenant-ов, settings резолвятся.
- [ ] Mutations для управления пользователями, ролями, tenant-ами работают.
- [ ] Нет ошибок "field not found" для core-level операций.
- [ ] Schema introspection возвращает корректный ответ.

---

## 10. Module lifecycle & registry integrity

**Файлы:**
- `apps/server/src/modules/mod.rs`
- `apps/server/src/modules/manifest.rs`
- `modules.toml`

- [ ] `validate_registry_vs_manifest()` проходит при старте сервера.
- [ ] Набор core модулей в `build_registry()` совпадает с `modules.toml`.
- [ ] `required = true` у core модулей совпадает с `ModuleKind::Core` в impl.
- [ ] `depends_on` в manifest совпадает с `dependencies()` в `RusToKModule` impl.
- [ ] Попытка disable core модуля через tenant API возвращает ошибку (`registry.is_core()`).
- [ ] Build/deployment manifest flow не расходится с runtime registry.

---

## 11. Команды

### 11.1 Сборка

```sh
# Server
cargo build -p rustok-server

# Leptos admin
cargo build -p rustok-admin

# Core crates workspace check
cargo check --workspace

# Next.js Admin
cd apps/next-admin && npm run build
cd apps/next-admin && npm run lint
cd apps/next-admin && npm run typecheck
```

### 11.2 Тесты core

```sh
cargo test -p rustok-core --lib
cargo test -p rustok-auth --lib
cargo test -p rustok-rbac --lib
cargo test -p rustok-tenant --lib
cargo test -p rustok-outbox --lib
cargo test -p rustok-server --lib
```

### 11.3 Изоляция: поиск нежелательных зависимостей

```sh
# Core crates не должны тянуть доменные модули
git grep -rn "rustok-content\|rustok-commerce\|rustok-blog\|rustok-forum\|rustok-pages\|rustok-media\|rustok-workflow" \
  -- crates/rustok-core/ crates/rustok-auth/ crates/rustok-rbac/ \
     crates/rustok-tenant/ crates/rustok-events/ crates/rustok-outbox/ \
     crates/rustok-index/ crates/rustok-cache/ crates/rustok-email/

# Поиск запрещённых role-based shortcuts в server authorization path
git grep -n "UserRole::Admin\|UserRole::SuperAdmin" -- apps/server/src/
git grep -n "infer_user_role_from_permissions" -- apps/server/src/
```

### 11.4 Health check

```sh
curl -f http://localhost:5150/api/health
```

### 11.5 Docker

```sh
docker compose config
docker compose up -d
```

---

## 12. Stop-the-line условия

Считать блокирующим drift любой из следующих случаев:

- Server не стартует при включённых только core модулях.
- `validate_registry_vs_manifest()` выбрасывает ошибку на старте.
- Любая admin-панель крашится при попытке открыть auth/dashboard с только core.
- Core crate импортирует опциональный доменный crate (content, commerce, blog, forum, pages, media, workflow).
- Core модуль успешно отключается через tenant API (ожидается ошибка).
- `/api/health` возвращает не 200 при чистом старте.
- GraphQL schema паникует при сборке без domain resolver-ов.
- В любой admin-панели отсутствует навигация по core функциям (auth, rbac, tenants, modules).
- `next-intl` не инициализируется в Next.js Admin.
- `cargo build -p rustok-server` или `cargo build -p rustok-admin` не компилируются.

---

## 13. Артефакты

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
- [План foundation-верификации](./platform-foundation-verification-plan.md)
- [План rolling-верификации RBAC для server и runtime-модулей](./rbac-server-modules-verification-plan.md)
- [Реестр проблем платформенной верификации](./platform-verification-issues-registry.md)
- [README каталога verification](./README.md)
