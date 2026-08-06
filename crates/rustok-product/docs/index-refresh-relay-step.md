# Product Index refresh durable relay step

Status: `source_complete_typed_family_and_runtime_pending`.

## Purpose

Product owns two append-only Index refresh ledgers:

- localized Product identities;
- ProductVariant identities with their parent Product.

The canonical writer validates one ledger row and its exact causal Product root envelope before
writing a sealed typed contract to `sys_events`. This slice adds one bounded durable relay step
around that writer without starting a background worker or changing the event wire contract.

## Durable cursor

`product_index_refresh_relay_cursors` stores one row per `(tenant_id, stream_kind)` where
`stream_kind` is exactly `locale` or `variant`.

The cursor:

- starts at sequence `0`;
- cannot move backwards;
- cannot change tenant or stream identity;
- cannot be deleted;
- is locked with `FOR UPDATE` before publication;
- advances only in the same transaction as the canonical outbox write.

The table is Product-owned. It is not an Iggy consumer cursor and does not replace the global
outbox relay lifecycle.

## One-step algorithm

`ProductIndexRefreshRelayStep` exposes:

- `publish_next_locale(tenant_id)`;
- `publish_next_variant(tenant_id)`.

Each explicit call:

1. reads the current durable cursor;
2. requests at most one immutable ledger row after that cursor through the existing bounded source;
3. starts a Product transaction and locks the exact tenant/stream cursor;
4. returns `CursorAdvanced` without publication when another caller already advanced the cursor;
5. returns `Idle` when no row existed at the observation point;
6. asks `ProductIndexRefreshEventFactory` to build the sealed Product event;
7. invokes `ProductIndexRefreshCanonicalWriter`;
8. advances the cursor with an expected-value fence;
9. commits the typed outbox envelope and cursor together.

The ledger is append-only, so reading the candidate before locking the cursor does not permit the
candidate facts to change. A concurrent append after an idle observation is handled by the next
explicit step.

## Identity and atomicity

For a published row:

```text
outbox envelope id = outbox correlation id = ledger refresh_id
outbox causation id = ledger root_event_id
relay cursor = ledger sequence_no
```

A crash before commit retains neither the outbox write nor cursor movement. A crash after commit
retains both. Exact write-once admission remains an additional fail-closed guard, not a substitute
for the transactional cursor.

Concurrent callers may observe the same candidate before taking the lock. Only the caller whose
observed cursor still matches the locked cursor may publish it. Later callers receive
`CursorAdvanced` and can retry from the returned sequence.

## Typed factory boundary

`ProductIndexRefreshEventFactory` has separate associated event types and builders for locale and
variant rows. Both associated types must implement `ProductIndexRefreshContract`, which itself
extends the sealed platform `EventContract`.

The relay therefore does not accept arbitrary JSON, raw event-type strings or unrelated typed
families. The future Product event family remains responsible for exposing the exact target facts
that the canonical writer compares with the immutable ledger row.

## Ownership limits

The relay step does not:

- run a loop, timer, scheduler or spawned task;
- own a lease or retry schedule;
- dispatch directly to local delivery or Iggy;
- acknowledge a broker message;
- mutate Product ledger rows;
- write Index entities or links;
- construct an `IndexMutation`;
- expose a public repair transport;
- alter the concrete-repair evidence gate.

Delivery after commit remains owned by the global `OutboxRelay`. Product and ProductVariant broker
consumption still requires route registration plus a concrete commit-before-ack worker.

## Wire-contract block

This slice deliberately does not add `ProductIndexRefreshEvent` or update event-contract digests.
The committed digest artifact on current `main` still has the same blob and values as the state
before the Reactions typed family was added. A maintainer must first regenerate and admit the
canonical digest artifact through the repository generator; another family must not be layered on
an already stale release artifact.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-refresh-relay-step.mjs
cargo check -p rustok-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows or
CI were executed by the implementation agent.
