# Payment provider webhooks

## Ownership

`rustok-payment` owns provider webhook verification, durable inbox state,
payment/refund lifecycle application, chargeback validation, retry classification,
bounded recovery, safe operator reads, and dead-letter replay. `rustok-commerce`
orchestrates the ecommerce family but must not parse provider payloads or persist
payment state.

Payment workstream completion and verification state is tracked in:

`crates/rustok-commerce/docs/implementation-plan.md#payment-workstream`

Marketplace financial/reversal completion and verification state is tracked in:

`crates/rustok-marketplace/docs/implementation-plan.md`

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
8. `PaymentObservedDomainEventApplier` routes payment/refund lifecycle events and
   validates completed chargeback facts against the authoritative payment collection.
9. After the payment owner stage succeeds, each host-composed
   `PaymentProviderProcessedEventObserver` receives the same immutable normalized event.
10. The marketplace observer ignores ordinary non-marketplace events. For events with a
    `marketplace_reversal` extension, it writes/processes the commerce reversal inbox and
    waits for append-only ledger evidence.
11. Mark the payment inbox event `processed` only after both the payment owner stage and
    every processed-event observer succeed.
12. Classify retryable failures as `failed`; permanent failures or exhausted retry
    budgets become `dead_letter`.

The same observed applier is mounted in webhook ingress, manual payment recovery, and
the scheduled payment recovery worker. Scheduled polling of already-processed events
remains as a bounded backfill/fallback for events committed before observer composition.

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
- `chargeback.completed`

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

Chargeback metadata:

```json
{
  "chargeback_id": "uuid",
  "collection_id": "uuid",
  "amount": "10.00",
  "currency_code": "USD",
  "metadata": {}
}
```

Authorized, captured, completed-refund, and completed-chargeback events require a
provider external reference. Provider adapters must return immutable owner ids in
normalized metadata; owner records are never discovered from an untrusted external
reference.

## Marketplace reversal extension

A completed refund or chargeback concerning marketplace lines carries a normalized
`marketplace_reversal` object either directly in event metadata or under
`metadata.marketplace_reversal`:

```json
{
  "marketplace_reversal": {
    "source_id": "refund-or-chargeback-uuid",
    "order_id": "order-uuid",
    "occurred_at": "2026-07-21T12:00:00Z",
    "currency_code": "USD",
    "currency_exponent": 2,
    "lines": [
      {
        "assessment_id": "uuid",
        "allocation_id": "uuid",
        "order_line_item_id": "uuid",
        "seller_id": "uuid",
        "commission_amount": 100,
        "seller_amount": 900,
        "seller_balance_bucket": "pending"
      }
    ]
  }
}
```

The extension contains normalized domain facts only. Payment and marketplace code do
not parse a provider SDK object or raw payload. Commerce converts the provider amount
to minor units exactly, rejects implicit rounding, verifies the line total, reloads
the authoritative refund/payment collection, and then invokes the marketplace root
financial port.

Ordinary non-marketplace refund and chargeback events omit this extension and are a
no-op for marketplace recovery.

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

The payment inbox enforces:

```text
(tenant_id, provider_id, delivery_id)
(tenant_id, provider_id, idempotency_key)
```

The marketplace reversal inbox additionally enforces:

```text
(tenant_id, provider_event_id)
(tenant_id, event_source, event_id)
(tenant_id, reversal_kind, source_id)
```

The same identity, payload digest, and normalized event returns the existing row.
Identity reuse with another payload or normalized event is rejected. A processed
replay does not repeat the owner command.

## Statuses and recovery

Payment provider inbox:

- `received`: verified facts stored, not claimed.
- `processing`: owned by one non-expired lease.
- `failed`: retryable failure without an active lease.
- `processed`: owner mutation and every observed post-owner effect committed.
- `dead_letter`: permanent failure or exhausted retry budget.

Marketplace reversal inbox:

- `received`;
- `processing`;
- `retryable_error`;
- `operator_review`;
- `processed`.

Automatic payment recovery selects only `received`, `failed`, and expired
`processing` rows. It never claims `dead_letter`. Recovery replays the payment owner
stage and the same host-composed observers.

The marketplace financial worker runs every 10 seconds with delayed missed ticks and
bounded batches. It adapts historical processed provider events containing marketplace
facts, recovers reversal inbox rows, and then runs the existing paid-event recovery
sweep. Existing reversal inbox deduplication makes this fallback safe alongside direct
observer delivery.

Manual terminal replay is limited to:

```text
dead_letter -> processing -> processed | dead_letter
```

It requires durable normalized facts and `payments:manage`.

## Safe operator projection

Payment operator APIs exclude idempotency keys, payload digest, normalized metadata,
lease details, raw error messages, signatures, and raw payloads.

Marketplace reversal operator APIs expose only normalized identifiers, reversal kind,
order/payment identity, currency/exponent, total amount, line count, status, stable
error fields, timestamps, and resulting reversal/ledger transaction ids. They never
return `lines_json` or provider metadata.

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
- Keep marketplace reversal lines in minor units and require exact amount conversion.
- Retry an operator-review reversal row only while no reversal or ledger transaction
  evidence has been stored.
- Compose processed-event observers through host runtime values; payment owner code must
  not depend on commerce implementations.

## Operator workflow

For payment `received`, `failed`, or expired `processing`:

1. Resolve the temporary owner/storage/observer dependency.
2. Allow the scheduled worker to recover the event, or call
   `POST /api/payment/provider-events/recovery/run?limit=N`.
3. Review stable error codes only.

For payment `dead_letter`:

1. Inspect the safe event projection.
2. Verify the provider-owned delivery reference in the provider dashboard.
3. Resolve identity, currency, amount, provider, lifecycle, or marketplace attribution
   conflicts.
4. Call `POST /api/payment/provider-events/{event_id}/replay`.
5. Never edit the inbox row manually.

For marketplace reversal `operator_review`:

1. Read the safe REST or GraphQL reversal projection.
2. Reconcile refund/chargeback, order, currency, and line attribution.
3. Retry only through the authenticated marketplace reversal operator mutation or REST
   route.
4. Never reset rows with stored reversal or ledger transaction evidence.
