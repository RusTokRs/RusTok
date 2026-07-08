# rustok-ai-order-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Module-owned admin surface for `rustok-ai-order`.

## FFA boundary

- `src/core.rs` owns order analytics, operator-assistant, and risk-summary panel policy.
- `src/transport.rs` owns the admin bootstrap facade and currently exposes a build-profile-selected native placeholder profile.
- `src/ui/leptos.rs` is the explicit Leptos adapter boundary. It exports a compile-safe view descriptor until the host integrates concrete rendered widgets.

This package is no longer a pure scaffold: the FFA `core + transport + ui` split is present, while the runtime rendering remains a planned follow-up.
