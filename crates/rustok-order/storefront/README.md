# rustok-order-storefront

Module-owned storefront UI package for `rustok-order`.

## Purpose

- Own storefront checkout result/order handoff presentation.
- Own complete-checkout request DTO construction for the storefront action handoff.
- Keep order status display policy outside umbrella `rustok-commerce`.

## Entry points

- `src/core.rs` — Leptos-free checkout result handoff view-model and action-label policy.
- `src/transport.rs` — framework-free complete-checkout request DTO and normalization facade used by host orchestration during the compatibility window.
- `src/ui/leptos.rs` — Leptos render adapter for the order checkout result handoff; action components emit order-owned request DTOs instead of raw cart ids.

## Interactions

`rustok-commerce-storefront` may pass aggregate checkout completion snapshots into this package and execute the async native/GraphQL orchestration callback, but checkout result presentation and complete-checkout request construction stay here.

See the platform documentation map in [`../../../docs/index.md`](../../../docs/index.md).
