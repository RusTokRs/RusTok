# server

## Назначение
`apps/server` — модуль/приложение RusToK. Здесь находится его код и корневая документация.

## Взаимодействие
- crates/rustok-core
- доменные модули crates/rustok-*
- apps/admin, apps/storefront и Next.js приложения как клиенты API/transport endpoints
- `rustok-mcp` как platform MCP adapter; persisted MCP management/API, Alloy scaffold draft control plane и DB-backed runtime bridge живут в `apps/server`, включая live bridge обратно в persisted draft store
- `rustok-content` orchestration runtime bridge и GraphQL conversion-команды (`promote_topic_to_post`, `demote_post_to_topic`, `split_topic`, `merge_topics`) также живут в `apps/server`

## UI transport surfaces

- `/api/graphql` остаётся обязательным публичным и headless-friendly контрактом.
- `/api/fn/*` обслуживает Leptos `#[server]` functions через `leptos_axum`.
- Для Leptos UI native `#[server]` transport добавляется параллельно GraphQL и не заменяет его.

## Документация
- Локальная документация: `./docs/`
- Общая документация платформы: `/docs`

## Конфигурация транспорта событий

Сервер поддерживает выбор транспорта доменных событий через `settings.rustok.events.transport`
или переменную окружения `RUSTOK_EVENT_TRANSPORT`.

- `memory` — in-memory bus для локальной разработки.
- `outbox` — запись событий в outbox-таблицу с фоновым relay worker.
- `iggy` — стриминг через Iggy transport.

Пример для YAML:

```yaml
settings:
  rustok:
    events:
      transport: outbox
      relay_interval_ms: 1000
```

Переменная окружения имеет приоритет над YAML:

```bash
RUSTOK_EVENT_TRANSPORT=iggy
```

При неверном значении сервер завершит старт с понятной ошибкой валидации конфигурации.

## Паспорт компонента
- **Роль в системе:** Главный backend RusToK: API, модули, миграции, orchestration runtime.
- **Основные данные/ответственность:** бизнес-логика и API данного компонента; структура кода и документации в корне компонента.
  Также platform-owned слои вроде MCP management API, persisted Alloy scaffold drafts и DB-backed MCP runtime bridge.
- **Взаимодействует с:**
  - crates/rustok-core
  - все доменные crates/rustok-*
  - apps/admin/apps/storefront как клиенты API
- **Точки входа:**
  - `apps/server/src/main.rs`
  - `apps/server/src/controllers/*`
- **Локальная документация:** `./docs/`
- **Глобальная документация платформы:** `/docs/`
