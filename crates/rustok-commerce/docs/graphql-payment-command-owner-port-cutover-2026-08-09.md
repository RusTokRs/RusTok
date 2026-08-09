# Commerce GraphQL Payment command owner-port cutover

Status: `source_complete_unvalidated`

Date: 2026-08-09

## Scope

This source slice moves the mounted Commerce GraphQL Payment provider mutations behind existing Payment-owned typed command capabilities:

- `authorizePaymentCollection`;
- `capturePaymentCollection`;
- `cancelPaymentCollection`;
- `createRefund`;
- `completeRefund`;
- `cancelRefund`.

The GraphQL field names, arguments, `PAYMENTS_UPDATE` permission checks, successful DTOs, and public error-code families remain unchanged. Fulfillment mutations in the same provider-operation object are intentionally out of scope for this slice.

## Owner capabilities

Commerce GraphQL now delegates collection transitions to `PaymentAdminCollectionCommandPort` and refund mutations to `PaymentAdminRefundCommandPort`. These are existing Payment owner capabilities already used by mounted admin HTTP routes.

`CommercePaymentCommandRuntime` composes the two owner runtimes for GraphQL. Schema composition prefers a host-shared composite runtime. If one is not present, it independently prefers host-shared `PaymentAdminCollectionCommandRuntime` and `PaymentAdminRefundCommandRuntime` values. Missing capabilities use Payment-owned in-process adapters with the deployment-selected `PaymentProviderRegistry`.

Mounted GraphQL therefore no longer constructs `PaymentOrchestrationService` for these six mutations. Payment persistence, provider journals, replay decisions, and provider execution remain inside `rustok-payment`.

## Request-owned write context

Each mutation passes a typed `PortContext` containing:

- the requested tenant id;
- the authenticated GraphQL user as `PortActor::user`;
- request locale when available, otherwise `und`;
- request channel when available;
- a stable GraphQL correlation identity;
- a two-second owner-call deadline;
- mandatory write-admission idempotency identity.

Collection transition admission identities are transport-local:

```text
graphql-payment-collection:{collection_id}:authorize_payment_collection
graphql-payment-collection:{collection_id}:capture_payment_collection
graphql-payment-collection:{collection_id}:cancel_payment_collection
```

Refund transition admission identities are likewise transport-local:

```text
graphql-refund:{refund_id}:complete_refund
graphql-refund:{refund_id}:cancel_refund
```

These `PortContext` identities are generic owner write-admission identity only. They are not described as new durable receipts.

## Durable Payment replay semantics

The owner command implementations preserve the already-established durable identities:

```text
payment_collection:{collection_id}:authorize
payment_collection:{collection_id}:capture
payment_collection:{collection_id}:cancel
payment_refund:{refund_id}
```

The GraphQL `createRefund` caller-provided `idempotencyKey` is passed without transformation both as the owner write-admission idempotency identity and as `CreateAdminRefundRequest.creation_key`. Payment continues to call `PaymentRefundCreationService::create_or_replay` with that exact creation identity.

Provider metadata and journal recovery semantics remain owned by the same Payment command adapters used by admin HTTP. This slice does not add a second GraphQL-specific provider journal or receipt layer.

## GraphQL error compatibility

Payment owner `PortError` values are mapped back to the existing bounded GraphQL families:

- validation -> `PAYMENT_REQUEST_INVALID`;
- not found -> `PAYMENT_RESOURCE_NOT_FOUND`;
- invalid transition/provider rejection -> `PAYMENT_STATE_CONFLICT`;
- provider/database unavailable -> `PAYMENT_TEMPORARILY_UNAVAILABLE` with `retryable = true`;
- invalid/unknown provider outcome and reserved-refund reconciliation -> `PAYMENT_RECONCILIATION_REQUIRED`;
- provider configuration -> `PAYMENT_CONFIGURATION_ERROR`.

The reserved-refund provider-unavailable owner code remains in the temporarily-unavailable GraphQL family, matching the previous GraphQL provider-error behavior after refund reservation.

Commerce logs only bounded owner kind/code length, stable operation/correlation facts, and public classification. It does not log the complete `PortError.message`, provider identifiers, provider payloads, or arbitrary owner code text.

## Still open

The broad canonical topology item remains open. At minimum, Commerce GraphQL Fulfillment provider mutations in the same file still use `FulfillmentOrchestrationService`, and additional post-order/change/return, checkout/reconciliation, or other mounted orchestration paths may still require owner-boundary work.

No FFA/FBA or broad topology status is promoted by this source slice.

## Intended checks

The focused source guard added with this slice is:

```bash
node scripts/verify/verify-commerce-graphql-payment-command-owner-port-cutover.mjs
```

Relevant compile/runtime checks remain for the maintainer to run separately.

No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, CI, runtime calls, provider execution, lost-response scenarios, restart scenarios, or remote-adapter scenarios were executed for this slice.
