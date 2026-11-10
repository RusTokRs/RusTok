# CRATE_API: rustok-installer

## 1. Architectural Role & Boundary Enforcement

`rustok-installer` is the shared installer foundation for RusToK. It owns installer domain contracts, data structures, and state transitions, guaranteeing zero duplication across entry points:
- **Zero Framework / Protocol Coupling:** The crate intentionally contains no HTTP handlers, CLI parser definitions, or direct database driver implementations.
- **Port/Adapter Boundary:** Execution interfaces are modeled as typed asynchronous ports:
  - `InstallDatabasePort`, `InstallSchemaPort`, `InstallBootstrapPort`, `InstallPersistencePort`, `InstallSeedPort`, `InstallAdminPort`, `InstallVerificationPort`, `InstallDeploymentPort`.
- **Feature Partitioning:**
  - `default = ["host-runtime", "seed-runtime"]`
  - `host-runtime`: Enables native runtime instance layout preparation, static distribution verification, and rollout integration.
  - `seed-runtime`: Enables consumer-owned seed profile workflows with identity and role provisioning.
  - Browser/WASM consumers may disable default features to consume lightweight, safe data structures and serialization contracts without native I/O.

---

## 2. Public Types & Data Structures

### Installation Planning & Topology
- `InstallEnvironment`: `Local`, `Demo`, `Test`, `Production`.
  - Methods: `is_production() -> bool`, `parse_cli_value(&str) -> Result<Self, String>`, `as_str() -> &'static str`.
- `InstallProfile`: `DevLocal`, `Monolith`, `HybridAdmin`, `HeadlessLeptos`.
  - Methods: `parse_cli_value(&str) -> Result<Self, String>`.
- `DatabaseEngine`: `Postgres`, `Sqlite`.
  - Methods: `parse_cli_value(&str) -> Result<Self, String>`.
- `DatabaseConfig`: `engine`, `url: SecretValue`, `create_if_missing: bool`.
- `TenantBootstrap`: `slug: String`, `name: String`.
- `AdminBootstrap`: `email: String`, `password: SecretValue`.
- `SeedProfile`: `None`, `Minimal`, `Dev`.
  - Methods: `parse_cli_value(&str) -> Result<Self, String>`, `default_enabled_modules() -> Vec<String>`.
- `ModuleSelection`: `enable: Vec<String>`, `disable: Vec<String>`.
- `InstallComposition`: `revision: String`, `hash: String`.
- `InstallTopologyMode`: `Monolith`, `Distributed`.
- `InstallRole`: `Monolith`, `Api`, `AdminSsr`, `StorefrontSsr`, `Worker`, `Registry`.
  - Methods: `supports_surface(surface) -> bool`, `as_str() -> &'static str`.
- `InstallSurface`: `Api`, `Admin`, `Storefront`, `Worker`, `Registry`.
- `InstallRoleAssignment`: `role: InstallRole`, `surfaces: Vec<InstallSurface>`.
- `InstallTopology`: `schema_version`, `mode`, `composition`, `distribution`, `surfaces`, `roles`.
  - Methods: `for_mode(mode) -> Self`, `bind_composition(rev, hash) -> Self`, `bind_distribution(dist) -> Self`, `validate() -> Result<(), String>`.
- `InstallPlan`: Aggregate configuration for an installation run.
  - Methods: `production_minimal(...) -> Self`.

### State Machine & Receipts
- `InstallState`: `Draft`, `PreflightPassed`, `ConfigPrepared`, `DatabaseReady`, `SchemaApplied`, `SeedApplied`, `AdminProvisioned`, `Deploying`, `Verified`, `Completed`, `Failed`, `FreshInstallCleaned`, `RecoveryRequired`.
  - Methods: `can_transition_to(self, to: InstallState) -> bool`, `transition_to(self, to: InstallState) -> Result<InstallState, StateTransitionError>`.
- `InstallStep`: `Preflight`, `Config`, `Database`, `Migrate`, `Seed`, `Admin`, `Deploy`, `Verify`, `Finalize`, `Rollback`.
- `ReceiptOutcome`: `Success`, `Skipped`, `Failed`.
- `InstallReceipt`: Durable execution proof per step.
  - Methods: `success(...) -> Result<Self, ReceiptError>`, `can_skip(...) -> Result<bool, ReceiptError>`.
- `checksum_json<T: Serialize>(value: &T) -> Result<String, ReceiptError>`: Deterministic SHA-256 JSON hashing.

