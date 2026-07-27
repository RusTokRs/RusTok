# Payment checkout compensation local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice closes stable local outcome context retention for the canonical root payment compensation
construction used by mounted checkout compensation:

- `CheckoutPaymentCompensationPort::compensate_checkout_payment`;
- root `InProcessCheckoutPaymentCompensationPort`;
- root `in_process_checkout_payment_compensation_port`.

The existing persistent implementation in `checkout_compensation.rs` remains unchanged. A new root
wrapper retains the delegated `PortContext` and safe request facts, calls the original owner, classifies
only exact stable returned `PortError` envelopes, and returns the same error unchanged.

## Canonical root cutover

The crate keeps `pub mod checkout_compensation`, so the existing module-path contracts, implementation
type, and legacy factory remain available for compatibility. Root exports now separate contracts from
construction:

- `CheckoutPaymentCompensationPort` and `CheckoutPaymentCompensationRequest` continue to come from the
  original module;
- root `InProcessCheckoutPaymentCompensationPort` and
  `in_process_checkout_payment_compensation_port` come from the context wrapper.

The mounted commerce compensation service imports the root names, so its default factory and
provider-registry constructor now use the wrapper without changing commerce source or public types.
Direct callers that deliberately construct through `rustok_payment::checkout_compensation` remain an
explicit bypass and are documented as remaining work.

## Delegation order

The wrapper performs no new admission or lifecycle policy. Its source order is:

1. clone the incoming `PortContext` for diagnostics;
2. retain safe request facts;
3. delegate the original context and request to the unchanged persistent owner;
4. inspect only a returned `PortError`;
5. emit a local event only when the exact stable code and message are covered;
6. return the same `PortError` unchanged.

The persistent owner continues to own write policy, write semantics, tenant parsing, checkout-operation
causation, optional-collection no-op behavior, identity validation, lifecycle admission, provider
journal recovery, provider cancellation, local cancellation, and commit checkpointing.

## Retained request facts

Covered diagnostics retain:

- checkout operation id;
- optional payment collection id;
- optional compensation-reason character length;
- metadata JSON kind;
- object-field or array-element count when applicable.

The raw compensation reason and metadata value are deliberately not recorded. Metadata can contain
arbitrary caller and provider data, and reason is an unvalidated string before owner normalization.
Shape and size evidence preserves useful request context without copying payload content into logs.

## Covered stable outcomes

The mapper requires exact `code + message` pairs. Code-only matching is intentionally forbidden.

| Stable envelope | Local operation | Severity |
| --- | --- | --- |
| `payment.checkout_compensation_identity_invalid` / `checkout operation and payment collection identity must be non-nil UUIDs` | `validate_compensation_identity` | warning |
| `payment.collection_not_found` / `payment collection was not found` | `load_collection` | warning |
| `payment.checkout_compensation_manual_reconciliation` / `payment checkout compensation requires manual reconciliation` | `require_manual_reconciliation` | error |
| `payment.checkout_compensation_state_conflict` / `payment collection changed while compensation was being applied` | `apply_compensation_state` | warning |
| `payment.checkout_compensation_state_conflict` / `payment lifecycle conflicts with checkout compensation` | `apply_payment_lifecycle` | warning |
| `payment.checkout_compensation_provider_state_conflict` / `payment provider cancellation is in an unsupported state` | `validate_provider_journal_state` | error |
| `payment.checkout_compensation_metadata_invalid` / `payment compensation metadata must be a JSON object` | `validate_provider_metadata` | warning |
| `payment.checkout_compensation_provider_identity_conflict` / `payment provider identity conflicts with the durable authorization` | `validate_provider_identity` | warning |
| `payment.checkout_compensation_encoding_failed` / `payment compensation request could not be encoded` | `encode_provider_cancel_request` | error |
| `payment.database_unavailable` / `payment storage is temporarily unavailable` | `owner_storage` | error |
| `payment.checkout_compensation_validation` / `payment compensation request is invalid` | `validate_owner_request` | warning |
| `payment.payment_not_found` / `payment was not found` | `load_payment` | warning |
| `payment.refund_not_found` / `refund was not found` | `load_refund` | warning |
| `payment.provider_unavailable` / `payment provider is temporarily unavailable` | `execute_provider_cancel` | error |
| `payment.provider_rejected` / `payment provider rejected the requested operation` | `execute_provider_cancel` | warning |
| `payment.provider_invalid_response` / `payment provider response could not be applied safely` | `normalize_provider_result` | error |
| `payment.provider_not_configured` / `payment provider is not configured` | `resolve_provider` | error |

