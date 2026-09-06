# Commerce admin refund owner-command cutover — 2026-08-09

Status: source-complete for the mounted admin refund create/complete/cancel routes; execution evidence pending and unvalidated.

## Scope

This slice advances the canonical ecommerce topology P0 without closing it. The mounted admin refund mutations now call a Payment-owned command capability instead of constructing Commerce `PaymentOrchestrationService`:

- `POST /admin/payment-collections/{id}/refunds`;
- `POST /admin/refunds/{id}/complete`;
- `POST /admin/refunds/{id}/cancel`.

Together with the preceding admin Payment read and collection-command cutovers, the mounted admin Payment REST surface no longer needs concrete Payment service/orchestration construction for these routes. Other Commerce Payment workflows and the broader Product/Order/Fulfillment/GraphQL topology remain separate work.

## Payment owner capability

`rustok-payment` publishes `PaymentAdminRefundCommandPort` and `PaymentAdminRefundCommandRuntime`. The built-in owner adapter owns:

- refund creation reservation and replay through `PaymentRefundCreationService`;
- caller creation-key validation at the durable Payment reservation boundary;
- collection lifecycle admission before a new/replayed provider refund attempt;
- provider selection from the host-selected `PaymentProviderRegistry`;
- `PaymentProviderOperationJournal` access;
- provider refund execution;
- durable provider-result adoption;
- provider-payment identity recovery from the authorize journal;
- provider success/error/reconciliation checkpointing;
- refund completion and cancellation through Payment-owned lifecycle persistence.

`PaymentService`, `PaymentRefundCreationService`, provider journal construction, and provider execution remain inside `rustok-payment` for this mounted path.

## Two idempotency identities remain distinct

Refund creation intentionally retains two different durable identities.

The caller-supplied `Idempotency-Key` remains the refund creation identity passed to `PaymentRefundCreationService::create_or_replay`. It protects reservation identity and request-hash replay. The HTTP adapter also carries that same stable caller key in `PortContext` so the generic write-port admission contract does not replace or weaken the owner creation identity.

After a refund has an owner-generated `refund_id`, provider execution continues to use the existing provider journal key:

`payment_refund:{refund_id}`

These identities are not interchangeable: the first identifies the caller-visible refund reservation, while the second identifies the external provider refund attempt for that reserved refund.

## Provider request compatibility

The owner command preserves the pre-cutover provider request structure:

- operation is `refund`;
- `refund_id` remains recorded both in the provider journal relation and provider metadata;
- `commerce_orchestration.operation` remains `create_refund`;
- the reserved `refund_id` and requested reason remain in the `commerce_orchestration` metadata;
- non-manual providers still recover `provider_payment_id` from the durable authorize operation when it is not already present;
- the authorize lookup key remains `payment_collection:{collection_id}:authorize`.

A retry after upgrade therefore targets the same refund reservation and provider journal rows rather than introducing a new provider identity.

## Reserved-refund reconciliation semantics

A provider failure can happen after the refund reservation has already been durably created. The owner port therefore retains distinct bounded outcomes for the two public cases that previously had refund-specific envelopes:

- unknown/invalid provider outcome after reservation -> `commerce_admin_refund_reconciliation_required` / HTTP 409;
- provider unavailable after reservation -> `commerce_admin_refund_provider_unavailable` / HTTP 503.

Other provider/payment failures retain the existing admin Payment public families. Provider or database error text is not exposed through the new public port errors; owner diagnostics retain structural identity/failure facts instead.

## Complete and cancel

Refund completion and cancellation remain Payment-owned lifecycle writes. They do not introduce a new provider operation in this cutover; the owner command delegates to the existing Payment lifecycle methods exactly as the previous Commerce orchestration wrapper did.

The transport supplies transition-scoped write-admission identities for complete/cancel. No new durable command-receipt or lost-response replay guarantee is claimed for those transition keys.

## Transport compatibility

The mounted handlers preserve:

- `PAYMENTS_UPDATE` admission;
- the required `Idempotency-Key` header and 191-byte maximum for create-refund;
- the existing `CreateRefundInput`, `CompleteRefundInput`, and `CancelRefundInput` request bodies;
- HTTP 201 for create/replay and HTTP 200 for complete/cancel;
- `RefundResponse` payloads;
- validation/not-found/state-conflict/provider-unavailable/provider-invalid-response/reconciliation-required/provider-not-configured/storage-unavailable families;
- tenant, authenticated user actor, locale, optional channel, and bounded deadline in `PortContext`.

## Runtime composition

`CommerceHttpRuntime` first accepts a host-selected `PaymentAdminRefundCommandRuntime`. If no external adapter was supplied, the built-in Payment owner adapter is composed with the host database and the same host-selected `PaymentProviderRegistry` already used by the admin collection command path.

External adapters therefore retain precedence while the built-in path remains provider-consistent with the host runtime.

## Remaining topology work

The canonical broad item “Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and Fulfillment concrete services behind host-composed owner ports” remains open.

Admin Payment REST is substantially cut over, but other mounted Commerce workflows still construct or own concrete Payment/Fulfillment orchestration, including post-order/change/return flows and remaining GraphQL/provider-operation surfaces. Fulfillment admin lifecycle construction is also a natural next focused owner-boundary slice.

## Validation status

`scripts/verify/verify-commerce-admin-refund-owner-command-cutover.mjs` is added as a source guard but is intentionally not executed here.

No tests, Cargo commands, formatting, verifier execution, workflows, CI, runtime HTTP calls, restart evidence, lost-response evidence, or external-provider execution evidence are claimed in this slice.
