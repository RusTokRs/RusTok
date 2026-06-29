# rustok-ai-product-admin

Module-owned admin surface for `rustok-ai-product`.

## FFA boundary

- `src/core.rs` owns product-copy, attribute-suggestion, and review-queue panel policy.
- `src/transport.rs` owns the admin bootstrap facade and currently exposes a native-first placeholder profile.
- `src/ui/leptos.rs` is the explicit Leptos adapter boundary. It exports a compile-safe view descriptor until the host integrates concrete rendered widgets.

This package is no longer a pure scaffold: the FFA `core + transport + ui` split is present, while the runtime rendering remains a planned follow-up.
