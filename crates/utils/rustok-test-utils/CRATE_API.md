# CRATE_API: rustok-test-utils

## 1. Architectural Role & Boundary Enforcement

`rustok-test-utils` owns test harnesses, in-memory/Postgres database helpers, mock event infrastructure, and domain entity fixtures across RusToK:
- **Dev-Only Dependency:** Dev-only dependency; never linked into production server binaries or customer-facing runtime distributions.
- **Isolated Test Databases:** Every test execution receives an isolated, freshly-initialized SQLite in-memory database or dynamically created/dropped PostgreSQL test schema.
- **Zero Raw Broker Requirements:** Tests record domain events via `MockEventTransport` without spinning up external message brokers (Iggy, RabbitMQ).
- **Domain Entity Fixtures:** Provides idiomatic builder patterns (`UserFixture`, `TenantFixture`, `ProductFixture`, `NodeFixture`) ensuring complete test data initialization with sensible defaults.

---

## 2. Public Surface & Data Structures

### Primary Modules
- `db`: Database setup, migration execution, and schema assertions.
  - `setup_test_db() -> DatabaseConnection`
  - `connect_postgres(...) -> DatabaseConnection`
  - `create_postgres_database(...)` / `drop_postgres_database_if_exists(...)`
  - `unique_postgres_database_name(...)` / `postgres_database_url(...)`
  - `assert_postgres_table_missing(...)` / `assert_postgres_column_missing(...)` / `assert_postgres_column_contract(...)`
- `events`: Event bus mocking and assertion helpers.
  - `MockEventTransport`: Implements `rustok_core::EventTransport`.
  - `MockEventBus`: Implements `rustok_core::EventBus`.
  - `mock_event_bus() -> Arc<MockEventBus>`
  - `mock_transactional_event_bus() -> TransactionalEventBus`
- `fixtures`: Builder patterns for domain entities.
  - `UserFixture`: Creates test users (`admin`, `super_admin`, `customer`, `manager`).
  - `TenantFixture`: Creates multi-tenant workspace records.
  - `ProductFixture` & `NodeFixture`: Creates catalog products and CMS content nodes.
- `helpers`: Security context generators, test IDs, and assertion macros.
  - `super_admin_context()`, `admin_context()`, `manager_context()`, `customer_context()`, `user_context(role, id)`
  - `unique_test_id()`, `unique_email()`, `unique_slug()`
  - `wait_for(timeout, interval, condition)`
  - Macros: `assert_ok!`, `assert_err!`, `assert_error_type!`, `assert_permission_scope!`, `with_roles!`

---

## 3. Error Contracts & Edge Case Guarantees

- **Migration Isolation:** SQLite in-memory connections enforce single-connection configuration (`max_connections = 1`) to eliminate multi-connection "table not found" race conditions.
- **Fail-Closed Database Cleanup:** PostgreSQL dynamic test database helpers drop transient databases via `DROP DATABASE IF EXISTS` with active connection termination.
- **Deterministic Event Ordering:** `MockEventTransport` preserves event insertion order under thread-safe mutex locks.

---

## 4. Concurrency & Concurrency Models

- **Mutex-Protected In-Memory Repositories:** Recorded events in `MockEventTransport` are guarded by `Arc<Mutex<Vec<RecordedEvent>>>`, allowing parallel test workers to publish and inspect events safely.
- **Global In-Memory SQLite Lock:** `DB_LOCK` serializes concurrent SQLite in-memory schema setups across parallel test threads.

---

## 5. Audit History & Verification Evidence

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` (`version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `rust-version.workspace = true`, `readme = "README.md"`, `repository.workspace = true`, `[package.metadata.docs.rs]`).
  - Verified 6 Universal Logic Dimensions across database utilities, mock event bus, and fixtures.
  - Standardized `CRATE_API.md` and verified contract tests:
    - `cargo test -p rustok-test-utils --lib`: 15 passed, 1 ignored (requires sqlite feature in seaorm).
    - `cargo test -p rustok-test-utils --test contract_surface`: 1 passed.
    - 23 passed doctests.
  - Clippy verification: 0 warnings with `-D warnings`.

---

## Minimum Contract Set

### Input DTOs/Commands
- Fixture builders provide fluent methods (`with_*`) that validate preconditions before building domain DTOs.
- Helpers create strictly valid `SecurityContext` models matching RBAC role requirements.

### Domain Invariants
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are preserved in test contexts.
- No fixture produces invalid UUIDs (e.g. nil UUID) unless explicitly requested for negative test scenarios.

### Events / Outbox Side Effects
- Mock transport implements `EventTransport` and `publish_contract` faithfully.
- Event recording records both domain events and contract event envelopes.

### Errors / Failure Codes
- Assertion macros preserve typed domain error classes without loss of error metadata.
