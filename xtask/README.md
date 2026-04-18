# xtask

`xtask` is the workspace-owned operational CLI for RusToK. It keeps local platform and module contract checks in Rust so they are runnable on Windows without depending on Bash-only scripts.

## Purpose

The tool provides one stable entry point for repository maintenance tasks that are too project-specific for plain Cargo commands. Its primary job is to validate that `modules.toml`, per-module `rustok-module.toml` files, local module documentation, server wiring, UI wiring, permissions, and event-runtime contracts stay aligned with the actual code.

## Responsibilities

- Validate the central module composition contract in `modules.toml`.
- Validate each scoped path module from `modules.toml` against its publish, runtime, UI, documentation, dependency, permission, and server-wiring contracts.
- Build targeted local module smoke plans for `cargo xtask module test <slug>`.
- Generate server module registry artifacts from `modules.toml`.
- Provide operator commands for module publishing, staging, governance follow-up actions, owner transfers, yanking, and remote runner execution.
- Keep mandatory local audit paths Windows-safe where possible and leave Bash-only scripts as optional perimeter checks.

## Entry Points

Run commands from the repository root:

```powershell
cargo xtask validate-manifest
cargo xtask module validate
cargo xtask module validate <slug>
cargo xtask module test <slug>
cargo xtask generate-registry
cargo xtask list-modules
```

Registry/operator flows use the same binary:

```powershell
cargo xtask module publish <slug> --dry-run
cargo xtask module stage-run <slug> <request-id> <stage> --dry-run
cargo xtask module runner <runner-id> --dry-run --once
cargo xtask module request-changes <request-id> --dry-run --auth-token <token> --reason <text> --reason-code <code>
cargo xtask module hold <request-id> --dry-run --auth-token <token> --reason <text> --reason-code <code>
cargo xtask module resume <request-id> --dry-run --auth-token <token> --reason <text> --reason-code <code>
cargo xtask module owner-transfer <slug> <new-owner-user-id> --dry-run --auth-token <token> --reason <text> --reason-code <code>
cargo xtask module yank <slug> <version> --dry-run --auth-token <token> --reason <text> --reason-code <code>
```

Use `--dry-run` first for commands that would contact the registry or mutate publish lifecycle state.

## Module Coverage

`xtask` does not auto-discover platform modules from `crates/`. The source of truth is the `[modules]` table in `modules.toml`.

- `cargo xtask validate-manifest` validates the central composition contract for every module declared in `modules.toml`.
- `cargo xtask module validate` validates every local `source = "path"` module declared in `modules.toml`.
- `cargo xtask module validate <slug>` validates one declared module.
- `cargo xtask module test <slug>` builds and runs the targeted smoke plan for one declared module.
- A crate under `crates/` is treated as a support/capability crate until it is added to `modules.toml`.

Unknown slugs fail fast with `Unknown module slug`. Local path modules fail validation if `rustok-module.toml` is missing.

## Adding A Platform Module

To make a new crate visible to `xtask` as a platform module:

1. Create the crate, normally under `crates/rustok-<slug>/`, and ensure it is a Cargo workspace member.
2. Add the local module docs minimum: `README.md`, `docs/README.md`, and `docs/implementation-plan.md`.
3. Add `rustok-module.toml` with matching `module.slug`, `module.version`, `module.ui_classification`, dependency metadata, and `[crate].entry_type` when the crate implements `RusToKModule`.
4. Add the module to `[modules]` in `modules.toml`; use `required = true` only for core modules, otherwise keep it optional.
5. Keep dependencies synchronized across `modules.toml.depends_on`, `rustok-module.toml [dependencies]`, and `RusToKModule::dependencies()`.
6. For optional runtime modules, add the matching `mod-<slug>` feature and server wiring in `apps/server/Cargo.toml`.
   Regular optional modules wire `mod-<slug>` to `dep:<crate>`, while `capability_only` ghost modules may use an empty feature guard when the crate is already always linked as a shared server capability dependency.
7. For required runtime modules, register the module in `apps/server/src/modules/mod.rs`.
8. For module-owned UI, declare `[provides.admin_ui]` and/or `[provides.storefront_ui]` only when the corresponding UI sub-crate exists and is wired into the host.
9. Update central navigation docs: `docs/modules/_index.md`, `docs/modules/registry.md`, and UI package indexes when applicable.
10. Run the local preflight commands:

```powershell
cargo xtask validate-manifest
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

## Interactions

- Reads `modules.toml` as the central composition source of truth.
- Reads each path module's `Cargo.toml`, `rustok-module.toml`, `README.md`, `docs/README.md`, and `docs/implementation-plan.md`.
- Scans module source files for declared runtime entry types, permissions, transports, and event-listener registration paths.
- Checks server-side feature flags, generated module registry wiring, controller shims, and default-enabled module closure.
- Checks admin/storefront UI host wiring against module-owned `provides.admin_ui` and `provides.storefront_ui` metadata.
- Writes `apps/server/src/modules/generated.rs` only through `cargo xtask generate-registry`.
- Contacts registry HTTP endpoints only for live operator commands when `--dry-run` is not used.

## Verification

Use targeted checks when editing `xtask`:

```powershell
cargo check -p xtask
cargo test -p xtask
cargo xtask validate-manifest
cargo xtask module validate <slug>
```

For module work, run `cargo xtask module validate <slug>` before broader workspace checks. Run `cargo xtask module test <slug>` when the module's compile or targeted runtime smoke plan is part of the change.

## Related Documentation

- [Module manifest contract](../docs/modules/manifest.md)
- [Module system plan](../docs/modules/module-system-plan.md)
- [Verification guide](../docs/verification/README.md)
