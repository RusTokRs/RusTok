# `rustok-cli-core` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-cli-core` defines the fundamental abstractions, traits, and serialization contracts for the RusToK platform CLI ecosystem. It decouples command discovery, argument payload definitions, and execution lifecycles from terminal argument parsing, terminal output rendering, and concrete database or network runtimes.

Key responsibilities:
- Command metadata specification (`CommandDescriptor`).
- Unified execution payload with JSON arguments and dry-run flag (`CommandRequest`).
- Structured outcome model with exit code, message, and structured data (`CommandOutcome`).
- Typed domain error taxonomy (`CliCoreError`, `CliCoreResult`).
- Asynchronous provider trait seam (`CommandProvider`).

## 2. Core Command Data Model & Serde Invariants

1. **`CommandDescriptor`**:
   - Holds command routing identity: `namespace` and `name`.
   - Declares human-readable `summary`.
   - `supports_dry_run`: Explicit capability flag indicating whether the command safely supports simulation without mutating system state.
   - Fully serializable and deserializable for machine-readable JSON inventory exports.
2. **`CommandRequest`**:
   - Represents the normalized command invocation payload.
   - `args`: Arbitrary JSON value holding normalized options and positionals.
   - `dry_run`: Explicit execution mode flag.
3. **`CommandOutcome`**:
   - `exit_code`: Numeric status code conforming to standard conventions (`0` for success, non-zero for errors).
   - `message`: Human-readable summary of command execution.
   - `data`: Optional structured JSON data payload rendered as pretty-printed JSON by CLI runners.

## 3. Asynchronous Provider Seam & Default Trait Behavior

`CommandProvider`:
- `fn commands(&self) -> Vec<CommandDescriptor>`: Exposes command descriptors contributed by this provider.
- `async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome>`:
  Executes the designated command. The default trait implementation enforces fail-closed behavior, returning `CliCoreError::UnknownCommand` if not overridden.

## 4. Dry-Run Policy & Exit Code Contracts

- **Dry-Run Enforcement**: Command runners must inspect `CommandDescriptor::supports_dry_run` before dispatching a request with `dry_run: true`. Any command invoked with dry-run that does not support it must return `CliCoreError::DryRunNotSupported`.
- **Exit Code Guarantees**: `is_success()` returns true if and only if `exit_code == 0`. Handled errors or syntax failures use non-zero exit codes.

## 5. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - Command names and namespaces are non-empty strings.
   - Descriptors and requests validate matching namespace and name routes.
2. **Branch Completeness & Anti-Swallowing**:
   - Errors are never suppressed; all failures are represented by typed `CliCoreError` variants (`UnknownCommand`, `InvalidInput`, `CommandFailed`, `DryRunNotSupported`).
3. **State Transition Integrity**:
   - Core types are pure, value-based data transfer objects with zero mutable shared state.
4. **Side-Effect Completeness**:
   - No filesystem, network, or terminal I/O is performed in `rustok-cli-core`.
5. **Algorithmic Correctness**:
   - Exact matching in `matches(&self, namespace: &str, name: &str)`.
   - Symmetric serialization and deserialization round trips for all core structs.
6. **Order of Operations & Concurrency**:
   - `CommandProvider` requires `Send + Sync` ensuring safe concurrent dispatch across Tokio async tasks.
