# CRATE_API: rustok-storage

## 1. Architectural Role & Boundary Enforcement

`rustok-storage` owns runtime storage driver initialization and canonical object-key policies for RusToK:
- **No Domain CRUD Wrapping:** Domain modules consume `Arc<dyn object_store::ObjectStore>` directly. This crate does not wrap `put`/`get`/`delete` or invent proprietary storage facades over `object_store`.
- **Decoupled Physical Placement:** Storage backends (Local filesystem, S3-compatible, or In-Memory) are configured by host composition. Domain modules construct portable logical keys; physical filesystem root directories or cloud bucket names never leak into object identities.
- **Hierarchical & CAS Key Invariants:**
  - *Chronological Keys:* Partitioned by date and terminal byte shard (`namespace/zone/scope/YYYY/MM/DD/shard/id.extension`), preventing filesystem directory node bloat.
  - *Content-Addressed Digest Keys:* Partitioned by SHA-256 two-byte prefixes (`namespace/objects/scope/sha256/aa/bb/full_digest`) for content-addressed CAS artifacts.
  - *Multi-Tenant Isolation:* Scopes enforce tenant boundaries (`tenants/{tenant_id}`, `tenants/{tenant_id}/modules/{module}`, or opaque multi-dimensional namespaces `tenants/{tenant_id}/owners/{owner_id}/instances/{instance_id}`).

---

## 2. Public Surface & Data Structures

### Primary Types & Entrypoints
- `StorageRuntime`: Composite runtime providing storage engine, optional signer, storage kind, and public URL formatting.
  - `from_config(config: &StorageConfig) -> object_store::Result<Self>`
  - `local(config: &LocalStorageConfig) -> object_store::Result<Self>` (synchronous for bootstrap)
  - `local_async(config: &LocalStorageConfig) -> object_store::Result<Self>` (asynchronous for server startup)
  - `in_memory() -> Self` (ephemeral store for tests and fallback contexts)
  - `with_signer(mut self, signer: Arc<dyn Signer>) -> Self`
  - `public_url(&self, path: &Path) -> Option<String>`
  - `put_options(&self, content_type: impl Into<String>) -> PutOptions`
  - `signed_download_url(&self, path: &Path, expires_in: Duration) -> object_store::Result<Option<String>>`
  - `signed_upload_url(&self, path: &Path, expires_in: Duration) -> object_store::Result<Option<String>>`
- `StorageConfig`: Runtime configuration selecting driver (`Local`, `S3`) and driver-specific parameters.
  - `bind_local_base_dir(&mut self, base_dir: impl AsRef<std::path::Path>)`
- `LocalStorageConfig`: Configuration for local directory storage (`base_dir`, `base_url`, `fsync`).
- `S3StorageConfig`: Configuration for S3-compatible cloud storage (`bucket`, `region`, `endpoint_url`, `access_key_id`, `secret_access_key`, `session_token`, `public_base_url`, `allow_http`, `virtual_hosted_style_request`).
  - Implements custom `Debug` with sensitive access credentials redacted.
- `StorageDriver`: Configuration enum (`Local`, `S3`).
- `StorageKind`: Runtime indicator enum (`Local`, `S3`).
- `ObjectZone`: Storage zone (`Objects`, `Staging`).
- `ObjectScope`: Scoping model (`Tenant(Uuid)`, `TenantModule { tenant_id, module }`, `Namespace { tenant_id, owner_id, instance_id }`, `Platform`).
- `ObjectKey`: Chronological key constructor wrapping `object_store::path::Path`.
- `DigestObjectKey`: SHA-256 content-addressed key constructor wrapping `object_store::path::Path`.

---

## 3. Error Contracts & Edge Case Guarantees

- `KeyError`:
  - `InvalidNamespaceIdentity`: Tenant, owner, or instance UUID is nil.
  - `InvalidSegment { name, value }`: Segment exceeds 64 characters, is empty, or contains non-lowercase alphanumeric / non-hyphen characters.
  - `InvalidExtension(String)`: Extension exceeds 16 characters, is empty, or contains non-lowercase alphanumeric characters.
  - `InvalidSha256Digest`: Hex string is not exactly 64 lowercase hexadecimal characters.
- **Path Sanitization & Injection Prevention:** Validated segments reject uppercase characters, slashes, backslashes, control characters, and path traversal sequences (`..`).
- **Local Attribute Degradation:** Local filesystem backends reject arbitrary metadata attributes; `put_options()` returns default options for `StorageKind::Local` while preserving `ContentType` on S3.
- **Credential Redaction:** `S3StorageConfig` implements custom `Debug` formatting that redacts access keys, secret keys, and session tokens, reporting only whether they are configured.

---

## 4. Concurrency & Concurrency Models

- **Thread-Safe Runtime Sharing:** `StorageRuntime` contains `Arc<dyn ObjectStore>` and optional `Arc<dyn Signer>`, enabling zero-overhead cloning across async tasks and server worker threads.
- **Conditional CAS & Concurrency Safety:** Standard object store operations support `PutMode::Create` and `PutMode::Update(UpdateVersion)`, preventing TOCTOU races during concurrent uploads.
- **Multipart Lifecycle Support:** Runtimes support initiated, chunked, and aborted multipart uploads (`put_multipart`), ensuring incomplete multi-megabyte uploads leave no orphaned storage artifacts upon failure.

---

## 5. Audit History & Verification Evidence

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` (`version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `rust-version.workspace = true`, `readme = "README.md"`, `repository.workspace = true`, `[package.metadata.docs.rs]`).
  - Verified 6 Universal Logic Dimensions across local, in-memory, and S3 storage adapters.
  - Authored comprehensive `contract_tests.rs` covering key construction, validation boundaries, custom debug redaction, and runtime options.
  - Verified unit and integration tests:
    - `cargo test -p rustok-storage --lib` (unit & contract tests)
    - `cargo test -p rustok-storage --test object_store_conformance`
  - Clippy verification: 0 warnings with `-D warnings`.