Unavailable, timeout, and invariant kinds use error severity. Manual reconciliation and an unsupported
durable provider-journal state also use error severity because they require operator or integrity
attention despite being represented by conflict errors. Other validation, not-found, rejection,
identity, lifecycle, and concurrent-state conflicts use warning severity.

## Retained diagnostic context

Covered outcomes record:

- truthful owner `rustok_payment`;
- public operation `compensate_checkout_payment`;
- operation-specific local label;
- boundary `checkout_payment_compensation_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- safe request facts described above;
- exact stable code and message;
- typed error kind and retryability;
- the complete delegated `PortError`.

## Pass-through behavior

The wrapper does not classify:

- policy rejection;
- missing write semantics;
- malformed tenant context;
- checkout-operation causation mismatch;
- any unrecognized code or message;
- successful no-op compensation with no collection id;
- successful already-cancelled recovery;
- successful provider-journal replay or adoption;
- successful provider and local cancellation.

Pre-delegation owner diagnostics therefore remain the sole events for policy, tenant, and causation
rejections. Successful and unknown outcomes do not receive an additional local event.

## Preserved owner behavior

This work does not change:

- compensation request or response DTOs;
- public codes, messages, kinds, or retryability;
- write policy, write semantics, tenant, or causation validation;
- optional missing-collection no-op behavior;
- captured-payment refund-policy reconciliation;
- payment collection lifecycle classification;
- provider selection or manual-provider fallback;
- canonical `payment_collection:{collection_id}:cancel` idempotency key;
- provider request metadata and `cancel_payment_collection` operation marker;
- provider journal begin, claim, replay, error, success, and reconciliation checkpoints;
- provider payment identity recovery;
- local cancellation race handling;
- final provider-operation commit checkpoint;
- commerce payment-before-order-before-inventory-before-cart compensation ordering;
- provider-registry constructor behavior.

The implementation in `checkout_compensation.rs` is unchanged.

## Static evidence

`scripts/verify/verify-payment-checkout-compensation-local-context.mjs` guards:

- legacy module compatibility plus canonical root type/factory cutover;
- wrapper constructor delegation for default and provider-registry construction;
- context and safe-fact retention before unchanged owner delegation;
- post-delegation-only mapping and same delegated error return;
- typed identifiers plus length/shape-only payload evidence;
- absence of raw reason or metadata fields in diagnostics;
- exact stable code-and-message classification;
- technical/integrity versus ordinary severity;
- complete `PortContext`, request-fact, and `PortError` fields;
- pass-through of tenant and causation envelopes;
- unchanged persistent policy, provider idempotency key, journal, cancellation, and checkpoint markers;
- mounted commerce use of root contracts, wrapper type, and wrapper factory.

## Remaining gaps

The ecommerce correlation-safe mapper task remains open for:

- direct callers that deliberately bypass the root wrapper through
  `rustok_payment::checkout_compensation`;
- payment execution and compensation owner policy/tenant/causation diagnostics beyond existing owner
  events;
- direct payment GraphQL and HTTP query/mutation envelopes;
- GraphQL customer reads and shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, provider replay, restart, remote-port, and cross-transport runtime evidence.

No FBA or FFA status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-checkout-compensation-local-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-payment-order-context.mjs
node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
cargo check -p rustok-commerce --lib
```
