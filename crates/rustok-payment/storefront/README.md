# rustok-payment-storefront

Module-owned storefront UI package for `rustok-payment`.

## Purpose

- Own storefront payment collection presentation and handoff copy.
- Keep payment display policy outside umbrella `rustok-commerce`.
- Provide Leptos adapters that can later be reused/replaced by Dioxus-facing adapters through the same payment-owned core contract.
- Ship package-owned `storefront/locales/en.json` and `storefront/locales/ru.json` bundles declared through `[provides.storefront_ui.i18n]`.
- Resolve manifest-entry copy from the host-provided `UiRouteContext.locale`; do not negotiate locale inside the package.

## Entry points

- `src/core.rs` — Leptos-free payment collection card view-model, fallback policy, and action-label policy.
- `src/transport.rs` — framework-free payment collection create/reuse request DTO, command metadata, and normalization facade used by host orchestration during the compatibility window.
- `src/ui/leptos.rs` — Leptos render adapter for payment collection handoff; action components emit payment-owned request DTOs instead of raw cart ids.

## Interactions

`rustok-commerce-storefront` may temporarily pass checkout-orchestration payment collection snapshots into this package and execute the async native/GraphQL orchestration callback, but presentation ownership, payment request construction, and create/reuse command metadata stay here.

See the platform documentation map in [`../../../docs/index.md`](../../../docs/index.md).
