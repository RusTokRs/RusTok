---
id: doc://docs/standards/patterns-vs-antipatterns.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# RusToK — Паттерны vs Антипаттерны

Сводный справочник правильных и неправильных подходов при разработке на платформе RusToK.

Каждый раздел содержит: что делать правильно (✅), что запрещено (❌), почему, и ссылку на детальный документ.

> **Статус:** Living document. Обновлять при добавлении новых модулей, паттернов, и обнаружении новых антипаттернов.

---

## Оглавление

1. [Архитектура](#1-архитектура)
2. [Качество кода (Rust)](#2-качество-кода-rust)
3. [Данные и БД](#3-данные-и-бд)
4. [Событийная система](#4-событийная-система)
5. [Auth и RBAC](#5-auth-и-rbac)
6. [Multi-Tenancy](#6-multi-tenancy)
7. [API (GraphQL + REST)](#7-api)
8. [Фронтенд](#8-фронтенд)
9. [Тестирование](#9-тестирование)
10. [Observability](#10-observability)
11. [Безопасность](#11-безопасность)
12. [DevOps и CI/CD](#12-devops-и-cicd)

---

## 1. Архитектура

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 1.1 | Модуль реализует `RusToKModule` trait и регистрируется в `build_registry()` | Модуль подключается напрямую в `app.rs` минуя registry | Обходит lifecycle, health checks, per-tenant toggle | [architecture/modules.md](../architecture/modules.md) |
| 1.2 | Core-модули возвращают `ModuleKind::Core`, optional — `ModuleKind::Optional` | Все модули имеют одинаковый `kind()` | Core-модули нельзя отключить, нужна формальная граница | [architecture/principles.md](../architecture/principles.md) |
| 1.3 | `dependencies()` в `RusToKModule` совпадают с `depends_on` в `modules.toml` | Зависимости заданы только в Cargo.toml или только в modules.toml | Runtime-проверка не ловит рассинхрон, модуль включится без dependency | [modules/manifest.md](../modules/manifest.md) |
| 1.4 | Бизнес-логика — в domain crates (`crates/rustok-*`), controllers — тонкие | Бизнес-логика в controllers/resolvers | Дублирование между REST и GraphQL, нетестируемость | [architecture/overview.md](../architecture/overview.md) |
| 1.5 | Модули взаимодействуют через EventBus, не вызывают друг друга напрямую | Прямые вызовы между доменными модулями | Coupling, нарушение event-driven принципа | [architecture/overview.md](../architecture/overview.md) |
| 1.6 | Write path — нормализованные таблицы, Read path — denormalized index | Один набор таблиц для write и read | Нарушает CQRS-lite, медленный storefront | [architecture/overview.md §CQRS-lite](../architecture/overview.md) |
| 1.7 | Loco hooks (`Hooks::routes`, `after_routes`, `connect_workers`) для lifecycle | Собственный lifecycle «чистого Axum» | Обходит Loco initialization, dependency injection, middleware chain | [ai/KNOWN_PITFALLS.md §Loco](../ai/KNOWN_PITFALLS.md) |
| 1.8 | Общие зависимости — через `AppContext.shared_store` | Глобальные singleton-объекты (static, lazy_static) | Нетестируемо, нет per-request scope, утечки между тестами | [ai/KNOWN_PITFALLS.md §Loco](../ai/KNOWN_PITFALLS.md) |

---

## 2. Качество кода (Rust)

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 2.1 | `Result<T, Error>` для обработки ошибок | `unwrap()` / `expect()` в production коде | Паника крашит весь сервер | [standards/coding.md §2](coding.md) |
| 2.2 | `thiserror` + иерархия typed errors | `anyhow` в library crates, `String` errors | Теряется типизация, невозможно match по ошибке | [standards/errors.md](errors.md) |
| 2.3 | Newtype pattern (`TenantId(Uuid)`, `UserId(Uuid)`) | Голые `Uuid` / `String` для ID | Можно перепутать user_id и tenant_id на уровне типов | [standards/coding.md §1.2](coding.md) |
| 2.4 | State machine через enum + transition methods | String-based status с if/else проверками | Нет compile-time гарантий валидных переходов | [guides/state-machine.md](../guides/state-machine.md) |
| 2.5 | `tokio::select!` с cancellation safety | `tokio::spawn` без join + без cleanup | Утечки тасков, незакрытые ресурсы | [standards/coding.md §3.1](coding.md) |
| 2.6 | `Semaphore` для ограничения concurrency | Неограниченный `tokio::spawn` в цикле | Тысячи тасков, OOM, resource exhaustion | [standards/coding.md §3.2](coding.md) |
| 2.7 | `Cow<'_, str>` для avoid unnecessary clones | `.to_string()` / `.clone()` повсюду | Ненужные аллокации, latency | [standards/coding.md §4](coding.md) |
| 2.8 | Функция < 20 строк, модуль < 500 строк | Функции > 40 строк, модули > 1000 строк | Сложность, нечитаемость, трудность тестирования | [standards/coding.md §9.1](coding.md) |
| 2.9 | < 4 аргумента функции (или struct для params) | > 6 аргументов функции | Трудно читать, легко перепутать аргументы | [standards/coding.md §9.1](coding.md) |
| 2.10 | `#[instrument]` на service methods | Нет трейсинга в сервисных методах | Невозможно отследить request flow | [standards/logging.md](logging.md) |
| 2.11 | Зависимость от trait objects (`Arc<dyn Repository>`) | Зависимость от конкретных типов (`PgOrderRepository`) | Невозможно подменить для тестирования | [standards/coding.md §1.1 (DI)](coding.md) |
| 2.12 | `const` для compile-time known values | `fn get_constant() -> T` для значений, известных на этапе компиляции | Runtime overhead без причины | [standards/coding.md §1.3](coding.md) |

---

## 3. Данные и БД

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 3.1 | **Всегда** `WHERE tenant_id = ?` в каждом запросе | SELECT/UPDATE/DELETE без tenant_id filter | **Критическая уязвимость**: cross-tenant data leak | [architecture/tenancy.md](../architecture/tenancy.md) |
| 3.2 | Параметризованные запросы через SeaORM | String concatenation для SQL | SQL injection | [standards/security.md](security.md) |
| 3.3 | Миграции через `RusToKModule::migrations()` | Ручные SQL скрипты мимо migration system | Рассинхрон схемы между окружениями | [architecture/principles.md](../architecture/principles.md) |
| 3.4 | Naming: `mYYYYMMDD_<module>_<nnn>_<description>` | Произвольные имена миграций | Нарушает порядок выполнения, конфликты | [architecture/principles.md](../architecture/principles.md) |
| 3.5 | Отдельные DTO (Input/Response) vs Entity | Entity из БД отдаётся напрямую в API | Утечка internal полей, coupling между API и schema | [architecture/api.md](../architecture/api.md) |
| 3.6 | Транзакция для write + event (`publish_in_tx`) | Отдельный write и отдельный publish | Событие уходит, а write откатился (или наоборот) | [ai/KNOWN_PITFALLS.md §Outbox](../ai/KNOWN_PITFALLS.md) |
| 3.7 | SeaORM entities с `#[derive(DeriveEntityModel)]` | Ручные SQL-строки для CRUD | Нет type safety, ручной маппинг, ошибки | — |
| 3.8 | Soft delete (status = Archived) для бизнес-сущностей | Hard DELETE для products/orders/nodes | Потеря аудитной истории, broken references | — |
| 3.9 | Index tables (`index_products`, `index_content`) для read path | Join 5+ таблиц для storefront queries | Медленные read-запросы, нагрузка на write DB | [architecture/overview.md §CQRS](../architecture/overview.md) |

---

## 4. Событийная система

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 4.1 | `publish_in_tx()` — событие в той же транзакции, что и write | `publish()` (fire-and-forget) для бизнес-событий | Событие может уйти при откате транзакции | [ai/KNOWN_PITFALLS.md §Outbox](../ai/KNOWN_PITFALLS.md) |
| 4.2 | `transport = "outbox"` для production | `transport = "memory"` в production | Потеря событий при рестарте, нет гарантий | [references/outbox/README.md](../references/outbox/README.md) |
| 4.3 | Outbox relay worker запущен | Production без relay worker | События навсегда застрянут в `sys_events` | [ai/KNOWN_PITFALLS.md §Outbox](../ai/KNOWN_PITFALLS.md) |
| 4.4 | `DomainEvent` с `tenant_id` в payload | События без tenant_id | Index не сможет определить, к какому tenant относится событие | — |
| 4.5 | Idempotent event handlers | Event handler без idempotency check | Дублирование данных при retry/replay | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 4.6 | Event versioning с backward compatibility | Breaking changes в event payload | Старые consumers ломаются | — |
| 4.7 | Использовать `IggyConfig`/`ConnectorConfig` из кода | Выдумывать конфигурацию Iggy | Несовместимые параметры, ошибки соединения | [ai/KNOWN_PITFALLS.md §Iggy](../ai/KNOWN_PITFALLS.md) |
| 4.8 | DLQ для failed events + admin replay endpoint | Silent drop failed events | Потеря данных без возможности восстановления | [architecture/events.md](../architecture/events.md) |

---

## 5. Auth и RBAC

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 5.1 | Permission extractors (`RequireProductsCreate(user)`) | Без RBAC-проверки в handler | Любой auth-пользователь может делать что угодно | [architecture/rbac.md](../architecture/rbac.md) |
| 5.2 | `AuthLifecycleService` для auth бизнес-логики | Дублирование auth логики в REST и GraphQL контроллерах | Рассинхрон поведения между transport-слоями | [architecture/api.md](../architecture/api.md) |
| 5.3 | `SecurityContext` с `get_scope()` в сервисах | Фильтрация данных только на уровне controller | Customer видит чужие заказы в list-запросах | [architecture/rbac.md §SecurityContext](../architecture/rbac.md) |
| 5.4 | JWT secret через env var (`JWT_SECRET`) | Hardcoded JWT secret в коде | Компрометация всех токенов | [standards/security.md](security.md) |
| 5.5 | Argon2 для password hashing | MD5/SHA256/bcrypt для паролей | Argon2 — стандарт, resistant к GPU/ASIC | — |
| 5.6 | Token invalidation при change-password | Старые токены остаются валидными после смены пароля | Скомпрометированный токен продолжает работать | — |
| 5.7 | Public endpoints явно помечены (health, login, storefront queries) | Endpoint без auth «по умолчанию» | Случайное раскрытие данных | [architecture/rbac.md](../architecture/rbac.md) |

---

## 6. Multi-Tenancy

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 6.1 | `TenantContext` extractor в каждом handler | Handler без tenant resolution | Данные всех tenants смешиваются | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.2 | `tenant_id` поле во **всех** domain-таблицах | Таблицы без tenant_id | Невозможно изолировать данные | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.3 | Negative cache для несуществующих tenants (TTL 60s) | Каждый запрос с невалидным tenant идёт в БД | DoS через несуществующие tenants | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.4 | Singleflight для cache miss (один запрос к БД) | Каждый concurrent request делает свой запрос к БД | Cache stampede при холодном старте | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.5 | Redis pub/sub для cross-instance invalidation | Только local cache invalidation | Старые данные на других инстансах | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.6 | `validate_registry_vs_manifest()` при старте | Manifest и registry рассинхронизированы | Модуль заявлен в manifest но не зарегистрирован (или наоборот) | [modules/manifest.md](../modules/manifest.md) |

---

## 7. API

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 7.1 | GraphQL для UI-клиентов (admin, storefront) | REST для всего | Over-fetching, N+1 queries, множество endpoints | [architecture/api.md](../architecture/api.md) |
| 7.2 | REST для интеграций, webhooks, batch jobs | GraphQL для machine-to-machine | Сложность парсинга, кэширование, SDK generation | [architecture/api.md](../architecture/api.md) |
| 7.3 | DataLoaders для N+1 prevention | Inline DB queries в resolvers | O(n) запросов вместо O(1) batched | [architecture/dataloader.md](../architecture/dataloader.md) |
| 7.4 | `#[utoipa::path(...)]` для OpenAPI | REST endpoints без OpenAPI annotations | Swagger UI не показывает endpoint | — |
| 7.5 | `validator` crate для input validation на DTOs | Manual if/else проверки в handlers | Непоследовательная валидация, пропущенные поля | [guides/input-validation.md](../guides/input-validation.md) |
| 7.6 | GraphQL error extensions для structured errors | Plain string errors в GraphQL | Клиент не может программно обработать ошибку | [standards/errors.md](errors.md) |
| 7.7 | Пагинация в list-запросах (limit/offset или cursor) | List без пагинации | Загрузка всей таблицы в память, OOM | — |
| 7.8 | `MergedObject` для модульной GraphQL schema | Единый монолитный Query/Mutation type | Coupling, невозможно отключить модуль | [architecture/api.md](../architecture/api.md) |

---

## 8. Фронтенд

### 8.1 Leptos (Admin / Storefront)

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 8.1.1 | `leptos-graphql` для GraphQL queries | Ручной fetch + manual JSON parsing | Нет типизации, ручной error handling | — |
| 8.1.2 | `leptos-auth` для auth state management | Ручное управление JWT в localStorage | Race conditions, нет refresh logic | — |
| 8.1.3 | `leptos-zustand` для глобального состояния | Props drilling через 5+ уровней | Нечитаемый код, пересоздание компонентов | — |
| 8.1.4 | `leptos-hook-form` для форм | Manual form state + onChange handlers | Boilerplate, отсутствие валидации | — |
| 8.1.5 | `iu-leptos` компоненты из design system | Кастомные компоненты с собственными стилями | Визуальная несогласованность | — |
| 8.1.6 | SSR для storefront (SEO) | CSR-only storefront | Нет SEO, медленный First Contentful Paint | — |
| 8.1.7 | CSR для admin panel (WASM) | SSR для admin panel | Admin не нуждается в SEO, CSR проще | — |

### 8.2 Next.js (Admin / Frontend)

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 8.2.1 | Packages из `packages/` (leptos-auth, leptos-graphql, etc.) | Дублирование кода между next-admin и next-frontend | Copy-paste, рассинхрон | — |
| 8.2.2 | TypeScript strict mode | `any` типы и `@ts-ignore` | Потеря type safety, runtime ошибки | — |
| 8.2.3 | Server Components для data fetching (Next.js 13+) | `useEffect` + fetch в каждом компоненте | Waterfalls, нет streaming | — |
| 8.2.4 | Clerk auth (next-admin) интегрирован с server JWT | Отдельные auth системы на фронте и бэке | Рассинхрон сессий | — |

---

## 9. Тестирование

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 9.1 | Трёхуровневая пирамида: unit → integration → contract | Только unit-тесты или только E2E | Дырки в покрытии, медленный feedback loop | [guides/testing.md](../guides/testing.md) |
| 9.2 | Polling с timeout вместо `sleep()` | `tokio::time::sleep(Duration::from_secs(1))` для ожидания async | Flaky тесты, ложные failures | [guides/testing.md](../guides/testing.md) |
| 9.3 | Transaction rollback для DB isolation в тестах | Общая БД без cleanup | Тесты зависят от порядка, flaky | [guides/testing.md](../guides/testing.md) |
| 9.4 | Mock **ports** (traits), не persistence layer | Мок SeaORM моделей напрямую | False confidence — реальные queries не тестируются | [guides/testing.md](../guides/testing.md) |
| 9.5 | Property tests для state machines | Только happy-path тесты для transitions | Пропущенные невалидные переходы | [guides/testing-property.md](../guides/testing-property.md) |
| 9.6 | Integration test для каждого нового DomainEvent | Event без тестов | Событие публикуется, но handler не обрабатывает | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 9.7 | Idempotency test для event handlers | Только happy-path event test | Дубликаты данных при retry | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 9.8 | `rustok-test-utils` для общих фикстур | Копирование test helpers между crates | Рассинхрон, дублирование | — |

---

## 10. Observability

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 10.1 | `#[instrument(skip(self, input))]` на service methods | Нет spans в сервисах | Невозможно трейсить request → service → DB | [standards/logging.md](logging.md) |
| 10.2 | Structured fields (`%tenant_id`, `%user_id`) | String formatting (`format!("user={}", id)`) | Нельзя фильтровать в Grafana/Loki | [standards/logging.md](logging.md) |
| 10.3 | Единый Prometheus registry | Разные registries в разных модулях | Метрики не экспортируются или дублируются | [ai/KNOWN_PITFALLS.md §Telemetry](../ai/KNOWN_PITFALLS.md) |
| 10.4 | Одна инициализация telemetry runtime | Множественная инициализация telemetry | Паника, дублирование spans, утечка памяти | [ai/KNOWN_PITFALLS.md §Telemetry](../ai/KNOWN_PITFALLS.md) |
| 10.5 | Info level для бизнес-событий, Error для failures | `tracing::error!` для всего | Alert fatigue, невозможно отделить важное | [standards/logging.md](logging.md) |
| 10.6 | НЕ логировать PII и secrets | Логирование email, password, tokens | GDPR violation, security breach | [standards/logging.md](logging.md) |

---

## 11. Безопасность

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 11.1 | HTTPS в production | HTTP без TLS | Man-in-the-middle, перехват токенов | [standards/security.md](security.md) |
| 11.2 | CSP, X-Frame-Options, HSTS headers | Без security headers | XSS, clickjacking | [standards/security.md](security.md) |
| 11.3 | Rate limiting на auth endpoints | Без rate limiting | Brute-force атаки на пароли | [standards/security.md](security.md) |
| 11.4 | SSRF allowlist для external URLs | Без проверки URL destinations | SSRF → доступ к internal services | [standards/security.md](security.md) |
| 11.5 | `Zeroize` для sensitive data в memory | Sensitive data остаётся в памяти после use | Memory dump → утечка secrets | [standards/coding.md §8.2](coding.md) |
| 11.6 | Secrets в env vars, не в коде | Hardcoded secrets/passwords/keys | Утечка при компрометации repo | [standards/security.md](security.md) |

---

## 12. DevOps и CI/CD

| # | ✅ Правильно | ❌ Неправильно | Почему | Детали |
|---|-------------|---------------|--------|--------|
| 12.1 | `cargo fmt --all && cargo clippy -- -D warnings` перед коммитом | Commit без formatting/linting | Шумные diff'ы, скрытые проблемы | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 12.2 | Conventional commits (`feat:`, `fix:`, `docs:`) | Произвольные commit messages | Невозможно автогенерировать CHANGELOG | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 12.3 | Branch naming: `feature/`, `fix/`, `docs/` | Произвольные имена веток | Путаница, нет автоматизации | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 12.4 | `cargo deny check` в CI | Без проверки лицензий и advisory | Vulnerable dependencies, license violations | — |
| 12.5 | Не редактировать CI/CD файлы без явного запроса | Автоматическое изменение workflow файлов | Сломанный CI для всех | [AGENTS.md](../../AGENTS.md) |
| 12.6 | Обновлять docs при изменении кода | Изменить код без обновления документации | Документация врёт, новые разработчики путаются | [AGENTS.md](../../AGENTS.md) |

---

## Связанные документы

- [Запрещённые действия (NEVER DO)](./forbidden-actions.md) — жёсткие запреты с последствиями
- [Стандарты кода](./coding.md) — детальный гайд с примерами
- [Обработка ошибок](./errors.md) — error handling patterns
- [Безопасность](./security.md) — OWASP coverage
- [Логирование](./logging.md) — structured logging
- [Known Pitfalls](../ai/KNOWN_PITFALLS.md) — ловушки для AI-агентов
- [Architecture Principles](../architecture/principles.md) — архитектурные принципы
- [Граф зависимостей модулей](../architecture/diagram.md) — 12 Mermaid-диаграмм (включая dependency graph)
- [Категории модулей A/B/C](../architecture/modules.md) — compile-time vs runtime vs optional
- [Реестр компонентов](../modules/registry.md) — каталог всех crates, apps, packages
