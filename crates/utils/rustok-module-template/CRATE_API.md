# CRATE_API: rustok-module-template

## 1. Architectural Role & Boundary Enforcement

`rustok-module-template` is the pure, deterministic renderer for the canonical standalone Rust module component project:
- **Independent Versioning & Provenance:** Versioned independently from `rustok-module-sdk` and `rustok-sandbox`, recording build toolchain (`RUST_TOOLCHAIN = "1.96.0"`) and template identity (`TEMPLATE_VERSION`).
- **Fail-Closed Index Integration Boundary:** Explicitly documents the architectural boundary between standalone WASI P2 components and native server modules (`docs/index-integration.md`). Prevents components from attempting unauthorized host database connections or native `rustok-index` dependencies.
- **Strict File Contract:** Generates a complete standalone project containing Cargo configuration, pinned toolchain (`wasm32-wasip2`), build policy, unit/contract tests, localization dictionaries (`en.json`), source manifest (`source-manifest.json`), and bounded sandbox scenarios.
- **Pre-emission Invariant Verification:** Every rendered source manifest and local sandbox scenario is parsed and validated against its canonical schema parser before files are emitted.

---

## 2. Public Surface & Data Structures

### Identity Constants
- `TEMPLATE_VERSION: &str`: Template crate version from `CARGO_PKG_VERSION`.
- `RUST_TOOLCHAIN: &str`: Pinned native Rust compiler version (`1.96.0`).

### Input & Render Models
- `ModuleTemplateInput`: Scaffolding specification.
  - Fields: `slug: String`, `version: String`, `display_name: String`.
- `TemplateFile`: Generated file payload.
  - Fields: `path: &'static str`, `contents: Vec<u8>`.
- `RenderedModule`: Collection of rendered template files.
  - `files(&self) -> &[TemplateFile]`
  - `file(&self, path: &str) -> Option<&[u8]>`
- `render(input: &ModuleTemplateInput) -> Result<RenderedModule, ModuleTemplateError>`: Main generation function.

---

## 3. Error Contracts & Edge Case Guarantees

- `ModuleTemplateError`:
  - `InvalidDisplayName`: Display name is empty, exceeds 80 characters, or contains control characters.
  - `InvalidSourceManifest(String)`: Generated source manifest failed schema or semantic validation.
  - `InvalidSandboxScenario(String)`: Generated sandbox scenario failed `LocalSandboxScenario::parse`.
  - `Serialization(serde_json::Error)`: JSON schema serialization failure.
  - `UnresolvedPlaceholder`: Any placeholder token (`{{` or `}}`) remained unreplaced in output files.

---

## 4. Concurrency & Concurrency Models

- **Pure Functional Rendering:** `render` is completely pure, memory-only, and stateless. No disk I/O, network requests, or environment variables are accessed during template evaluation.
- **Thread-Safe & Re-entrant:** Safe for concurrent invocations across worker threads or CLI processes.

---

## 5. Audit History & Verification Evidence

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` (`version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `rust-version.workspace = true`, `readme = "README.md"`, `repository.workspace = true`, `[package.metadata.docs.rs]`).
  - Audited 6 Universal Logic Dimensions across display name validation, placeholder resolution, schema digests, and sandbox scenario constraints.
  - Authored comprehensive `contract_tests.rs` verifying display name validation bounds, file completeness, placeholder-free outputs, Cargo metadata, and pinned WASI toolchain targets.
  - Verified 8/8 unit tests:
    - `cargo test -p rustok-module-template --lib`: 8 passed, 0 failed.
  - Clippy verification: 0 warnings with `-D warnings`.

