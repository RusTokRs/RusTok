# rustok-payment-storefront

Module-owned storefront UI package for `rustok-payment`.

## Purpose

- Own storefront payment collection presentation and handoff copy.
- Keep payment display policy outside umbrella `rustok-commerce`.
- Provide Leptos adapters that can later be reused/replaced by Dioxus-facing adapters through the same payment-owned core contract.

## Entry points

- `src/core.rs` — Leptos-free payment collection card view-model and fallback policy.
- `src/ui/leptos.rs` — Leptos render adapter for payment collection handoff.

## Interactions

`rustok-commerce-storefront` may temporarily pass checkout-orchestration payment collection snapshots into this package, but presentation ownership stays here.

See the platform documentation map in [`../../../docs/index.md`](../../../docs/index.md).
