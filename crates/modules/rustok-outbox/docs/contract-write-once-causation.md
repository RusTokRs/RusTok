# Exact caused contract write-once boundary

Status: `source_complete_owner_execution_pending`.

## Purpose

Some owner workflows already retain two durable identities before publishing a
bounded typed event:

- one exact idempotency identity that must become the typed envelope and inbox
  identity;
- one exact predecessor root-event identity that must remain transport
  causation metadata.

Generating either UUID again would break replay admission or causal tracing.
Copying the predecessor into a typed payload would also change the event-family
schema and its committed digest.

## Envelope contract

`ContractEventEnvelope::new_with_envelope_id_and_causation` accepts:

- a non-nil caller-owned envelope UUID;
- exact tenant and optional actor scope;
- a non-nil predecessor envelope UUID;
- one sealed `EventContract` payload.

The caller-owned envelope UUID is used as both `id` and `correlation_id`.
The predecessor is stored only in the existing optional `causation_id` envelope
field. The constructor delegates to the same registered-envelope validation as
ordinary typed publication.

This is a constructor/API extension only. It does not add an event family,
change a payload, alter the serialized envelope shape, or require an update to
`crates/rustok-events/contracts/event-contract-digests.json`.

## Canonical outbox contract

`TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id_and_causation`
constructs the exact caused envelope and passes it to the existing canonical
`OutboxTransport::write_contract_envelope_once_in_tx` boundary.

The write path:

1. inserts into `sys_events` by caller-owned primary key with conflict ignored;
2. reads the winning row through the same owner transaction;
3. validates row metadata and the decoded registered envelope;
4. compares envelope ID, correlation ID, causation ID, tenant, actor, event type,
   schema version and typed payload;
5. accepts an exact replay or returns `ContractEventWriteOnceError::Conflict`.

Generated timestamp and trace metadata remain owned by the first writer and are
not compared. Database, serialization, missing-row or validation failures remain
`Unavailable`.

## Product Index usage

The Product locale and ProductVariant refresh ledgers already retain:

- `refresh_id`, reserved as the typed envelope and Index inbox identity;
- `root_event_id`, the exact Product lifecycle predecessor.

A later Product Index relay can therefore publish each ledger row with:

```text
id = correlation_id = refresh_id
causation_id = root_event_id
```

without reconstructing identity, copying causation into the payload, or creating
a Product-specific outbox writer.

## Deliberate limits

This slice does not:

- add Product/ProductVariant typed event families;
- update event schemas or committed digests;
- read Product refresh ledgers;
- start a relay or broker consumer;
- register Index mutation routes;
- acknowledge broker deliveries;
- add retry, DLQ or retention policy;
- change the concrete-repair evidence gate.

## Maintainer verification

```bash
cargo test -p rustok-events contract::tests --lib -- --nocapture
cargo test -p rustok-outbox --test contract_write_once -- --nocapture
node scripts/verify/verify-outbox-contract-write-once-causation.mjs
cargo check -p rustok-events --all-targets
cargo check -p rustok-outbox --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, database scenarios,
workflows or CI were executed by the implementation agent.
