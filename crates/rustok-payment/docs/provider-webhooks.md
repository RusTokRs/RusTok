# Payment provider webhooks

## Ownership

`rustok-payment` owns provider webhook verification, durable inbox state,
payment/refund lifecycle application, retry classification, and dead-letter
operations. `rustok-commerce` must not parse provider payloads or mutate payment
state from a webhook controller.

The machine-readable contract is
`contracts/payment-provider-webhook-v1.json`.

## Execution order

1. The HTTP adapter obtains tenant scope, provider id, delivery id,
   idempotency key, signature headers, and the raw body.
2. Axum rejects bodies larger than 1 MiB before provider code runs.
3. `PaymentProviderRegistry::execute_webhook` invokes the selected provider.
   The provider verifies the signature and returns a normalized event. It must
   not mutate payment lifecycle state.
4. `PaymentProviderEventJournal::receive` stores the SHA-256 payload hash and
   delivery identity. The raw body is not persisted.
5. The event receives a bounded processing lease.
6. `PaymentDomainEventApplier` routes the normalized event to payment or refund
   owner commands.
7. The inbox event becomes `processed` only after the owner command succeeds.
8. Retryable failures become `failed`; permanent failures or exhausted retries
   become `dead_letter`.

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

Expired `processing` rows and `failed` rows are retryable. `processed` and
`dead_letter` rows are terminal.

## Security rules

- Never persist or log the raw provider body.
- Never mark `signature_verified` from an HTTP header alone. Only a successful
  provider SPI result may enter the inbox.
- Never expose provider SDK, SQL, signature, or raw payload errors in HTTP
  responses.
- Keep normalized metadata below 64 KiB and depth 16.
- Require tenant scope before provider lookup or inbox access.
- Production profiles must not expose the manual provider webhook adapter.

## Operator workflow

For a `failed` event:

1. Inspect its safe `error_code`, provider id, delivery id, event type, attempt
   count, and owner reference.
2. Fix the owner/provider dependency.
3. Reclaim the event with a new lease and process it.

For a `dead_letter` event:

1. Verify the provider event in the provider dashboard.
2. Compare the stored payload hash and external reference.
3. Resolve malformed identity, currency, amount, or lifecycle ordering.
4. Use an explicit operator replay command after remediation. Do not edit the
   inbox row manually.

## Evidence status

The current implementation has source code, migrations, OpenAPI, and regression
tests. It does not yet have recorded live HTTP signature verification,
PostgreSQL concurrency, provider redelivery, or dead-letter replay evidence.
The webhook boundary therefore remains `source_only` / `boundary_ready`, not
`transport_verified`.
