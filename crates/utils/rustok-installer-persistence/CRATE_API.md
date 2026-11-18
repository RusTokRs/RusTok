# CRATE_API: rustok-installer-persistence

## 1. Architectural Role & Boundary Enforcement

`rustok-installer-persistence` is the SeaORM persistence adapter for the installer subsystem of RusToK:
- **Zero Host Leakage:** SeaORM entities, migration execution, and database schema mappings remain strictly contained within this crate. Neither `apps/server` nor `rustok-installer-cli` maintain private installer database models.
- **Contract Fulfillment:** Implements neutral ports defined by `rustok-installer`:
  - `InstallDatabasePort`, `InstallSchemaPort`, `InstallBootstrapPort`, `InstallPersistencePort`, `InstallSeedPort`, `InstallAdminPort`, `InstallVerificationPort`, `InstallDeploymentPort`.
- **Migration composition:** Explicitly enables the `rustok-migrations`
  `composition` feature required by its owner-schema and remaining-schema ports.
- **Atomic Bootstrap & Provisioning:** Unifies user creation and RBAC assignment in single-transaction primitives (`AuthUserBootstrapDbWriter`, `RbacRoleAssignmentDbWriter`) to prevent orphaned administrator identities upon role binding failures.
- **Decoupled DTOs:** Public consumer surfaces expose clean data transfer objects such as `InstallStepReceiptItem` and `InstallSessionRecord` instead of raw, mutable SeaORM active models.

---

## 2. Public Surface & Data Structures

### Primary Services & Port Adapters
- `InstallerPersistenceService`: SeaORM-backed persistence manager for installer jobs, sessions, and step receipts.
  - `new(db: DatabaseConnection) -> Self`
  - `create_http_job(job_id: Uuid, submitted_at: DateTime<Utc>) -> Result<install_http_job::Model, DbErr>`
  - `get_http_job(job_id: Uuid) -> Result<Option<install_http_job::Model>, DbErr>`
  - `has_running_http_job() -> Result<bool, DbErr>`
  - `finish_http_job_succeeded(...) -> Result<install_http_job::Model, DbErr>` (compare-and-set on `status = running`)
  - `finish_http_job_failed(...) -> Result<install_http_job::Model, DbErr>` (compare-and-set with bounded error message)
  - `create_session(plan: &InstallPlan, tenant_id: Option<Uuid>, created_by: Option<Uuid>) -> Result<install_session::Model, DbErr>`
  - `record_receipt(session: &InstallSessionRecord, receipt: &InstallReceipt) -> Result<install_step_receipt::Model, DbErr>`
  - `acquire_lock(session: install_session::Model, owner: &str, ttl: Duration) -> Result<install_session::Model, DbErr>`
  - `set_state(session: &InstallSessionRecord, state: InstallState) -> Result<install_session::Model, DbErr>`
  - `set_tenant_id(session: &InstallSessionRecord, tenant_id: Uuid) -> Result<install_session::Model, DbErr>`
  - `get_session(session_id: Uuid) -> Result<Option<install_session::Model>, DbErr>`
  - `latest_session() -> Result<Option<install_session::Model>, DbErr>`
  - `has_completed_session() -> Result<bool, DbErr>`
  - `list_receipts(session_id: Uuid) -> Result<Vec<InstallStepReceiptItem>, DbErr>`
- `SeaOrmInstallerPorts`: Default SeaORM implementation of database, schema, and durable installer-state ports.
- `SeaOrmInstallerApplyPorts<'a>`: Standalone installer executor adapter independent of HTTP host state.
  - `new(registry: &'a ModuleRegistry) -> Self`
- `SeaOrmInstallerBootstrapPorts<'a>`: Reusable seed and bootstrap adapter composing tenant, user, RBAC, and module lifecycle writers.
  - `new(db: DatabaseConnection, registry: &'a ModuleRegistry, defaults: Vec<String>) -> Self`
  - `apply_seed_profile(plan: &InstallPlan, actor: &str) -> Result<InstallSeedOutcome, InstallExecutionError>`
  - `provision_admin(plan: &InstallPlan, tenant_id: Uuid, password: &str) -> Result<InstallAdminOutcome, InstallExecutionError>`

### Entities & Decoupled DTOs
- `entities::install_session::Model`: Database entity representing an installer session (`id`, `status`, `attempt_generation`, `profile`, `plan_snapshot`, `lock_owner`, `lock_expires_at`, `completed_at`).
- `entities::install_step_receipt::Model`: Step-level receipt entity tracking the `attempt_generation`, inputs, checksums, and diagnostics.
- `InstallSessionRecord`: Neutral session snapshot carrying status, owner, and attempt generation to every mutation port.
- `InstallStepReceiptItem`: Includes the generation under which the receipt was recorded.
- `entities::install_http_job::Model`: Asynchronous HTTP install job tracking submit, start, completion, and sanitized errors.


---

## 3. Error Contracts & Edge Case Guarantees

