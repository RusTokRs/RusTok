# rustok-fulfillment-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Module-owned storefront UI package for `rustok-fulfillment`.

## Purpose

- Own storefront fulfillment/shipping handoff presentation.
- Own seller-aware delivery-group shipping selection UI and request DTO normalization.
- Keep shipping-option display and build-profile-selected native/GraphQL transport policy outside umbrella `rustok-commerce` while commerce temporarily provides the aggregate checkout SSR endpoint/body adapter.
- Ship package-owned `storefront/locales/en.json` and `storefront/locales/ru.json` bundles declared through `[provides.storefront_ui.i18n]`.
- Resolve manifest-entry copy from the host-provided `UiRouteContext.locale`; do not negotiate locale inside the package.

## Entry points

- `src/model.rs` — serializable storefront delivery-group and shipping-option DTOs.
- `src/core/mod.rs` — Leptos-free selection request helpers and labels.
- `src/transport.rs` — serializable selection command DTOs, selection-plan validation, typed transport errors, and owner selected-path policy for native + GraphQL-compatible command execution.
- `src/transport/native_server_adapter/server_functions.rs` — native server-function adapter backed by `HostRuntimeContext` DB/event-bus handles; it has no Loco runtime or outbox Loco-adapter dependency.
- `src/ui/leptos.rs` — Leptos render adapter for fulfillment-owned shipping handoff and selection UI.

See the platform documentation map in [`../../../docs/index.md`](../../../docs/index.md).
