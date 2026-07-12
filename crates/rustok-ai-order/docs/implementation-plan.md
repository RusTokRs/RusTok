# rustok-ai-order implementation plan

## Current state

`rustok-ai-order` owns descriptors and generated-payload validation for
`order_analytics` and the sensitive `order_ops_assistant` flow. `rustok-ai`
consumes its registration API; order status remains owned by `rustok-order`
through `CheckoutCompletionPort`. The crate does not directly depend on an
order runtime or execute provider calls.

## FFA/FBA readiness

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Structural shape: `core_transport_ui`
- `CheckoutCompletionPort` / `order.checkout_completion.v1` supports
  `read_order_status`. Degraded behavior is
  `generate_summary_without_live_status`, `require_operator_review`, and
  `skip_prefill_execution`.
- Evidence: `crates/rustok-ai-order/contracts/ai-order-fba-registry.json`,
  `crates/rustok-ai-order/contracts/evidence/ai-order-consumer-static-matrix.json`,
  `crates/rustok-ai-order/contracts/evidence/ai-order-runtime-fallback-smoke.json`,
  and `scripts/verify/verify-ai-fba-baseline.mjs`.

## Next results

1. **Exercise the composed order-status boundary.** Add a direct-runtime test
   that consumes `CheckoutCompletionPort`, preserves the request deadline and
   typed error mapping, and proves each declared degraded behavior. Done when
   `rustok-ai` and the support adapter are covered together.
2. **Render the owner-admin package in its hosts.** Connect its existing
   core/selected-transport/UI layers to the admin route and prove native
   server-function selection with parallel GraphQL/headless parity. Done when
   host-level evidence covers both transport paths.
3. **Keep AI output advisory until a product workflow approves execution.** A
   new action must remain review-gated and have a structured prefill contract
   before it can leave the operator flow. Done when no generated
   `order_ops_assistant` output can invoke an order mutation implicitly.

## Verification

- `npm run verify:ai-order:fba`
- `npm run verify:ai:domain-verticals`
- `cargo test -p rustok-ai-order --lib`

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [AI order FBA registry](../contracts/ai-order-fba-registry.json)