- **Bounded & Sanitized Error Disclosures:** `bounded_http_job_error_message` prevents connection strings, raw passwords, and internal infrastructure exceptions from being stored in public job records.
- **Job Output Size Capping:** HTTP installer job outputs exceeding 256 KB (`MAX_HTTP_INSTALL_JOB_OUTPUT_BYTES`) omit bulky payloads to protect the persistence store.
- **Lock Owner Validation:** Empty or whitespace-only lock owners are rejected with an explicit validation error; values exceeding the 128-byte storage bound are rejected before claim.
- **Lease TTL Validation:** Lock TTL must be 1–86,400 seconds; zero, negative, and over-limit durations are rejected. Timestamp addition is checked and returns an error instead of panicking on overflow.
- **Postgres Database Provisioning Validation:** URL scheme verification (`postgres` / `postgresql`), non-empty database names, non-empty usernames, and strict SQL identifier quoting (`quote_ident`, `quote_literal`).
- **Production Environment Seed Guard:** Rejects `SeedProfile::Dev` when `plan.environment.is_production()`.
- **Seed lifecycle replay:** The deterministic tenant/module/direction/actor key
  reuses the owner journal's original revision. Missing historical revision,
  conflicting command identity, and a later observed change to the requested
  intent fail explicitly; the final intent read does not lock out later writes.
  Fresh enables fail before journal admission when the admitted release cannot
  supply its schema. Current releases do not yet supply it, so new seed enables
  remain blocked; exact completed-command replay and disable remain available.

---

## 4. Concurrency & Concurrency Models

- **Compare-and-Set Job Terminal State:** `finish_http_job_succeeded` and `finish_http_job_failed` filter on `status = running`, guaranteeing that once a terminal status is written, concurrent transitions fail.
- **Serializable Lock Acquisition:** PostgreSQL and MySQL begin lock acquisition in a `Serializable` transaction. SQLite uses the conditional session-claim update as its write fence.
- **Fail-Closed Global Admission:** A session is claimed only from `draft` with `lock_owner IS NULL`, and cannot be claimed twice. Any other unreconciled session with a recorded owner blocks a new apply even after its lease expires. `recovery_required` blocks admission even if its owner has been cleared; completed, safely failed, and freshly cleaned sessions release the gate.
- **No Lease-Expiry Takeover:** Expiry is not proof that the old process stopped. Stale sessions require explicit reconciliation before another apply can start. Resumable execution and external-effect reconciliation remain open.
- **Attempt Fencing:** A successful claim atomically increments `attempt_generation` while recording the owner. Every state, tenant, and receipt mutation locks the target session row in a transaction and verifies both values before writing. PostgreSQL/MySQL use an exclusive row lock; SQLite obtains its write fence before reading. Stale owners and stale generations are rejected. Receipt rows persist the generation that authorized the write.
- **Recovery Evidence:** In `RecoveryRequired`, only `Rollback` receipts may be appended. The persistence owner will release a session into `Failed` or `FreshInstallCleaned` only when the current attempt has a successful `Rollback` receipt; a failed rollback receipt alone never releases admission.
- **Atomic Administrator Provisioning:** Admin identity creation (`ensure_user_on`) and SuperAdmin role assignment (`assign_role_permissions_on`) occur inside a single database transaction. Any failure triggers rollback, ensuring zero orphaned identities without roles.
- **Terminal Session Permanence:** Completed installer sessions permanently record `completed_at`, and `has_completed_session()` remains `true` even in the presence of subsequent failed recovery attempts.

---

## 5. Audit History & Verification Evidence

- **Installer lease and attempt-fencing hardening (2026-11-18):**
  - Added one-time draft-session claims and a fail-closed global admission gate that
    retains unfinished and `recovery_required` sessions after lease expiry.
  - Extended serializable acquisition to MySQL and retained PostgreSQL; SQLite
    obtains a write fence through the conditional claim update.
  - Added a durable monotonic attempt generation to session and receipt rows;
    claims increment it atomically with owner assignment.
  - State, tenant, and receipt mutations now lock the session row and compare
    the current owner+generation before writing. Receipts retain the generation.
  - Bounded TTLs to 1–86,400 seconds and guarded timestamp addition against overflow.
  - Added regression coverage for same-session re-claim, expired-lease blocking,
    stale owner/generation rejection, generation advancement, stale receipt
    rejection, recovery-required blocking, final-state release, and TTL bounds.
  - Verification was not run in this task at the user's request; see the server
    implementation plan for exact commands.

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` with workspace metadata (`repository.workspace = true`, `rust-version.workspace = true`, `readme = "README.md"`, `rustok-migrations.workspace = true`, `[package.metadata.docs.rs]`).
  - Audited 6 Universal Logic Dimensions across session records, step receipts, serializable lock acquisition, atomic principal provisioning, and state transitions.
  - Fixed cross-crate migration dependency gap in `rustok-commerce` where outdated cart migration `m20260325_000102_create_cart_tables` was referenced instead of canonical `m20261029_000101_create_cart_schema`.
  - Added comprehensive `contract_tests.rs` verifying session persistence, state transitions, lock competition rejection, whitespace owner validation, and receipt roundtrip conversion.
  - Verified 6/6 unit tests and 5/5 integration tests:
    - `cargo test -p rustok-installer-persistence --lib`: 6 tests passed.
    - `cargo test -p rustok-installer-persistence --test completion_guard`: 2 tests passed.
    - `cargo test -p rustok-installer-persistence --test admin_provisioning_atomicity`: 3 tests passed.
  - Clippy verification: 0 warnings with `-D warnings`.

