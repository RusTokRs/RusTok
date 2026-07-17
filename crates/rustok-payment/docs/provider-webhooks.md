# Payment provider webhooks

## Ownership

`rustok-payment` owns provider webhook verification, durable inbox state,
payment/refund lifecycle application, retry classification, bounded recovery,
safe operator reads, and dead-letter replay. `rustok-commerce` orchestrates the
ecommerce family but must not parse provider payloads or persist payment state.

Implementation tasks, completion marks, verification state, execution order, and
promotion gates are tracked only in:

`crates/rustok-commerce/docs/implementation-plan.md#payment-workstream`

This document is an operational runbook, not a roadmap.

Machine-readable contract:

`crates/rustok-payment/contracts/payment-provider-webhook-v1.json`

## Mounted routes

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

Reading requires `payments:read` or `payments:manage`. Recovery and replay require
`payments:manage`.

## Execution order

1. Resolve tenant, provider, provider signature header, optional delivery/replay
   identity hints, and the unchanged raw body.
2. Reject empty bodies and bodies larger than 1 MiB.
3. Invoke `PaymentProviderRegistry::execute_webhook`.
4. The selected provider verifies the signature and derives authoritative
   `delivery_id` and `replay_key` from the signed provider event. Optional transport
   headers are only cross-check hints and may not define durable identity.
5. The provider returns a normalized event without mutating payment lifecycle state.
6. `PaymentProviderEventJournal::receive_verified` atomically stores the SHA-256
   digest, provider-verified delivery/replay identities, normalized event type,
   external reference, and bounded normalized metadata. Raw body and signature are
   discarded.
7. Claim a bounded processing lease.
8. `PaymentDomainEventApplier` routes the normalized event to payment/refund owner
   commands.
9. Mark the inbox event `processed` only after the owner command succeeds.
10. Classify retryable failures as `failed`; permanent failures or exhausted retry
    budgets become `dead_letter`.

Writing verified identity and normalized facts with the first receipt removes the
crash window between signature verification and durable replay data. Database
guards make those facts immutable.

## Normalized event contract

Every verified result contains:

- `provider_id`;
- authoritative `delivery_id`;
- authoritative `replay_key`;
- normalized `event_type`;
- optional provider `external_reference`;
- bounded metadata object.

Supported event types:

- `payment.authorized`
- `payment.captured`
- `payment.cancelled`
- `refund.completed`

Payment metadata:

```json
{
  "collection_id": "uuid",
  "amount": "25.00",
  "currency_code": "USD",
  "metadata": {}
}
```

Refund metadata:

```json
{
  "refund_id": "uuid",
  "amount": "10.00",
  "currency_code": "USD",
  "metadata": {}
}
```

Authorized, captured, and completed-refund events require a provider external
reference. Provider adapters must return immutable owner ids in normalized
metadata; owner records are never discovered from an untrusted external reference.

## Identity and deduplication

Delivery and replay identities come from signature-verified provider data. These
headers are optional hints only:

```text
x-rustok-provider-delivery-id
x-webhook-id
idempotency-key
```

When a hint is present, it must equal the verified provider result or the request is
rejected before inbox insertion.

The inbox enforces:

```text
(tenant_id, provider_id, delivery_id)
(tenant_id, provider_id, idempotency_key)
```

The same identity, payload digest, and normalized event returns the existing row.
Identity reuse with another payload or normalized event is rejected. A processed
replay does not repeat the owner command.

## Statuses and recovery

- `received`: verified facts stored, not claimed.
- `processing`: owned by one non-expired lease.
- `failed`: retryable failure without an active lease.
- `processed`: owner mutation committed.
- `dead_letter`: permanent failure or exhausted retry budget.

Automatic recovery selects only `received`, `failed`, and expired `processing`
rows. It never claims `dead_letter`.

Manual terminal replay is limited to:

```text
dead_letter -> processing -> processed | dead_letter
```

It requires durable normalized facts and `payments:manage`.

The standard server background-worker lifecycle runs bounded recovery when the
runtime profile enables workers. It uses the shared shutdown handle, prevents
duplicate startup within one process, pages through tenants, and relies on CAS
leases for cross-replica exclusion. The recovery endpoint invokes the same service
for an immediate bounded operator sweep.

## Safe operator projection

The operator API returns only:

- event id;
- provider id and delivery id;
- status and normalized event type;
- external reference;
- attempt count and stable error code;
- received, updated, and processed timestamps.

It excludes idempotency key, payload digest, normalized metadata, lease details,
raw error message, signature, and raw payload.

## Security rules

- Never persist or log the raw provider body or signature.
- Never trust a signature header without successful provider verification.
- Never use unverified transport headers as durable delivery or replay identity.
- Persist provider-verified identity and normalized facts atomically with receipt.
- Never rewrite verified normalized facts.
- Never expose provider SDK, SQL, signature, raw body, or internal error text.
- Keep normalized metadata below 64 KiB and depth 16.
- Resolve tenant scope before provider lookup or inbox access.
- Require `payments:manage` for recovery and replay.
- Do not expose the manual provider as a production webhook adapter.

## Operator workflow

For `received`, `failed`, or expired `processing`:

1. Resolve the temporary owner/storage dependency.
2. Allow the scheduled worker to recover the event, or call
   `POST /api/payment/provider-events/recovery/run?limit=N`.
3. Review stable error codes only.

For `dead_letter`:

1. Inspect the safe event projection.
2. Verify the provider-owned delivery reference in the provider dashboard.
3. Resolve identity, currency, amount, provider, or lifecycle conflicts.
4. Call `POST /api/payment/provider-events/{event_id}/replay`.
5. Never edit the inbox row manually.

Current completion and verification status is maintained only in the main commerce
implementation plan.
