# rustok-order-storefront

Module-owned storefront UI package for `rustok-order`.

## Purpose

- Own storefront checkout result/order handoff presentation.
- Keep order status display policy outside umbrella `rustok-commerce`.

## Entry points

- `src/core.rs` — Leptos-free checkout result handoff view-model.
- `src/ui/leptos.rs` — Leptos render adapter for the order checkout result handoff.

## Interactions

`rustok-commerce-storefront` may pass aggregate checkout completion snapshots into this package while checkout orchestration transport remains in commerce.

See the platform documentation map in [`../../../docs/index.md`](../../../docs/index.md).
