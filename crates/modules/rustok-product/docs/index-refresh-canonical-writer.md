# Product Index refresh canonical writer

Status: `source_complete_typed_family_and_relay_pending`.

## Purpose

Product locale and ProductVariant refresh ledgers already retain one immutable source row per exact
Index identity. Each row carries:

- `refresh_id`, reserved as the canonical typed envelope and downstream inbox identity;
- `root_event_id`, the exact Product lifecycle predecessor;
- tenant and target identity;
- the positive trigger-owned source revision.

This slice adds the canonical write-once handoff to `sys_events` without defining the public Product
refresh event family or starting a relay loop.

## Typed boundary

`ProductIndexRefreshContract` extends the sealed `rustok_events::EventContract`. The trait is owned by
`rustok-product`, so only Product can implement it for the future Product refresh family. Other
modules cannot implement `EventContract` for arbitrary payloads and cannot route an unrelated typed
family through Product ledger identities.

The event must expose one exact target:

- `Locale { product_id, locale, source_version }`; or
- `Variant { product_id, variant_id, source_version }`.

`ProductIndexRefreshCanonicalWriter` compares those typed facts with the immutable ledger row before
any outbox write. A target-kind, identity, locale, variant or revision mismatch returns
`ProductIndexRefreshPublicationError::ContractMismatch`.

## Causal root validation

The refresh ledger intentionally has no foreign key to `sys_events`, so the writer proves causation
inside the caller-owned transaction before publishing. It reads `root_event_id` from canonical
`sys_events`, decodes and revalidates the registered root `EventEnvelope`, and requires:

- row ID, envelope ID and ledger `root_event_id` to match;
- durable row event type and schema version to match the decoded envelope;
- envelope tenant to match the ledger tenant;
- the root payload to be `ProductCreated`, `ProductUpdated`, `ProductPublished` or `ProductDeleted`;
- the root payload Product identity to match the ledger Product identity.

A missing, non-root, corrupt or mismatched predecessor returns
`ProductIndexRefreshPublicationError::CausationMismatch`. Database read failures remain
`Unavailable`.

## Canonical envelope identity

For a matching row, causal root and event, the writer calls the shared exact-caused write-once
boundary with:

```text
id = correlation_id = refresh_id
causation_id = root_event_id
tenant_id = ledger tenant
actor_id = validated root envelope actor
```

The derived refresh event does not reconstruct an actor from mutable Product state. It preserves the
optional actor already retained by the exact causal root envelope.

Exact replay returns the same `refresh_id`. Reuse of that identity for different envelope scope,
causation or typed facts returns `Conflict`. Validation, serialization and database failures return
`Unavailable` without leaking infrastructure details.

## Ownership

The writer:

- writes only through the canonical `rustok-outbox` transaction API;
- does not dispatch directly to local delivery or Iggy;
- does not own retry, leases, DLQ, retention or acknowledgement;
- does not mutate the append-only Product refresh ledgers;
- does not write Index tables or construct `IndexMutation` values;
- requires a caller-owned database transaction.

Delivery after commit remains owned by the global `OutboxRelay` and the selected
`outbox_local|outbox_iggy` profile.

## Deliberate limits

This slice does not:

- add `ProductIndexRefreshEvent` to `rustok-events`;
- change the event registry, transport schema or committed digests;
- implement the Product trait for an event family that does not yet exist;
- page the ledgers or persist a relay cursor;
- register Product or ProductVariant Index routes;
- start a concrete broker consumer;
- add retry, DLQ or commit-before-ack evidence;
- alter or bypass the concrete-repair evidence gate.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-refresh-canonical-writer.mjs
cargo check -p rustok-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, database scenarios, workflows or CI were executed
by the implementation agent.
