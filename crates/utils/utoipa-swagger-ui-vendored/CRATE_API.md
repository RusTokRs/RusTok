# CRATE_API: utoipa-swagger-ui-vendored

## 1. Architectural Role & Boundary Enforcement

`utoipa-swagger-ui-vendored` provides embedded Swagger UI static distribution assets for OpenAPI documentation in RusToK:
- **Offline / Air-Gapped Documentation:** Bundles the pre-built Swagger UI distribution assets directly into the binary without requiring runtime external CDN access or internet connectivity.
- **Deterministic Asset Distribution:** Embeds compressed ZIP archives with deterministic hashes, ensuring bit-for-bit identical assets across all deployment environments.
- **Zero Runtime Dependencies:** Pure binary asset crate with zero runtime dependencies.

---

## 2. Public Surface & Data Structures

### Primary Export
- `SWAGGER_UI_VENDORED: &[u8]`: Raw byte slice containing the pre-packaged Swagger UI ZIP archive containing `swagger-initializer.js` and `index.html`.

---

## 3. Error Contracts & Edge Case Guarantees

- **Valid ZIP Magic Bytes:** Asset payload begins with the standard ZIP header signature `[0x50, 0x4B, 0x03, 0x04]` (`PK\x03\x04`).
- **Required File Entries:** Archive deterministically contains the entrypoint `index.html` and configuration initializer `swagger-initializer.js`.

---

## 4. Concurrency & Concurrency Models

- **Immutable Static Asset:** `SWAGGER_UI_VENDORED` is a compile-time static slice (`&'static [u8]`), inherently thread-safe (`Sync + Send`), lock-free, and immutable.

---

## 5. Audit History & Verification Evidence

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` (`version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `rust-version.workspace = true`, `readme = "README.md"`, `repository.workspace = true`, `[package.metadata.docs.rs]`).
  - Authored 5-section `CRATE_API.md` and `contract_tests.rs`.
  - Verified static slice integrity, ZIP magic bytes, and asset contents.
  - Verified unit and contract tests:
    - `cargo test -p utoipa-swagger-ui-vendored --lib`: 2 passed, 0 failed.
  - Clippy verification: 0 warnings with `-D warnings`.
