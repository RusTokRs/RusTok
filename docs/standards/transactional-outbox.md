---
id: doc://docs/standards/transactional-outbox.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Стандарт: Transactional Outbox

## Цель

Зафиксировать единое обязательное правило публикации событий:
**изменение доменных данных и запись события в outbox выполняются только в рамках одной и той же транзакции БД**.

Это гарантирует, что для критических доменных событий не возникает состояний:
- данные изменились, но событие потеряно;
- событие записано, но данные откатились.

## Обязательное правило

Для всех критических доменных событий:

1. Открыть транзакцию.
2. Выполнить изменения доменных данных.
3. Записать событие в outbox через транзакционный API (`publish_in_tx` или эквивалент).
4. Выполнить `commit`.

Если любой шаг до `commit` завершился ошибкой, транзакция должна быть откатана целиком.

## Допустимые и запрещённые API-пути

### Допустимо

- `publish_in_tx(...)`
- Эквиваленты, явно принимающие транзакционный контекст (`&txn`, `impl ConnectionTrait` внутри active transaction, unit-of-work с bound transaction).

Критерий допустимости: запись события в outbox выполняется тем же транзакционным соединением, что и изменение доменной модели.

### Запрещено для критических событий

- `publish(...)` после `commit`.
- Любой вызов публикации вне транзакции, если доменные данные уже изменены и зафиксированы.

Исключение возможно только для некритических/информационных событий, где допустима eventual consistency без строгой атомарности (должно быть явно задокументировано в модуле).

## Минимальные шаблоны SeaORM

Ниже — минимальные шаблоны с правильным порядком операций.

### Шаблон A: явная транзакция через `TransactionTrait`

```rust
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};

pub async fn handle_command(
    db: &DatabaseConnection,
    outbox: &dyn TransactionalEventBus,
    input: CommandInput,
) -> Result<(), DomainError> {
    let txn = db.begin().await?;

    // 1) Изменяем доменные данные в той же транзакции
    let aggregate = DomainRepo::update_in_tx(&txn, input).await?;

    // 2) Пишем событие в outbox в той же транзакции
    outbox
        .publish_in_tx(&txn, DomainEvent::aggregate_changed(&aggregate))
        .await?;

    // 3) Коммитим транзакцию только после успешной записи outbox
    txn.commit().await?;
    Ok(())
}
```

### Шаблон B: generic-функция от `ConnectionTrait` (выполняется внутри текущего txn)

```rust
use sea_orm::ConnectionTrait;

pub async fn persist_and_enqueue<C: ConnectionTrait>(
    conn: &C,
    outbox: &dyn TransactionalEventBus,
    aggregate: Aggregate,
) -> Result<(), DomainError> {
    DomainRepo::save(conn, &aggregate).await?;
    outbox
        .publish_in_tx(conn, DomainEvent::from(&aggregate))
        .await?;
}
```

Примечание: `persist_and_enqueue` не открывает и не коммитит транзакцию сама — она должна вызываться из кода, где уже обеспечен transaction boundary.

## Антипаттерн (нельзя для критических событий)

```rust
// ❌ Неправильно: событие публикуется после commit
let txn = db.begin().await?;
DomainRepo::update_in_tx(&txn, input).await?;
txn.commit().await?;
outbox.publish(DomainEvent::critical(...)).await?;
```

Даже при ретраях это оставляет окно потери события между `commit` и `publish`.

## Что проверять в PR

- Для критических событий в коде присутствует единый transaction boundary для domain write + outbox write.
- Используется `publish_in_tx` (или эквивалент с транзакционным контекстом), а не `publish` после `commit`.
- Порядок операций: `domain write -> outbox write -> commit`.
