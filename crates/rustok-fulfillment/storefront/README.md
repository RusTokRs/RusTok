# rustok-fulfillment-storefront

Module-owned storefront UI package for `rustok-fulfillment`.

## Purpose

- Own storefront fulfillment/shipping handoff presentation.
- Own seller-aware delivery-group shipping selection UI and request DTO normalization.
- Keep shipping-option display policy outside umbrella `rustok-commerce` while commerce temporarily provides aggregate checkout transport.

## Entry points

- `src/model.rs` — serializable storefront delivery-group and shipping-option DTOs.
- `src/core/mod.rs` — Leptos-free selection request helpers and labels.
- `src/ui/leptos.rs` — Leptos render adapter for fulfillment-owned shipping handoff and selection UI.

See the platform documentation map in [`../../../docs/index.md`](../../../docs/index.md).
