# Cargo Scoped Compilation and Verification Policy

## 1. Universal Mandate: Strictly Scoped Verification
RusToK is a massive modular monorepo containing 80+ workspace packages and modules.
Running workspace-wide compilation wastes significant resources and compilation time.

- **STRICTLY FORBIDDEN:**
  - Running bare `cargo check`, `cargo build`, `cargo test`, `cargo clippy`, `cargo run` without `-p <package>` or `--manifest-path`.
  - Passing `--workspace` or `--all` flags unless the user explicitly and literally asks for a full workspace test/build.
  - Compiling or checking `rustok-server` (`-p rustok-server`) when modifying a domain module or library. The server crate features pull all 40+ domain modules. Modules in RusToK (FBA/FFA) are sovereign and self-contained; they must be verified independently.

- **MANDATORY:**
  - Always identify the exact crate being modified by inspecting its `Cargo.toml` (e.g. `crates/modules/rustok-messenger/admin` has package name `rustok-messenger-admin`).
  - Run verification scoped strictly to that package:
    ```powershell
    cargo check -p <package_name>
    cargo test -p <package_name> --lib
    ```
  - When running `run_command`, either pass `-p <package_name>` from workspace root, or set `Cwd` to the directory containing the crate's `Cargo.toml`.

## 2. FFA (Fluid Frontend Architecture) Verification Rules
1. **Core logic:** All domain logic and view-models live in the framework-agnostic `core/` package and MUST be tested using pure unit tests:
   ```powershell
   cargo test -p <module-name> --lib
   ```
2. **UI Adapters:** When editing Leptos/Dioxus components in `admin/` or `storefront/`, check only the specific adapter package:
   ```powershell
   cargo check -p <module-name>-admin
   # or
   cargo check -p <module-name>-storefront
   ```

## 3. Fast Failure Guardrail
The workspace root [Cargo.toml](file:///d:/RusTok/Cargo.toml) has `default-members = []`. Any bare `cargo check` or `cargo test` in root will immediately error with:
`error: manifest path contains no package: The manifest is virtual, and the workspace has no members.`
When you see this error, do NOT attempt to bypass it by adding `--workspace`. Instead, immediately specify the relevant package using `-p <package_name>`.
