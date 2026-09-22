# Commerce admin Fulfillment owner-command cutover — 2026-08-09

Status: source-complete for mounted admin Fulfillment ship/deliver/reopen/reship/cancel routes; execution evidence pending and unvalidated.

## Scope

This slice advances the canonical ecommerce topology P0 without closing it. Five mounted admin Fulfillment lifecycle routes now call a Fulfillment-owned command capability:

- `POST /admin/fulfillments/{id}/ship`;
- `POST /admin/fulfillments/{id}/deliver`;
- `POST /admin/fulfillments/{id}/reopen`;
- `POST /admin/fulfillments/{id}/reship`;
- `POST /admin/fulfillments/{id}/cancel`.

`POST /admin/fulfillments` is intentionally out of scope. Manual fulfillment creation is a cross-owner Commerce workflow: it validates Order ownership and remaining quantities, applies seller-aware shipping-profile grouping, persists the Fulfillment, and then creates a provider label. Moving that entire workflow into `rustok-fulfillment` would incorrectly make Fulfillment own Order/Commerce policy.

## Fulfillment owner capability

`rustok-fulfillment` now publishes `FulfillmentAdminCommandPort` and `FulfillmentAdminCommandRuntime` with typed ship, deliver, reopen, reship, and cancel commands. The built-in adapter owns:

- `FulfillmentService` construction and lifecycle persistence;
- shipping-option provider resolution;
- `FulfillmentProviderOperationJournal` construction and replay adoption;
- provider execution through the host-selected `FulfillmentProviderRegistry` for ship/reship/cancel;
- provider-result metadata adoption;
- reconciliation checkpointing when a provider side effect succeeded but local persistence failed.

Mounted Commerce HTTP does not construct `FulfillmentService` or `FulfillmentOrchestrationService` for these five routes.

## Provider replay compatibility

The owner command adapter preserves the pre-cutover immutable provider request payload and exact payload-sensitive key algorithm:

`fulfillment:{fulfillment_id}:{operation}:{fnv128}`

The two FNV-1a 64-bit passes retain the existing offset bases and concatenate into the same 32-hex-character suffix. Provider request metadata also remains compatible:

- ship: `commerce_orchestration.operation = ship` plus carrier, tracking number, and item adjustments;
- reship: `commerce_orchestration.operation = reship` plus carrier, tracking number, and item adjustments;
- cancel: `commerce_orchestration.operation = cancel` plus the requested reason.

The existing local `provider_operation` metadata is also retained when provider results are committed to Fulfillment state.

## Lifecycle replay compatibility

The owner path retains two compatibility behaviors from the previous Commerce facade:

- a completed reship replay returns the already-shipped Fulfillment when `metadata.provider_operation.operation == "reship"`;
- cancelling an already-cancelled Fulfillment returns the current projection instead of re-running the provider operation.

Ship continues to adopt already-committed provider journal state. Provider operations already marked `reconciliation_required` remain a validation-class replay result, matching the pre-cutover mounted HTTP family.

## Reconciliation semantics

When ship/reship/cancel provider execution succeeds but the subsequent local Fulfillment persistence fails, the owner adapter marks the provider operation `reconciliation_required` and returns the existing public reconciliation family:

- HTTP 409;
- `commerce_admin_fulfillment_reconciliation_required`;
- `Fulfillment operation requires reconciliation`.

Ordinary validation, not-found, invalid-transition, and storage failures keep their existing public HTTP families. Internal provider/database text is not copied into owner `PortError` messages.

## Port write identity

Mounted HTTP supplies tenant, authenticated user actor, locale, optional channel, a two-second deadline, and a stable transition-scoped admission identity:

`admin-fulfillment:{fulfillment_id}:{operation}`

This identity satisfies the generic write-port admission contract. It is not a newly claimed durable command receipt. Durable provider replay for ship/reship/cancel remains the Fulfillment-owned payload-sensitive provider journal described above. Deliver/reopen do not gain a durable replay receipt in this slice.

## Runtime composition

`CommerceHttpRuntime` prefers a host-injected `FulfillmentAdminCommandRuntime`. When none is supplied, it composes the public in-process Fulfillment owner runtime using the same host-selected `FulfillmentProviderRegistry` already used by Commerce provider integration.

This preserves external adapter precedence without constructing concrete Fulfillment services in the mounted command adapter.

## Mounted compatibility strategy

The existing `controllers/admin/fulfillments.rs` remains a private compatibility module. `fulfillments_owner_commands.rs` wildcard-reexports its unaffected list/show/create and generated Utoipa compatibility symbols, while local definitions shadow the five mounted lifecycle commands.

This follows the already-established Payment cutover pattern and avoids changing public route/OpenAPI symbol names.

## Remaining topology work

The canonical broad item “Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and Fulfillment concrete services behind host-composed owner ports” remains open.

The next Fulfillment-specific boundary is manual admin fulfillment creation. That work should keep Order and seller/shipping-profile policy in Commerce while replacing cross-owner concrete access with typed owner capabilities and keeping create-label durability/recovery intact.

Post-order/change/return workflows and remaining GraphQL/provider-operation construction also keep the broad P0 open.

## Validation status

`scripts/verify/verify-commerce-admin-fulfillment-owner-command-cutover.mjs` is added as a source guard but is intentionally not executed here.

No tests, Cargo commands, formatting, verifier execution, workflows, CI, runtime HTTP calls, restart/lost-response evidence, or external-provider execution evidence are claimed in this slice.
