# Implementation plan for `rustok-payment`

## Current state

`rustok-payment` owns payment collections, payments, refunds, provider SPI
policy, payment lifecycle, signature-verified webhook ingress, durable provider
event inbox state, bounded retry recovery, and operator dead-letter replay. It
references cart, order, and customer by identifier only. `rustok-commerce`
composes checkout; it must not reintroduce a payment-service facade, payment
storefront aggregation, provider-payload parsing, or lifecycle persistence in an
external adapter.

The storefront package owns collection creation/reuse, collection reads, and
refund summaries. Native endpoints use `HostRuntimeContext` and a shared
transactional event bus; GraphQL remains the selected fallback path. Provider
registry checks capability, health, unavailable mode, and degraded fallback
before invoking an adapter, while `PaymentService` remains the lifecycle owner.

Provider webhook source implementation now includes:

- `POST /payment/webhooks/{provider_id}` mounted through module codegen;
- SHA-256-only raw-payload audit with no raw body or signature persistence;
- atomic verified normalized facts plus inbox receipt;
- immutable normalized audit facts;
- delivery/idempotency deduplication and processing leases;
- payment/refund owner appliers;
- bounded background recovery for `received`, `failed`, and expired
  `processing` rows;
- safe operator read/list/recovery routes;
- manual `dead_letter -> processing` replay with `payments:manage`.

The background recovery worker is part of the standard `mod-payment` server
profile and is disabled when the runtime profile does not run background
workers.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract:
  `payment.checkout.v1+provider_spi.v1+provider_webhook_inbox.v1` in
  `crates/rustok-payment/contracts/payment-fba-registry.json`.
- Webhook contract and runbook:
  `crates/rustok-payment/contracts/payment-provider-webhook-v1.json` and
  `crates/rustok-payment/docs/provider-webhooks.md`.
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

Source implementation and source tests do not promote the module beyond
`boundary_ready`. No compile, migration, HTTP, scheduler, PostgreSQL concurrency,
or external-provider signature evidence was executed in this change session.

## Open results

1. **Wire a production gateway adapter through the provider registry.** Add
   concrete gateway configuration and cryptographic signature verification only
   through the guarded provider seam; adapters must not persist payment state or
   bypass capability, unavailable-mode, idempotency, or inbox rules.
   **Depends on:** approved gateway credentials, webhook endpoint configuration,
   provider SDK selection, and deployment-owned secret management.
   **Done when:** a production-like gateway exercises authorize, capture,
   cancel, refund, duplicate delivery, out-of-order capture, retry recovery, and
   dead-letter replay with `PaymentService` as the sole lifecycle owner.

2. **Execute migration and concurrency evidence.** Run the new inbox/replay
   migrations and targeted tests against SQLite and PostgreSQL, then exercise
   concurrent claim, expired lease, duplicate delivery, payload collision,
   normalized-fact immutability, and worker restart scenarios.
   **Depends on:** a build environment with repository dependencies and
   PostgreSQL test infrastructure.
   **Done when:** migration up/down, owner lifecycle, recovery worker, and HTTP
   replay evidence is retained without relying on static source inspection.

3. **Complete base checkout-port remote evidence.** Execute create/reuse and
   read-status contracts through a remote adapter and validate timeout, typed
   error, fallback, and cart-ownership behavior before promoting beyond
   `boundary_ready`.
   **Depends on:** a checkout consumer and remote adapter environment.
   **Done when:** executable evidence covers the published payment checkout
   profiles, including create/reuse parity with storefront GraphQL and native
   paths.

4. **Keep payment and commerce contracts aligned.** Synchronize module docs,
   metadata, storefront evidence, verifier policy, and umbrella checkout
   documentation with any provider or checkout surface change.
   **Depends on:** the change-owning payment or commerce contract.
   **Done when:** public transport, provider policy, payload-audit policy, and
   operational recovery guidance name the same owner and fallback behavior.

## Verification

- `npm run verify:payment:storefront-boundary`
- `npm run verify:ecommerce:fba`
- `npm run verify:ecommerce:provider-spi-evidence`
- `cargo xtask module validate payment`
- `cargo xtask module test payment`
- `cargo check -p rustok-payment --all-features`
- `cargo check -p rustok-server --features mod-payment`
- Targeted provider inbox, lifecycle, replay, immutability, recovery-worker, and
  checkout/reconciliation tests.

## Change rules

1. Keep payment lifecycle, provider policy, provider-event persistence, and
   replay policy in this module.
2. Never persist or expose raw provider payloads, signatures, SQL messages, or
   provider SDK errors.
3. Update local documentation, `rustok-module.toml`, and the umbrella commerce
   plan with a payment/provider or checkout contract change.
4. Update this status block and `docs/modules/registry.md` with any FFA/FBA
   boundary change.
