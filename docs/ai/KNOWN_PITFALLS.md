---
id: doc://docs/ai/KNOWN_PITFALLS.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# KNOWN_PITFALLS для AI (RusToK)

Короткий список типичных ошибок перед изменениями кода.

## Loco

- Не обходить Loco hooks (`Hooks::routes`, `Hooks::after_routes`, `Hooks::connect_workers`) через отдельный жизненный цикл «чистого Axum». См. `docs/references/loco/README.md`.
- Не заменять `AppContext` на глобальные singleton-объекты, если зависимость уже должна жить в `ctx.shared_store`.
- Не смешивать произвольные контракты ошибок: для контроллеров придерживаться `loco_rs::Result<...>`.

## Iggy / Outbox

- Для write + event не использовать fire-and-forget `publish(...)`; нужен `publish_in_tx(...)`.
- Не переносить в код Kafka/NATS-специфичные API (offset commits, subject-only routing), которых нет в текущем abstraction.
- Не выдумывать конфигурацию Iggy: сначала сверяться с актуальными `IggyConfig`, `ConnectorConfig`, `ConnectorMode`.


## MCP

- Не обходить typed tools/response envelope (`McpToolResponse`) ad-hoc JSON-ответами.
- Не переносить бизнес-логику в MCP адаптер: слой должен оставаться тонким над service/registry.
- Для ограниченного доступа использовать allow-list инструментов через `McpServerConfig::with_enabled_tools(...)`.

## Outbox

- Для write + event, требующих консистентности, использовать `publish_in_tx(...)`, а не `publish(...)`.
- Не запускать production c outbox без relay-воркера.

## Telemetry

- Не делать многократную инициализацию telemetry runtime.
- Не разносить метрики по разным registry без необходимости.

## Database / SeaORM

- **Всегда** добавлять `.filter(...::Column::TenantId.eq(tenant_id))` к каждому SELECT/UPDATE/DELETE. Запрос без `tenant_id` — это cross-tenant data leak.
- Не использовать `find().all(&db)` без фильтра — загрузит ВСЮ таблицу.
- Не создавать domain-таблицы без поля `tenant_id UUID NOT NULL`.
- Не использовать string concatenation для SQL — только параметризованные запросы через SeaORM.
- Не возвращать Entity напрямую из API — создавать отдельные DTO (Input/Response).
- Не делать hard DELETE для бизнес-сущностей (products, orders, nodes) — использовать soft delete через status = Archived.
- Миграции именовать строго: `mYYYYMMDD_<module>_<nnn>_<description>`.

## State Machines

- Не использовать `String` для status полей — использовать enum с type-safe transitions.
- Не добавлять переходы между состояниями без обновления property tests (`*_proptest.rs`).
- Не допускать «обратных» переходов без явного ADR (например, Published → Draft).
- Каждый новый state machine обязан иметь proptest для exhaustive проверки переходов.

## Frontend / Leptos

- Не использовать `fetch()` напрямую — использовать `leptos-graphql` для GraphQL queries.
- Не хранить JWT вручную в localStorage — использовать `leptos-auth`.
- Не копировать компоненты между admin и storefront — использовать `iu-leptos` design system.
- Не делать SSR для admin panel (использовать CSR/WASM) и не делать CSR для storefront (использовать SSR для SEO).
- Не пробрасывать props через 5+ уровней — использовать `leptos-zustand` для глобального состояния.

## Frontend / Next.js

- Не дублировать код между `apps/next-admin` и `apps/next-frontend` — выносить в `packages/`.
- Не использовать `any` типы — строгий TypeScript mode.
- Не забывать про Clerk ↔ Server JWT интеграцию в `apps/next-admin`.
- Не использовать `@ts-ignore` / `@ts-expect-error` — исправлять типы.

## Docker / Deployment

- Не запускать production с `transport = "memory"` — использовать `transport = "outbox"`.
- Не забывать relay worker при deployment с outbox transport.
- Не использовать default credentials из `.env.dev.example` в production.
- Не экспонировать `/swagger` и `/metrics` без auth в production.

## Migrations

- Не менять уже применённые миграции — создавать новые.
- Не удалять колонки без предварительного ADR и migration plan.
- Не создавать миграции вне `RusToKModule::migrations()` — используй стандартный механизм.
- Не забывать добавить миграцию для каждой новой entity.

## Обязательная проверка перед изменениями

Если задача затрагивает Loco/Iggy/MCP/Outbox/Telemetry/Database/Frontend:
1. Сначала открыть соответствующий reference-пакет:
   - `docs/references/loco/README.md`
   - `docs/references/iggy/README.md`
   - `docs/references/mcp/README.md`
   - `docs/references/outbox/README.md`
   - `docs/references/telemetry/README.md`
2. Прочитать [Запрещённые действия](../standards/forbidden-actions.md) — жёсткие запреты.
3. Прочитать [Паттерны vs Антипаттерны](../standards/patterns-vs-antipatterns.md) — сводная таблица.
4. Только после этого менять код/документацию.
