# rustok-blog docs

В этой папке хранится документация модуля `crates/rustok-blog`.

## Documents

- [Implementation plan](./implementation-plan.md) — план развития модуля

## Модуль в картинке

```
┌─────────────────────────────────────────────────────────┐
│                    rustok-blog                          │
│                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │ PostService │  │  DTOs       │  │ StateMachine│     │
│  │             │  │             │  │             │     │
│  │ - create    │  │ - Create    │  │ Draft       │     │
│  │ - update    │  │ - Update    │  │ Published   │     │
│  │ - publish   │  │ - Response  │  │ Archived    │     │
│  │ - archive   │  │ - Query     │  │ Comment     │     │
│  │ - delete    │  │             │  │             │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
│         │                                     │         │
│         │                                     │         │
│         ▼                                     ▼         │
│  ┌─────────────┐                      ┌─────────────┐   │
│  │  BlogError  │                      │ Permissions │   │
│  │             │                      │             │   │
│  │ RichError   │                      │ posts:*     │   │
│  │ compatible  │                      │ comments:*  │   │
│  └─────────────┘                      │ categories:*│   │
│                                       │ tags:*      │   │
│                                       └─────────────┘   │
└─────────────────────────────────────────────────────────┘
                          │
                          │ Uses
                          ▼
┌─────────────────────────────────────────────────────────┐
│                  rustok-content                         │
│                                                         │
│  Nodes, Bodies, Translations (storage layer)           │
└─────────────────────────────────────────────────────────┘
```

## Ключевые решения

### Wrapper Module Pattern
Blog не создаёт собственные таблицы, а использует таблицы content-модуля с `kind = "post"`. Это:
- Уменьшает дублирование схемы
- Обеспечивает консистентность данных
- Позволяет использовать общий функционал (версионирование, локализация)

### Type-Safe State Machine
Статусы постов реализованы как типобезопасная state machine:
- Невалидные переходы невозможны на уровне компилятора
- Каждый статус содержит специфичные данные (published_at, reason)
- Легко тестировать и документировать

### Rich Errors
Все ошибки конвертируются в `RichError`:
- Понятные сообщения для пользователей
- Детальная информация для разработчиков
- Коды ошибок для автоматической обработки


## Roadmap / status

Краткая синхронизация с `implementation-plan.md`:

- ✅ `PostService` и `CommentService` реализованы и покрыты unit + частью integration сценариев.
- ⬜ `CategoryService` (`src/services/category.rs`) ещё не создан; запланирован на **Phase 3, P1** с отдельными DTO, tenancy-проверками и интеграцией в Post validation.
- ⬜ `TagService` (`src/services/tag.rs`) ещё не создан; запланирован на **Phase 3, P1** с нормализацией/уникальностью тегов и API-слоем.
- 🟨 Integration-тесты находятся в состоянии **partial**: в `tests/integration.rs` уже есть рабочие sqlite сценарии (comments/events), но часть post lifecycle тестов пока `#[ignore]` и должна быть доведена до CI-ready состояния (**Phase 3, P0**).

## Event contracts

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)

## Rich-text contract

- Blog/Forum backend принимает только `rt_json_v1` payload и выполняет обязательные `schema validation + sanitize` на сервере.
- Клиентские валидаторы считаются advisory-only и не являются источником истины.
- Детали спецификации: [docs/standards/rt-json-v1.md](../../../docs/standards/rt-json-v1.md).
