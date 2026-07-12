# Implementation plan for `rustok-payment`

## Current state

`rustok-payment` owns payment collections, payments, provider SPI policy, and
payment lifecycle. It references cart, order, and customer by identifier only.
`rustok-commerce` composes checkout; it must not reintroduce a payment-service
facade, payment storefront aggregation, or lifecycle persistence in an external
adapter.

The storefront package owns collection creation/reuse, collection reads, and
refund summaries. Native endpoints use `HostRuntimeContext` and a shared
transactional event bus; GraphQL remains the selected fallback path. Provider
registry checks capability, health, unavailable mode, and degraded fallback
before invoking an adapter, while `PaymentService` remains the lifecycle owner.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `payment.checkout.v1` in
  `crates/rustok-payment/contracts/payment-fba-registry.json`.
- Contract and provider evidence:
  `crates/rustok-payment/contracts/evidence/payment-contract-test-static-matrix.json`,
  `crates/rustok-payment/contracts/evidence/payment-provider-spi-static-matrix.json`,
  `crates/rustok-payment/contracts/evidence/payment-provider-spi-runtime-smoke.json`,
  and `crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-evidence.json`.
- `scripts/verify/verify-payment-storefront-boundary.mjs` locks the owner
  storefront core/transport/UI split and host-neutral native checkout runtime.
- Owner transport uses `execute_selected_transport`: native `#[server]` is
  selected first and GraphQL is the parallel fallback for
  `create_payment_collection`, `fetch_payment_collection`, and
  `fetch_refund_summary`.

## Open results

1. **Wire production gateway adapters through the provider registry.** Add
   concrete gateway configuration and operational invocation only through the
   guarded provider seam; adapters must not persist payment state or bypass
   capability, unavailable-mode, and idempotency checks.
   **Depends on:** approved gateway credentials, webhook ingress configuration,
   and deployment-owned secret management.
   **Done when:** a production-like gateway exercises authorize, capture,
   cancel, refund, and replay-safe webhook delivery with `PaymentService` as the
   sole lifecycle owner.

2. **Complete base checkout-port remote evidence.** Execute create/reuse and
   read-status contracts through a remote adapter and validate timeout, typed
   error, fallback, and cart-ownership behavior before promoting beyond
   `boundary_ready`.
   **Depends on:** a checkout consumer and remote adapter environment.
   **Done when:** executable evidence covers the published `payment.checkout.v1`
   profiles, including create/reuse parity with storefront GraphQL and native
   paths.

3. **Keep payment and commerce contracts aligned.** Synchronize module docs,
   metadata, storefront evidence, and umbrella checkout documentation with any
   provider or checkout surface change.
   **Depends on:** the change-owning payment or commerce contract.
   **Done when:** public transport, provider policy, and operational recovery
   guidance name the same owner and fallback behavior.

## Verification

- `npm run verify:payment:storefront-boundary`
- `npm run verify:ecommerce:fba`
- `npm run verify:ecommerce:provider-spi-evidence`
- `cargo xtask module validate payment`
- `cargo xtask module test payment`
- Targeted payment-collection lifecycle, provider SPI, and webhook replay tests.

## Change rules

1. Keep payment lifecycle and provider policy in this module.
2. Update local documentation, `rustok-module.toml`, and the umbrella commerce
   plan with a payment/provider or checkout contract change.
3. Update this status block and `docs/modules/registry.md` with any FFA/FBA
   boundary change.
