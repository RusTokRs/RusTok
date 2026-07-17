# rustok-ai-order implementation plan

## Current state

`rustok-ai-order` owns descriptors and generated-payload validation for
`order_analytics` and the sensitive `order_ops_assistant` flow. `rustok-ai`
consumes its registration API; order status remains owned by `rustok-order`
through `CheckoutCompletionPort`. The crate does not directly depend on an
order runtime or execute provider calls.

## Completed direct-execution evidence

Both order verticals declare a domain-owned advisory execution policy:
`review_required: true` and `persistence: none`. The composed `rustok-ai`
direct-runtime tests prove that `order_analytics` and `order_ops_assistant`
return generated output without writing order data. The operations assistant
remains marked sensitive; any future owner-owned order action must consume a
reviewed, explicit command rather than generated output.

The same direct path consumes order status only through the host-composed
`CheckoutCompletionPort`. It enforces a three-second read timeout and maps
owner `PortError` values into structured degraded context. A missing, slow, or
unavailable port keeps generation advisory, requires review, and skips any
prefill execution; it never falls back to an order service or storage access.

## FFA/FBA readiness

- FFA status: `not_started` (no standalone support-adapter UI).
- FBA status: `boundary_ready` (`no_ui_boundary`).
- Structural shape: `no_ui_boundary`
- Adapter controls are composed by the `rustok-ai` owner Leptos and Next.js
  admin surfaces; this support crate must not expose an order-owned route.
- `CheckoutCompletionPort` / `order.checkout_completion.v1` supports
  `read_order_status`. Degraded behavior is
  `generate_summary_without_live_status`, `require_operator_review`, and
  `skip_prefill_execution`.
- Evidence: `crates/rustok-ai-order/contracts/ai-order-fba-registry.json`,
  `crates/rustok-ai-order/contracts/evidence/ai-order-consumer-static-matrix.json`,
  `crates/rustok-ai-order/contracts/evidence/ai-order-runtime-fallback-smoke.json`,
  and `scripts/verify/verify-ai-fba-baseline.mjs`.

## Next results

1. **Maintain the owner-port execution boundary.** The direct runtime proves
   that status reads use `CheckoutCompletionPort`, while generated output never
   writes order data. Any future action must remain review-gated and use a
   structured prefill contract before an owner workflow can apply it.

## Verification

- `npm run verify:ai-order:fba`
- `npm run verify:ai:domain-verticals`
- `cargo test -p rustok-ai-order --lib`
- `cargo test -p rustok-ai --features server direct::tests::direct_order_ --lib`

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [AI order FBA registry](../contracts/ai-order-fba-registry.json)
