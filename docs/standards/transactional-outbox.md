---
id: doc://docs/standards/transactional-outbox.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Standard: Transactional Outbox

## Purpose

Define a single mandatory rule for event publishing:
**domain data changes and writing the event to the outbox must be performed within the same database transaction**.

This guarantees that for critical domain events, the following states do not occur:
- data changed but event lost;
- event written but data rolled back.

## Mandatory Rule

For all critical domain events:

1. Open a transaction.
2. Perform domain data changes.
3. Write the event to the outbox via a transactional API (`publish_in_tx` or equivalent).
4. Execute `commit`.

If any step before `commit` fails, the transaction must be rolled back entirely.

## Allowed and Forbidden API Paths

### Allowed

- `publish_in_tx(...)`
- Equivalents that explicitly accept a transactional context (`&txn`, `impl ConnectionTrait` inside an active transaction, unit-of-work with bound transaction).

The admissibility criterion: writing the event to the outbox uses the same transactional connection as the domain model changes.

### Forbidden for Critical Events

- `publish(...)` after `commit`.
- Any publish call outside a transaction, if domain data has already been changed and committed.

An exception is only possible for non-critical/informational events where eventual consistency without strict atomicity is acceptable (must be explicitly documented in the module).

## Minimal SeaORM Templates

Below are minimal templates with the correct operation order.

### Template A: Explicit transaction via `TransactionTrait`

```rust
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};

pub async fn handle_command(
    db: &DatabaseConnection,
    outbox: &dyn TransactionalEventBus,
    input: CommandInput,
) -> Result<(), DomainError> {
    let txn = db.begin().await?;

    // 1) Modify domain data in the same transaction
    let aggregate = DomainRepo::update_in_tx(&txn, input).await?;

    // 2) Write event to outbox in the same transaction
    outbox
        .publish_in_tx(&txn, DomainEvent::aggregate_changed(&aggregate))
        .await?;

    // 3) Commit the transaction only after successful outbox write
    txn.commit().await?;
    Ok(())
}
```

### Template B: Generic function from `ConnectionTrait` (executed within current txn)

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

Note: `persist_and_enqueue` does not open or commit a transaction itself — it must be called from code that already provides a transaction boundary.

## Antipattern (Not Allowed for Critical Events)

```rust
// ❌ Incorrect: event is published after commit
let txn = db.begin().await?;
DomainRepo::update_in_tx(&txn, input).await?;
txn.commit().await?;
outbox.publish(DomainEvent::critical(...)).await?;
```

Even with retries, this leaves a window for event loss between `commit` and `publish`.

## What to Check in PR

- For critical events, the code has a single transaction boundary for domain write + outbox write.
- Uses `publish_in_tx` (or equivalent with transactional context), not `publish` after `commit`.
- Operation order: `domain write -> outbox write -> commit`.
