# Payment provider webhooks

## Ownership

`rustok-payment` owns provider webhook verification, durable inbox state,
payment/refund lifecycle application, retry classification, safe operator reads,
and dead-letter replay. `rustok-commerce` must not parse provider payloads or
mutate payment state from a webhook controller.

The machine-readable contract is
`contracts/payment-provider-webhook-v1.json`.

## Mounted HTTP routes

Provider ingress:

```text
POST /payment/webhooks/{provider_id}
```

Tenant-scoped operator routes:

```text
GET  /api/payment/provider-events/{event_id}
GET  /api/payment/provider-events/dead-letter?limit=50
POST /api/payment/provider-events/{event_id}/replay
```

Reading requires `payments:read` or `payments:manage`. Replay requires
`payments:manage`.

## Execution order

1. The HTTP adapter resolves tenant scope, provider id, delivery id,
   idempotency key, one supported signature header, and the raw body.
2. Axum rejects empty bodies and bodies larger than 1 MiB.
3. `PaymentProviderRegistry::execute_webhook` invokes the registered provider's
   `handle_webhook` method. The provider verifies the signature and returns a
   normalized event. It must not mutate payment lifecycle state.
4. `PaymentProviderEventJournal::receive` stores the SHA-256 payload hash and
   immutable delivery identity. The raw body and signature are not persisted.
5. The event receives a bounded processing lease.
6. The verified normalized event type, external reference, and metadata are
   checkpointed while the lease is active.
7. `PaymentDomainEventApplier` routes the normalized event to payment or refund
   owner commands.
8. The inbox event becomes `processed` only after the owner command succeeds.
9. Retryable failures become `failed`; permanent failures or exhausted retries
   become `dead_letter`.

Checkpointing before owner apply is what makes operator replay possible without
retaining sensitive raw provider bytes.

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

A replay with the same identity and payload hash returns the existing event. A
reused identity with another payload hash is rejected.

A `processed` replay does not run the owner command again. If the owner command
succeeded but the final inbox update failed, provider redelivery is safe because
owner payment/refund transitions accept the already-applied state.

## Inbox statuses

- `received`: verified and stored, not claimed.
- `processing`: claimed by one non-expired lease.
- `failed`: retryable failure, no active lease.
- `processed`: owner mutation committed.
- `dead_letter`: permanent failure or retry budget exhausted.

Expired `processing` rows and `failed` rows are retryable. They are not confused
with `dead_letter`: automatic retry queries do not claim dead-letter rows.

The only allowed terminal replay transition is:

```text
dead_letter -> processing -> processed | dead_letter
```

It is initiated by the protected operator endpoint and requires a normalized
checkpoint. A failed manual replay returns the event to `dead_letter` rather
than creating an automatic retry loop.

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
- Never expose provider SDK, SQL, signature, raw payload, or internal error text
  in HTTP responses.
- Keep normalized metadata below 64 KiB and depth 16.
- Require tenant scope before provider lookup or inbox access.
- Require `payments:manage` for a dead-letter replay.
- Production profiles must not expose the manual provider as a webhook adapter.

## Operator workflow

For a retryable `failed` event, provider redelivery or a recovery worker may
reclaim the event after the owner/provider dependency is available.

For a `dead_letter` event:

1. Inspect its safe provider, delivery, event type, attempt count, owner
   reference, and error code.
2. Verify the event in the provider dashboard using the provider-owned delivery
   reference; raw payload data is intentionally absent from RusToK storage.
3. Resolve the identity, currency, amount, provider, or lifecycle conflict.
4. Call `POST /api/payment/provider-events/{event_id}/replay` with
   `payments:manage`.
5. Do not edit the inbox row manually.

## Evidence status

The implementation has source code, migrations, module-codegen routing, OpenAPI,
and regression tests. The tests have not been executed in this change session,
and there is no recorded live external-provider signature verification,
PostgreSQL concurrency, provider redelivery, or HTTP dead-letter replay evidence.
The boundary therefore remains `source_only` / `boundary_ready`, not
`transport_verified`.
