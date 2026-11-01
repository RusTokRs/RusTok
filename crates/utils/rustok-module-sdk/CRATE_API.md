# rustok-module-sdk — Crate API

Product-neutral Rust author SDK for untrusted RusToK module components.

## Public surface

- `SDK_VERSION` — independently released author SDK identity recorded in build provenance.
- `WIT_PACKAGE` / `WIT_WORLD` / `WIT_SOURCE` — the canonical WIT contract identities and source.
- `bindings` re-exports: `Guest` and the `rustok` guest world bindings.

## Invariants

- The SDK stays product-neutral: no RusToK domain module types leak into the guest contract.
- The WIT package version is the compatibility boundary; guest code compiled against it is accepted only for the matching world.
