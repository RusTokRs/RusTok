# rustok-module-sdk

## Purpose

`rustok-module-sdk` is the product-neutral Rust author SDK for untrusted RusToK
module components. It packages the canonical WIT world and generates guest
bindings with Bytecode Alliance `wit-bindgen` tooling.

## Responsibilities

- Own the single canonical `rustok:module@1.0.0/module-runtime` WIT source.
- Generate Rust imports, the guest `Guest` trait, and the public component
  export macro from that source.
- Expose stable package/world identity constants for author tooling.

## Interactions

Guest crates depend on this SDK, implement `Guest::run`, call the generated
`rustok::module::host::invoke` import for admitted capabilities, and export the
component with `rustok_module_sdk::export!(Module)`. The neutral host executor generates
its Wasmtime bindings from the same WIT file; neither side hand-maintains ABI
structs.

The SDK has no server, database, marketplace, AI, Alloy, network-client, or
platform-runtime dependency.

`rustok-module-template` versions and renders the canonical standalone author
project separately; the SDK remains the sole WIT/bindings owner.

## Entry Points

- `Guest`
- `rustok::module::host::invoke`
- `export!(ComponentType)`
- `SDK_VERSION`, `WIT_PACKAGE`, `WIT_WORLD`, and `WIT_SOURCE`

See the [local documentation](./docs/README.md).
