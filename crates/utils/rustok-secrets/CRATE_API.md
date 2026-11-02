# CRATE_API: rustok-secrets

## 1. Architectural Role & Boundary Enforcement

`rustok-secrets` owns runtime secret-reference contracts and the process-level resolver registry for RusToK:
- **Zero Plaintext Persistence:** Callers persist only `SecretRef { resolver, key }` references. Raw secrets are never stored in databases or serialized into application logs.
- **Resolver Registry Isolation:** Resolvers, endpoints, and credentials are configured at process startup in `SecretResolverRegistry`. Tenants cannot provide custom network endpoints or escape resolver containment.
- **Tenant Policy Guard:** Every resolver registration binds a `SecretAccessPolicy` (`DenyAll`, `Exact`, `Prefix`, `TenantPrefix`). `resolve_for_tenant` and `validate_reference_for_tenant` enforce tenant boundary isolation, preventing cross-tenant secret access.
- **Memory Redaction:** Secret values are wrapped in `secrecy::SecretString`, guaranteeing that `Debug` and `Display` implementations redact sensitive payloads.

---

## 2. Public Surface & Data Structures

### Primary Types & Traits
- `SecretRef`: Persisted reference identifying a resolver alias and key (`resolver: String`, `key: String`).
- `SecretRefStatus`: Query status descriptor (`resolver: String`, `configured: bool`).
- `SecretAccessPolicy`: Scoped access policy.
  - `DenyAll`
  - `Exact(Vec<String>)`
  - `Prefix(Vec<String>)`
  - `TenantPrefix { prefix: String }`
- `SecretResolver`: Async trait for backend resolvers (`async fn resolve(&self, key: &str) -> Result<SecretString, SecretError>`).
- `SecretResolverRegistry`: Thread-safe resolver catalog with in-memory caching.
  - `builder() -> SecretResolverRegistryBuilder`
  - `resolve_for_tenant(&self, tenant_id: Uuid, reference: &SecretRef) -> Result<SecretString, SecretError>`
  - `validate_reference_for_tenant(&self, tenant_id: Uuid, reference: &SecretRef) -> Result<(), SecretError>`
  - `invalidate(&self, reference: Option<&SecretRef>)`
  - `contains(&self, alias: &str) -> bool`
- `SecretResolverRegistryBuilder`:
  - `resolver(alias, resolver, policy) -> Self`
  - `cache_ttl(ttl: Duration) -> Self` (clamped to max 60 seconds)
  - `build() -> SecretResolverRegistry`

### Built-in Resolvers
- `EnvResolver`: Resolves process environment variables.
- `MountedFileResolver`: Reads secrets from mounted filesystem paths with strict canonical path traversal containment.
- `VaultResolver`: HashiCorp Vault KV v2 resolver supporting Token or Kubernetes Service Account authentication.
- `KubernetesSecretResolver`: Reads in-cluster Kubernetes Secrets via service account tokens.
- `AwsSecretsManagerResolver`: Reads AWS Secrets Manager via default AWS credentials or custom client.
- `GcpSecretManagerResolver`: Reads GCP Secret Manager via Application Default Credentials (ADC) or custom client.
- `AzureKeyVaultResolver`: Reads Azure Key Vault via workload, managed, or developer identity credentials.

---

## 3. Error Contracts & Edge Case Guarantees

- `SecretError`:
  - `ResolverNotFound(String)`: Unknown resolver alias.
  - `BlankKey`: Key is whitespace or empty.
  - `ForbiddenKey { resolver, key }`: Access rejected by `SecretAccessPolicy`.
  - `NotFound { resolver, key }`: Secret not found in backend provider.
  - `Resolver { resolver, message }`: Underlying provider execution error.
- **Path Containment:** `MountedFileResolver` canonicalizes paths and rejects inputs escaping the configured mount root.
- **Provider Injection Hardening:**
  - `VaultResolver`: Enforces HTTPS endpoints, disallows userinfo/query/fragment, validates KV mount and secret path segments.
  - `KubernetesSecretResolver`: Enforces DNS-1123 naming rules for namespaces and secret names.
  - `GcpSecretManagerResolver`: Enforces project ID and secret ID regex constraints, preventing cross-project resource path injection.
  - `AzureKeyVaultResolver`: Rejects non-HTTPS or parameter-bearing vault endpoints.
- **Response Size Capping:** HTTP response payloads in remote resolvers (`VaultResolver`, `KubernetesSecretResolver`) are capped at 1 MB (`MAX_SECRET_RESPONSE_BYTES`).

---

## 4. Concurrency & Concurrency Models

- **Short-Lived Caching with Invalidation:** Cached secrets are stored in `Arc<RwLock<HashMap<SecretRef, CachedSecret>>>` with a maximum TTL of 60 seconds, allowing prompt rotation via `invalidate()`.
- **RwLock Reader/Writer Splitting:** Concurrent tenant reads utilize shared read locks (`RwLock::read()`); cache updates and invalidations serialize under write locks (`RwLock::write()`).

---

## 5. Audit History & Verification Evidence

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` (`version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `rust-version.workspace = true`, `readme = "README.md"`, `repository.workspace = true`, `[package.metadata.docs.rs]`).
  - Audited 6 Universal Logic Dimensions across local, cloud (AWS, GCP, Azure), and remote (Vault, Kubernetes) resolvers.
  - Authored comprehensive `contract_tests.rs` verifying `SecretAccessPolicy` isolation, blank key rejection, resolver validation, TTL clamping, and mounted file path traversal containment.
  - Verified 16/16 unit tests:
    - `cargo test -p rustok-secrets --lib`: 16 passed, 0 failed.
  - Clippy verification: 0 warnings with `-D warnings`.

