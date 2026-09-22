# Categorized Workspace Layout for Crates

- Date: 2026-09-06
- Status: Accepted

## Context

The `crates/` directory currently contains 136 packages in a single flat directory. This causes several architectural and developer-experience issues:

1. **Semantic ambiguity**: platform foundation libraries (`rustok-core`, `rustok-api`, `rustok-events`), UI libraries (`leptos-ui`, `leptos-table`), platform modules (`rustok-product`, `rustok-auth`), operational utilities (`rustok-cli`, `rustok-test-utils`), and workers (`rustok-sandbox`, `rustok-artifact-*`) all reside on the exact same directory level.
2. **"Core library vs Core module" confusion**: `rustok-core` (a foundation runtime library) is frequently confused with "Core modules" from `modules.toml` because they share the flat `crates/` directory and common naming.
3. **Module taxonomy cohesion**: In RusToK architecture ([`docs/architecture/principles.md`](../docs/architecture/principles.md)), platform modules are classified as `Core` (required) or `Optional`. However, this is a runtime configuration property in `modules.toml` (`required = true / false`) and code (`ModuleKind`), not a reason to split modules across different filesystem folders. Reclassifying a module must never force directory moves. External integrations (such as `ai`, `mcp`, `iggy`) are also full-fledged modules with permissions, events, and adapters, and belong together with all other modules.

## Decision

1. Reorganize `crates/` into five canonical top-level categories:
   - `crates/modules/`: **All platform modules** (both `Core` and `Optional`, including commerce, content, community, and integration modules like `ai` and `mcp`). Sub-crates owned by a module (`admin`, `storefront`, `cli`) remain inside that module's directory.
   - `crates/libs/`: **Platform foundation libraries** (`rustok-core`, `rustok-api`, `rustok-events`, `rustok-telemetry`, `rustok-runtime`, `rustok-web`, `rustok-fba`). These are non-module contract and runtime libraries.
   - `crates/ui/`: **Reusable UI libraries and frontend primitives** (`leptos-ui`, `leptos-table`, `leptos-forms`, `rustok-ui-core`, `rustok-ui-i18n`, `rustok-graphql`, `fly-*`).
   - `crates/utils/`: **Tooling, CLI, test utilities, installers, and builders** (`rustok-cli`, `rustok-test-utils`, `rustok-build`, `rustok-installer`, `rustok-storage`, `rustok-secrets`, `rustok-migrations`).
   - `crates/workers/`: **Isolated background workers and build/node agents** (`rustok-sandbox`, `rustok-artifact-node-*`, `rustok-module-build-*`, `rustok-verification-*`, `rustok-static-distribution-worker`).

2. Update root `Cargo.toml` workspace members to reflect the new hierarchy:
   ```toml
   [workspace]
   members = [
       "apps/*",
       "crates/libs/*",
       "crates/modules/*",
       "crates/ui/*",
       "crates/utils/*",
       "crates/workers/*",
   ]
   ```

3. Update `modules.toml` module paths from `crates/<name>` to `crates/modules/<name>`.

4. Update `xtask` manifest validation and module command runners to resolve module paths from `modules.toml` and workspace definitions.

5. Update documentation links in `docs/` and `README.md` files atomically in the same change.

## Consequences

### Positives

- **Immediate structural clarity**: developers, contributors, and AI agents immediately understand the architectural layer of any crate from its folder path (`libs/`, `modules/`, `ui/`, `utils/`, `workers/`).
- **Elimination of concept confusion**: `rustok-core` is located in `crates/libs/rustok-core/`, making it obvious that it is a foundation library, not a platform module.
- **Stable module locations**: moving a module between `Core` and `Optional` status remains an in-place configuration change in `modules.toml` without filesystem churn.
- **Clean Cargo workspace navigation**: IDEs and repository tools present structured package categories instead of an unorganized list of 136 directories.

### Trade-offs and Follow-up Actions

- **Large atomic cutover**: requires updating path dependencies in `Cargo.toml`, module paths in `modules.toml`, resolver logic in `xtask`, and documentation links in `docs/`.
- Must be executed atomically following repository zero-legacy policy.
