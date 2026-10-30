# CRATE_API: rustok-module-sdk

## 1. Architectural Role & Boundary Enforcement

`rustok-module-sdk` is the standalone, product-neutral Rust author SDK for untrusted RusToK WebAssembly module components:
- **Single Canonical WIT Source:** Owns the definitive WebAssembly Interface Type (WIT) contract defined in `wit/module-runtime.wit` (`rustok:module@1.0.0/module-runtime`).
- **Component Model Guest Bindings:** Automatically generates guest bindings and export macros using Bytecode Alliance's `wit-bindgen`.
- **Zero Runtime Dependencies:** Intentionally decoupled from databases, HTTP hosts, Axum, SeaORM, and platform composition roots. Guest components depend strictly on this lightweight SDK.
- **Symmetric Host-Guest Interface:** Host execution engines (e.g. `rustok-sandbox`) compile against the exact same WIT definition, guaranteeing ABI compatibility without manual struct duplication.

---

## 2. Public Surface & Data Structures

### Identity Constants
- `SDK_VERSION: &str`: Current crate version derived from `CARGO_PKG_VERSION`.
- `WIT_PACKAGE: &str`: Package identifier `rustok:module@1.0.0`.
- `WIT_WORLD: &str`: Selected component world `module-runtime`.
- `WIT_SOURCE: &str`: Raw WIT definition text embedded via `include_str!`.

### Guest Traits & Macros
- `Guest`: Core trait required for guest module components.
  - `fn run(input: String) -> Result<String, String>`: Entry point invoked by the host sandbox runner.
- `export!(ComponentType)`: Procedural export macro registering the component type with the WebAssembly runtime.
- `rustok::module::host::invoke(capability: &str, operation: &str, input: &str) -> Result<String, String>`: Host capability invocation import available to admitted guest components.

---

## 3. Error Contracts & Edge Case Guarantees

- **Stringified Error Boundary:** In accordance with the untrusted component boundary, both `run` and `invoke` exchange serialized JSON strings and return `Result<String, String>`, isolating host memory and panic state from guest execution.
- **Strict WIT Versioning:** SemVer-pinned package namespace (`rustok:module@1.0.0`) guarantees ABI immutability across minor releases.

---

## 4. Concurrency & Concurrency Models

- **Thread-Isolated Guest Execution:** WASM components compiled with `rustok-module-sdk` execute in isolated, single-threaded WebAssembly instances managed by the host engine.
- **Stateless ABI Boundary:** Invocations across the host-guest seam are stateless; state persistence and concurrency management remain strictly host responsibilities.

---

## 5. Audit History & Verification Evidence

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` (`version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `rust-version.workspace = true`, `readme = "README.md"`, `repository.workspace = true`, `[package.metadata.docs.rs]`).
  - Audited WIT world invariants and guest trait generation.
  - Authored comprehensive `contract_tests.rs` locking in package identity, host invocation signature, and world structure.
  - Verified 3/3 unit tests, 1/1 integration tests, and 1/1 doctests:
    - `cargo test -p rustok-module-sdk --lib`: 3 passed, 0 failed.
    - `cargo test -p rustok-module-sdk --test author_api`: 1 passed.
  - Clippy verification: 0 warnings with `-D warnings`.

