# Payment checkout execution local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice extends the canonical `InProcessCheckoutPaymentExecutionPort` entrypoint with
post-delegation local outcome diagnostics for:

- `CheckoutPaymentExecutionPort::prepare_checkout_collection`;
- `CheckoutPaymentExecutionPort::authorize_checkout_collection`;
- `CheckoutPaymentExecutionPort::capture_checkout_collection`;
- `CheckoutPaymentExecutionPort::read_checkout_collection`.

The existing commerce checkout stage already retains complete context when the payment boundary
returns a `PortError`. This work closes the corresponding payment owner-side local outcome gap after
policy, tenant, and checkout-operation causation checks have accepted the call.

## Delegation order

Each operation preserves its existing admission contract:

1. require the existing read or write policy;
2. require write semantics for prepare, authorize, and capture only;
3. parse the payment tenant context;
4. require the checkout-operation causation identity;
5. retain the accepted `PortContext` and safe request facts;
6. delegate to the unchanged private payment execution method;
7. classify only a returned stable `PortError` envelope;
8. return that same `PortError` unchanged.

The read operation now delegates through a private helper with the same source order it previously
contained inline:

1. validate checkout payment identity;
2. load the payment collection through `PaymentService`;
3. map the same owner error;
4. validate the collection against the checkout identity;
5. return the same collection response.

## Retained request facts

Covered diagnostics retain typed request identity:

- checkout operation id;
- cart id;
- order id;
- optional customer id;
- optional collection id;
- requested amount.

Potentially unvalidated caller strings are not recorded. The event retains only character counts for:

- currency code;
- order plan hash;
- requested provider id;
- provider payment id.

Request metadata is not recorded. This prevents malformed or oversized identity/provider strings and
arbitrary metadata from being copied into owner diagnostics before validation.

## Covered stable outcomes

The mapper requires exact `code + message` pairs. Code-only matching is intentionally forbidden.

### Checkout identity and collection integrity

| Stable envelope | Local operation |
| --- | --- |
| `payment.checkout_identity_invalid` / `checkout payment identity contains invalid UUID or amount fields` | `validate_checkout_identity` |
| `payment.checkout_currency_invalid` / `checkout payment currency must be a three-letter alphabetic code` | `validate_checkout_currency` |
| `payment.checkout_plan_hash_invalid` / `checkout payment order plan hash must be a 64-character hexadecimal value` | `validate_checkout_plan_hash` |
| `payment.checkout_collection_id_invalid` / `checkout payment collection identity must be a non-nil UUID` | `validate_collection_id` |
| `payment.checkout_collection_operation_conflict` / `payment collection belongs to another checkout operation` | `validate_collection_operation` |
| `payment.checkout_collection_plan_conflict` / `payment collection belongs to another checkout order plan` | `validate_collection_plan` |
| `payment.checkout_collection_identity_conflict` / `payment collection does not match the checkout identity` | `validate_collection_identity` |
| `payment.checkout_collection_identity_missing` / `payment collection has no checkout identity` | `require_collection_identity` |
| `payment.checkout_collection_identity_conflict` / `payment collection has mismatched checkout identity` | `validate_collection_identity` |

### Lifecycle and provider execution

| Stable envelope | Local operation |
| --- | --- |
| `payment.checkout_authorize_state_conflict` / `cancelled payment collection cannot be authorized` | `validate_authorize_lifecycle` |
| `payment.checkout_capture_state_conflict` / `payment collection lifecycle does not allow capture` | `validate_capture_lifecycle` |
| `payment.checkout_authorize_request_invalid` / `checkout payment authorization request is invalid` | `validate_authorize_request` |
| `payment.provider_metadata_invalid` / `payment provider metadata must be a JSON object` | `validate_provider_metadata` |
| `payment.provider_identity_conflict` / `payment provider identity conflicts with the durable authorize operation` | `validate_provider_identity` |
| `payment.provider_idempotency_key_required` / `payment provider operation requires an idempotency key` | `require_provider_idempotency_key` |
| `payment.provider_request_encoding_failed` / `payment provider request could not be encoded` | `encode_provider_request` |
| `payment.provider_operation_invalid` / `unsupported checkout payment provider operation` | `select_provider_operation` |
| `payment.provider_unavailable` / `payment provider is temporarily unavailable` | `execute_provider_operation` |
| `payment.provider_rejected` / `payment provider rejected the requested operation` | `execute_provider_operation` |
| `payment.provider_not_configured` / `payment provider is not configured` | `resolve_provider` |
| `payment.checkout_execution_manual_reconciliation` / `payment checkout execution requires manual reconciliation` | `require_manual_reconciliation` |

