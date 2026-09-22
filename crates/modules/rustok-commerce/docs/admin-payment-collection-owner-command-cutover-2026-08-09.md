# Commerce admin Payment collection owner-command cutover — 2026-08-09

Status: source-complete for mounted admin payment-collection authorize/capture/cancel routes; execution evidence pending and unvalidated.

## Scope

This slice advances the canonical ecommerce topology P0 without closing it. Three mounted admin payment-collection mutation routes now call a Payment-owned command capability instead of constructing Commerce `PaymentOrchestrationService` for their execution:

- `POST /admin/payment-collections/{id}/authorize`;
- `POST /admin/payment-collections/{id}/capture`;
- `POST /admin/payment-collections/{id}/cancel`.

Refund creation/completion/cancellation are intentionally out of scope and remain on the existing compatibility orchestration source for a separate owner-boundary slice.

## Payment owner capability

`rustok-payment` publishes `PaymentAdminCollectionCommandPort` and `PaymentAdminCollectionCommandRuntime` with authorize, capture, and cancel commands. The built-in adapter owns:

- payment collection lifecycle admission;
- provider selection;
- `PaymentProviderOperationJournal` access;
- provider execution through `PaymentProviderRegistry`;
- durable provider-result adoption;
- provider-payment identity recovery from the authorize journal;
- local Payment persistence after provider success;
- reconciliation-required checkpointing when provider outcome or local persistence is uncertain.

`PaymentService`, provider journal construction, and provider execution remain inside `rustok-payment` for this mounted path.

## Durable replay compatibility

The cutover preserves the existing provider journal identities exactly:

- `payment_collection:{collection_id}:authorize`;
- `payment_collection:{collection_id}:capture`;
- `payment_collection:{collection_id}:cancel`.

It also preserves the request payload metadata used by the pre-cutover Commerce orchestration:

- authorization keeps `commerce_orchestration.operation = authorize_payment_collection` plus `requested_provider_payment_id`;
- capture keeps `commerce_orchestration.operation = capture_payment_collection`;
- cancel keeps `commerce_orchestration.operation = cancel_payment_collection` plus the requested cancellation reason;
- capture/cancel continue to recover `provider_payment_id` from the durable authorize operation when the provider requires it.

Therefore upgraded retries target the same Payment-owned provider journal rows instead of changing provider operation identity.

## Port write identity

The mounted HTTP adapter supplies a bounded write `PortContext` containing tenant, authenticated user actor, locale, optional channel, a two-second deadline, and a stable transition-scoped admission identity:

`admin-payment-collection:{collection_id}:{operation}`

This key satisfies the generic write-port admission contract. It is not a newly claimed durable command receipt. Durable provider replay remains the existing Payment-owned journal and lifecycle behavior described above.

## Public transport compatibility

The mounted handlers preserve:

- `PAYMENTS_UPDATE` admission;
- the existing `AuthorizePaymentInput`, `CapturePaymentInput`, and `CancelPaymentInput` request bodies;
- `PaymentCollectionResponse` responses;
- validation/not-found/state-conflict/provider-unavailable/provider-invalid-response/reconciliation-required/provider-not-configured/storage-unavailable public HTTP families.

Internal Payment failures are mapped to bounded `PortError` categories and structural diagnostics. The new owner path does not place raw provider or database error text into public errors.

## Runtime composition

`CommerceHttpRuntime` first accepts a host-selected `PaymentAdminCollectionCommandRuntime`. When no external command runtime was injected, the built-in owner adapter is composed from the host database and the same host-selected `PaymentProviderRegistry` used by Commerce payment provider integration.

External command adapters therefore retain precedence while the built-in profile keeps provider selection consistent with the host registry.

## Remaining topology work

The canonical broad item “Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and Fulfillment concrete services behind host-composed owner ports” remains open.

For Payment admin HTTP specifically, the next source slice is refund command ownership: refund reservation/create replay, provider refund execution, refund completion, and refund cancellation. That follow-up must retain `payment_refund:{refund_id}` provider identity, caller refund creation idempotency, reserved-refund reconciliation semantics, and current public envelopes.

Other remaining Commerce Payment/Fulfillment/GraphQL concrete-service construction also keeps the broad P0 open.

## Validation status

`scripts/verify/verify-commerce-admin-payment-collection-owner-command-cutover.mjs` is added as a source guard but is intentionally not executed here.

No tests, Cargo commands, formatting, verifier execution, workflows, CI, runtime HTTP calls, restart evidence, or external-provider execution evidence are claimed in this slice.
