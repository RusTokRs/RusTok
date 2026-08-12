# Commerce GraphQL return-decision owner-port cutover

Status: `source_complete_validation_pending`

## Scope

This bounded ecommerce topology slice moves the mounted GraphQL `createOrderReturnDecision` mutation off the legacy `PostOrderOrchestrationService` compatibility path and onto the same owner-backed return-decision orchestration used by mounted REST.

The GraphQL boundary composes `ReturnDecisionOwnerOrchestrationService` from the host-selected `OrderPostOrderCommandPort` and `PaymentAdminReadPort`. Directly embedded compatibility schemas retain explicit in-process owner-runtime fallbacks; mounted schemas consume host composition.

## Preserved behavior

- `orders:update` remains required for the mutation.
- `payments:update` remains conditionally required for refund decisions.
- Return-only, refund, exchange, and claim decision semantics are unchanged.
- Captured payment-collection discovery continues through `list_payment_collection_projections` with the legacy-compatible order/status filter.
- Actual refund/provider execution remains inside `PaymentOrchestrationService` with the host-selected provider registry.
- The public GraphQL schema and response shape are unchanged.

## Error boundary

Order command `PortError` values are mapped through the existing bounded post-order owner GraphQL mapper. Payment admin-read `PortError` values receive a dedicated bounded GraphQL mapper that logs only typed kind/code-length/correlation metadata and never the raw owner message.

Legacy `PostOrderOrchestrationError` remains the compatibility branch for validation and Payment refund/provider execution failures.

## Remaining topology work

The broad ecommerce topology P0 remains open. Return completion and other remaining mounted Commerce REST/GraphQL concrete Product, Order, Payment, and Fulfillment construction must still move behind host-composed owner ports in separate bounded slices.