### Secrets Handling
- `SecretMode`: `Env`, `DotenvFile`, `MountedFile`, `ExternalSecret`.
  - Methods: `parse_cli_value(&str) -> Result<Self, String>`.
- `SecretRef`: `backend: String`, `key: String`.
  - Methods: `parse_cli_value(&str) -> Result<Self, String>`.
- `SecretValue`: `Plaintext { value: String }`, `Reference { reference: SecretRef }`.
  - Methods: `as_plaintext(&self) -> Option<&str>`.
- `redact_secret(&SecretValue) -> serde_json::Value`
- `redact_install_plan(&InstallPlan) -> serde_json::Value`
- `resolve_local_secret_value(&SecretValue, label: &str) -> Result<String, SecretResolutionError>`

---

## 3. Public Ports, Operations & Error Contracts

### Execution Pipeline (`execution.rs`, with `feature = "host-runtime"`)
- `execute_install_apply<P>(ports: &P, plan: InstallPlan, options: InstallApplyOptions) -> Result<InstallApplyOutput, InstallExecutionError>`
- Required Port Traits:
  - `InstallDatabasePort`: Prepares target DB connection and runtime handle.
  - `InstallSchemaPort<R>`: Applies bootstrap owner schema, then canonical remaining schema.
  - `InstallBootstrapPort<R>`: Imports base distribution receipt.
  - `InstallPersistencePort<R>`: Manages durable sessions, locks, receipts, and state transitions.
  - `InstallSeedPort<R>`: Applies initial seed profile.
  - `InstallAdminPort<R>`: Provisions the super-admin user.
  - `InstallVerificationPort<R>`: Verifies tenant, admin, and active modules.
  - `InstallDeploymentPort<R>`: Dispatches immutable distribution rollout.

### Seed Runtime Pipeline (`seed.rs`, with `feature = "seed-runtime"`)
- `execute_seed_profile(request, tenant_port, principal_port, module_port, content_port) -> Result<SeedExecutionOutcome, SeedExecutionError>`
- A requested content import must use the development profile and supply a
  non-empty path, loader, and admin request. These prerequisites and the demo
  customer password are checked before any seed owner is called. Partial
  content outcomes fail unless continuation is explicitly requested; an outcome
  naming a different module always fails. `SeedContentOutcome::is_success`
  requires both zero failed items and no reported errors; an unknown item count
  remains zero and does not turn a loader error into success.
- Ports:
  - `SeedTenantPort`: Ensures seed tenant creation.
  - `SeedPrincipalPort`: Atomically provisions user credentials and initial RBAC role.
  - `SeedModulePort`: Configures tenant module enablement flags.
  - `SeedContentPort`: Loads optional demo module fixtures.

### Errors
- `StateTransitionError`: Invalid state transition attempt.
- `SecretResolutionError`: Missing, unreadable, or unsupported secret backend.
- `ReceiptError`: JSON serialization, I/O, or signature verification failure.
- `InstallExecutionError`: Pipeline stage failure.
- `SeedExecutionError`: Seed validation, dependency, or content failure.

---

## 4. Usage Patterns & Invariants

1. **State Machine Durability & Non-Rollback Integrity:**
   - Early stage failures (prior to schema application) allow clean-up via `FreshInstallCleaned`.
   - Once database schemas or persistent mutations occur (`SchemaApplied`, `SeedApplied`, `AdminProvisioned`, `Deploying`, `Verified`), failures must transition to `RecoveryRequired` rather than pretending rollback succeeded.
2. **Secret Redaction Invariant:**
   - Plaintext credentials must never appear in persisted session records, step receipts, diagnostics JSON, or logs.
   - Production installations strictly reject plaintext secrets during preflight evaluation.
3. **Distribution Admittance Binding:**
   - Every install topology must be digest-pinned and validated against the admitted distribution bundle and composition hash.
   - Roles must strictly match the admitted static distribution artifacts.

---

## 5. Verification & Testing Matrix

- `contract_tests.rs`: Comprehensive contract suite locking in state transitions, CLI parsing, preflight rules, secret redaction, JSON checksumming, and seed profile operations.
- `tests/base_bundle_install_tests.rs`: Multi-stage simulation of the `execute_install_apply` state machine, failure recovery branching, and distribution verification.
- `tests/seed_principal_atomicity_guard.rs`: Static architecture enforcement ensuring atomic identity and role provisioning.

