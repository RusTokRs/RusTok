# Payment provider webhooks

## Ownership

`rustok-payment` owns provider webhook verification, durable inbox state,
payment/refund lifecycle application, retry classification, bounded recovery,
safe operator reads, and dead-letter replay. `rustok-commerce` must not parse
provider payloads or mutate payment state from a webhook controller.

The machine-readable contract is
`contracts/payment-provider-webhook-v1.json`.

Implementation tasks, completion marks, verification state, and promotion gates
are tracked only in `docs/implementation-plan.md`. This document is an
operational runbook and must not maintain a second roadmap or status checklist.

## Mounted HTTP routes

Provider ingress:

```text
POST /payment/webhooks/{provider_id}
```

Tenant-scoped operator routes:

```text
GET  /api/payment/provider-events/{event_id}
GET  /api/payment/provider-events/dead-letter?limit=50
POST /api/payment/provider-events/recovery/run?limit=50
POST /api/payment/provider-events/{event_id}/replay
```

Reading requires `payments:read` or `payments:manage`. Recovery and dead-letter
replay require `payments:manage`.

## Execution order

1. The HTTP adapter resolves tenant scope, provider id, delivery id,
   idempotency key, one supported signature header, and the raw body.
2. Axum rejects empty bodies and bodies larger than 1 MiB.
3. `PaymentProviderRegistry::execute_webhook` invokes the registered provider's
   `handle_webhook` method. The provider verifies the signature and returns a
   normalized event. It must not mutate payment lifecycle state.
4. `PaymentProviderEventJournal::receive_verified` atomically stores the
   SHA-256 payload hash, immutable delivery identity, normalized event type,
   external reference, and bounded normalized metadata. The raw body and
   signature are not persisted.
5. The event receives a bounded processing lease.
6. `PaymentDomainEventApplier` routes the durable normalized event to payment or
   refund owner commands.
7. The inbox event becomes `processed` only after the owner command succeeds.
8. Retryable failures become `failed`; permanent failures or exhausted retries
   become `dead_letter`.

Writing verified normalized facts with the first inbox receipt removes the
crash window in which the raw body was discarded before replayable facts became
durable. A compatibility checkpoint method remains only for legacy rows and
tests. Database triggers make normalized facts immutable after their first
verified write.

## Normalized event contract

Supported event types:

- `payment.authorized`
- `payment.captured`
- `payment.cancelled`
- `refund.completed`

Payment events require normalized metadata:

```json
{
  "collection_id": "uuid",
  "amount": "25.00",
  "currency_code": "USD",
  "metadata": {}
}
```

Refund completion requires:

```json
{
  "refund_id": "uuid",
  "amount": "10.00",
  "currency_code": "USD",
  "metadata": {}
}
```

Authorized, captured, and completed-refund events also require a provider
external reference.

Provider adapters must not ask the owner layer to locate records by an
untrusted external reference. They must return the immutable owner id in the
normalized metadata.

## Deduplication

The inbox enforces both identities:

```text
(tenant_id, provider_id, delivery_id)
(tenant_id, provider_id, idempotency_key)
```

A replay with the same identity, payload hash, and normalized facts returns the
existing event. A reused identity with another payload or different normalized
facts is rejected.

A `processed` replay does not run the owner command again. If the owner command
succeeded but the final inbox update failed, provider redelivery or the recovery
worker is safe because owner payment/refund transitions accept the already
applied state.

## Inbox statuses

- `received`: verified normalized facts are stored, not claimed.
- `processing`: claimed by one non-expired lease.
- `failed`: retryable failure, no active lease.
- `processed`: owner mutation committed.
- `dead_letter`: permanent failure or retry budget exhausted.

Expired `processing` rows, `failed` rows, and unclaimed `received` rows are
eligible for bounded recovery. They are not confused with `dead_letter`:
automatic recovery queries never claim dead-letter rows.

The only allowed terminal replay transition is:

```text
dead_letter -> processing -> processed | dead_letter
```

It is initiated by the protected operator replay endpoint and requires durable
normalized facts. A failed manual replay returns the event to `dead_letter`
rather than creating an automatic retry loop.

## Bounded retry recovery

The protected endpoint
`POST /api/payment/provider-events/recovery/run?limit=N` performs a tenant-scoped
bounded sweep. The journal clamps the limit to `1..100` and selects only:

- `received` events;
- `failed` events;
- `processing` events whose lease expired.

Each event is claimed with its own CAS lease and applied from the durable
normalized checkpoint. The response reports only counts plus event id, status,
and stable error code for failures. It does not return internal provider or SQL
messages. A legacy row without normalized facts is moved to `dead_letter`
instead of being retried forever.

The standard server background-worker lifecycle executes the same
`PaymentProviderEventRecoveryService::run` path on a bounded delayed interval
when the runtime profile enables background workers. The worker reuses the
shared shutdown handle, prevents duplicate startup within one process, pages
through tenants, and relies on inbox CAS leases for cross-replica exclusion. The
HTTP endpoint remains an explicit operator-triggered sweep of the same recovery
contract.

## Safe operator projection

The operator API returns only:

- event id;
- provider id and delivery id;
- status and normalized event type;
- external reference;
- attempt count and stable error code;
- received, updated, and processed timestamps.

It deliberately excludes the idempotency key, payload hash, normalized metadata,
lease owner, lease expiry, raw error message, signature, and raw payload.

## Security rules

- Never persist or log the raw provider body or signature.
- Never mark `signature_verified` from an HTTP header alone. Only a successful
  provider SPI result may enter the inbox.
- Persist verified normalized facts atomically with the first inbox receipt.
- Never rewrite normalized facts after their verified receipt.
- Never expose provider SDK, SQL, signature, raw payload, or internal error text
  in HTTP responses.
- Keep normalized metadata below 64 KiB and depth 16.
- Require tenant scope before provider lookup or inbox access.
- Require `payments:manage` for recovery and dead-letter replay.
- Production profiles must not expose the manual provider as a webhook adapter.

## Operator workflow

For a retryable `received`, `failed`, or expired `processing` event:

1. Resolve the temporary owner/storage dependency.
2. Allow the scheduled worker to reclaim it, or call
   `POST /api/payment/provider-events/recovery/run?limit=N` with
   `payments:manage` for an immediate bounded sweep.
3. Review only stable failure codes; internal error messages remain private.

For a `dead_letter` event:

1. Inspect its safe provider, delivery, event type, attempt count, owner
   reference, and error code.
2. Verify the event in the provider dashboard using the provider-owned delivery
   reference; raw payload data is intentionally absent from RusToK storage.
3. Resolve the identity, currency, amount, provider, or lifecycle conflict.
4. Call `POST /api/payment/provider-events/{event_id}/replay` with
   `payments:manage`.
5. Do not edit the inbox row manually.

## Status reference

Current completion marks, unexecuted verification steps, and the promotion state
are maintained only in `docs/implementation-plan.md`. Update that file in the
same commit as any source, contract, evidence, or runtime-verification change.
