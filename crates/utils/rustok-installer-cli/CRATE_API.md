# CRATE_API: rustok-installer-cli

## 1. Architectural Role & Boundary Enforcement

`rustok-installer-cli` is the command adapter connecting the RusToK CLI framework (`rustok-cli-core`) to installer domain services (`rustok-installer`, `rustok-starter`, `rustok-installer-persistence`):
- **Terminal Adapter Only:** Owns command line option extraction, input validation, and result formatting. Contains zero custom installation state machine or database persistence logic.
- **Fail-Closed Execution:** Rejects production runs for developer-only workflows (e.g. `seed apply`). Requires explicit `--root` and cryptographically verified base-distribution receipts for install workflows.
- **Provider Seam:** Dispatches commands through `InstallerCommandProvider` implementing the canonical `CommandProvider` trait from `rustok-cli-core`.

---

## 2. Public Types & Data Structures

- `InstallerCommandProvider`: Command provider struct encapsulating `RuntimeComposition`.
  - Methods:
    - `pub fn new(runtime: RuntimeComposition) -> Self`: Constructs the provider with a runtime handle.
    - `pub fn runtime(&self) -> &RuntimeComposition`: Accesses the composed runtime.
  - Trait Implementations:
    - `CommandProvider`: Exposes command descriptors and asynchronous execution.
    - `Debug`: Manual formatting displaying non-exhaustive struct details.
- `command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider>`:
  - Canonical factory function for registering installer CLI commands into `rustok-cli`.

---

## 3. Public Ports, Operations & Error Contracts

### Exposed Commands

1. **`seed apply` (with `--dry-run`):**
   - Options: `--environment` (required: `local`, `demo`, `test`; `production` is strictly rejected), `--profile` (`none`, `minimal`, `dev`), `--password` (or `SEED_ADMIN_PASSWORD`), `--demo-customer-password`, `--tenant_name`, `--tenant_slug`, `--starter`.
   - Behavior: Validates and runs seed profile; imports optional starter blueprint. In dry-run mode, validates configuration without database mutations.

2. **`starter import` (with `--dry-run`):**
   - Options: `--tenant_slug`, `--blueprint` / `--starter` (e.g. `default`), `--file` (custom JSON blueprint path).
   - Behavior: Validates and imports blueprint into an existing tenant.

3. **`install plan`:**
   - Options: `--root`, `--database_url` or `--database_secret_ref`, `--admin_password` or `--admin_password_ref`, `--base-distribution-receipt`, `--base-distribution-public-key`.
   - Behavior: Validates input, verifies receipt signature and composition match, and renders redacted install plan without database access.

4. **`install preflight`:**
   - Behavior: Evaluates preflight policy checks (e.g. database engine, secret mode, topology) and reports diagnostic issues without modifying state.

5. **`install apply` (with `--dry-run`):**
   - Options: Same as `install plan` plus `--lock-owner`, `--lock-ttl-secs`, `--pg-admin-url`.
   - Behavior: Executes the complete installer state machine through `rustok-installer`. Dry-run mode evaluates and returns preflight report without state mutations.

6. **`install status`:**
   - Behavior: Reads and displays the latest durable installer session from persistence.

### Error Taxonomy
- `CliCoreError::InvalidInput`: Missing required options, invalid integers, unsupported engines/environments, or invalid receipt paths.
- `CliCoreError::CommandFailed`: Preflight failure, DB connection failure, or seed/migration errors.
- `CliCoreError::UnknownCommand`: Unrecognized namespace/name dispatches.

---

## 4. Usage Patterns & Invariants

- **Dry-Run Non-Mutation:** All commands supporting dry-run must return early without touching database connections or executing filesystem mutations.
- **Secret Redaction:** Commands displaying plan outputs must invoke `redact_install_plan` to guarantee plaintext passwords and connection URLs never appear in terminal output.
- **Supply Authority Integrity:** Distributed install invocations strictly verify Ed25519 signatures and composition hash equivalence of admitted static distribution receipts.

---

## 5. Verification & Testing Matrix

- `src/contract_tests.rs`: Comprehensive test suite verifying provider construction, descriptor registration, dry-run execution, option validation, and error paths without requiring a live database.
- `tests`: Regression tests validating CLI execution against simulated runtime compositions.