### Payment owner storage and lifecycle mapping

| Stable envelope | Local operation |
| --- | --- |
| `payment.database_unavailable` / `payment storage is temporarily unavailable` | `owner_storage` |
| `payment.checkout_execution_validation` / `checkout payment request is invalid` | `validate_owner_request` |
| `payment.collection_not_found` / `payment collection was not found` | `load_collection` |
| `payment.payment_not_found` / `payment was not found` | `load_payment` |
| `payment.refund_not_found` / `refund was not found` | `load_refund` |
| `payment.checkout_execution_state_conflict` / `payment lifecycle conflicts with checkout execution` | `apply_payment_lifecycle` |

## Severity

Unavailable, timeout, and invariant kinds use error severity. Two conflict envelopes also use error
severity because they identify durable integrity or unresolved external-effect conditions:

- missing checkout identity on a persisted payment collection;
- checkout execution manual reconciliation.

Ordinary validation, not-found, lifecycle conflict, provider rejection, and identity mismatch outcomes
use warning severity.

## Pass-through behavior

The local mapper does not handle:

- policy rejection;
- missing write semantics;
- malformed tenant context;
- checkout-operation causation mismatch;
- any unrecognized code or message;
- successful prepare, authorize, capture, or read responses;
- successful authorized/captured replay adoption;
- successful provider journal recovery or commit adoption.

Those outcomes preserve the preceding behavior without another local event.

## Preserved owner behavior

This work does not change:

- request or response DTOs;
- public codes, messages, kinds, or retryability;
- payment provider registry selection;
- manual provider fallback;
- provider authorize/capture request payloads;
- canonical provider idempotency keys;
- provider journal claim, checkpoint, replay, or reconciliation policy;
- collection creation/reuse and order attachment;
- typed payment collection lifecycle admission;
- local authorization or capture persistence;
- checkout collection identity validation;
- commerce stage checkpoints or boundary mapping;
- factory names or provider-registry constructor behavior.

The original private prepare, authorize, capture, journal, and provider helper implementations remain
unchanged. Only the read body moved into a private helper with the same operations and ordering.

## Static evidence

`scripts/verify/verify-payment-checkout-execution-local-context.mjs` guards:

- policy → tenant → causation → safe-fact retention → owner delegation → local mapping ordering;
- write semantics for prepare/authorize/capture and their absence on read;
- unchanged private prepare, authorize, and capture calls;
- preserved read validation/load/validation sequence;
- typed UUID/amount retention;
- length-only retention for unvalidated caller strings;
- absence of raw currency, plan hash, provider identity, provider payment identity, and metadata fields;
- exact stable code-and-message classification;
- operation-specific authorize and capture labels;
- technical/integrity versus ordinary severity;
- complete delegated `PortContext` fields;
- same delegated `PortError` return;
- unknown and pre-delegation error pass-through;
- unchanged stable envelope construction in the existing payment execution source modules.

Payment compensation uses the same correlation-safe pattern behind its canonical root factory. Its
separate contract and guard are documented in
[`checkout-compensation-local-context.md`](./checkout-compensation-local-context.md).

## Remaining gaps

The ecommerce correlation-safe mapper task remains open for:

- payment execution and compensation policy, tenant, and causation diagnostics beyond existing owner
  events;
- direct callers that bypass canonical payment root factories;
- direct payment query/mutation transport envelopes;
- GraphQL customer reads and shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, provider replay, restart, remote-port, and cross-transport runtime evidence.

No FBA or FFA status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-checkout-execution-local-context.mjs
node scripts/verify/verify-payment-checkout-compensation-local-context.mjs
node scripts/verify/verify-commerce-checkout-payment-stage-context.mjs
node scripts/verify/verify-payment-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
cargo check -p rustok-commerce --lib
```
