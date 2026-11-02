# `rustok-cli` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-cli` is the terminal-facing platform CLI runner for RusToK maintenance, distribution, and module workflows.
It parses command-line arguments, enforces namespace dispatch policies, normalizes options and flags into structured JSON parameters, and coordinates execution across registered command providers without coupling to HTTP transport layers or absorbing domain business logic.

Key responsibilities:
- Command dispatch across namespaced providers (`CommandRegistry`, `CommandProvider`).
- Uniform terminal argument parsing and flag normalization (`parse_command_args`).
- Support for structured JSON inventory listing and human-readable help formatting (`render_command_list`, `render_command_list_json`).
- Deterministic exit representation and outcome rendering (`CliExit`, `CommandOutcome`).
- Non-blocking runtime initialization from environment or in-memory settings (`run_with_args`, `run_with_environment`, `run_with_runtime`).

## 2. Bounded Command Registry & Namespaced Dispatch Seam

To maintain clear subsystem boundaries and prevent conflicting command routes:
1. **Namespace Uniqueness**:
   - `CommandRegistry::from_providers` validates that every registered `(namespace, name)` tuple is unique across all configured providers.
   - Any duplicate command collision immediately fails registration with `CommandRegistryError::DuplicateCommand`.
2. **Provider Delegation Seam**:
   - Built-in commands (`core list`) are provided by `BuiltInProvider`.
   - Domain and operations commands are contributed through external distribution providers (`rustok-cli-registry`, `rustok-cli-platform`).
   - The CLI runner never inspects internal database models directly; all execution delegates to provider ports.

## 3. Argument Parsing, Normalization & Dry-Run Guarantees

1. **Option and Positional Normalization**:
   - `parse_command_args` converts `--key value` and `--key=value` into `options` object entries with underscores (`key_name`).
   - Positional arguments are preserved in sequence under `positionals`.
   - Trailing arguments after `--` are treated as literal positionals regardless of leading dashes.
2. **Dry-Run Policy**:
   - `--dry-run` is parsed into a first-class boolean flag on `CommandRequest`.
   - If a caller requests `--dry-run` on a command whose descriptor specifies `supports_dry_run: false`, execution is rejected with `CliCoreError::DryRunNotSupported` before invoking the provider.

## 4. Runtime Composition & Environment Bootstrapping

- `run_with_environment`: Discovers database connections, cache endpoints, and platform settings from process environment variables before dispatching commands.
- `run_with_args`: Defaults to lightweight runtime composition without database connectivity for help and offline operations.
- `run_with_runtime`: Accepts an explicit `RuntimeComposition` for hermetic testing and isolated integration environments.
- `CliExit`: Translates command outcomes into standard process exit codes (`0` for success, `2` for syntax or unknown command errors).

## 5. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - Options must not be empty (e.g. `--` or `--=val` rejected with typed syntax errors).
   - Unknown list options or missing values for `--namespace` return explicit error messages.
2. **Branch Completeness & Anti-Swallowing**:
   - All parser and provider errors formatted with contextual usage text and returned as `CliExit::failure`.
   - Provider outcomes with non-null JSON data are safely serialized; serialization failures return typed errors rather than panics.
3. **State Transition Integrity**:
   - Execution is stateless and idempotent; each command invocation operates on an independent `CommandRequest`.
   - Dry-run mode guaranteed to prevent side effects on unsupported commands.
4. **Side-Effect Completeness**:
   - Output channels (`stdout`, `stderr`) explicitly partitioned in `CliExit`.
   - Structured JSON formatting appends trailing newlines for POSIX compliance.
5. **Algorithmic & Calculation Precision**:
   - Deterministic sorting of command inventory by namespace and name in `collect_commands`.
   - Stable JSON serialization in `render_command_list_json`.
6. **Order of Operations & Concurrency**:
   - Namespace validation and dry-run compatibility verified prior to provider execution.
   - Provider lookup follows registry order without TOCTOU race hazards.
