# rustok-modules-cli / CRATE_API

## Platform Capability Bindings
- Implements `CommandProvider` from `rustok-cli-core` for standalone module authoring CLI commands.
- Integrates with `rustok-build-source`, `rustok-module-sdk`, `rustok-module-template`, `rustok-runtime`, and `rustok-sandbox`.

## Domain Collaborations
- Collaborates with `rustok-modules` authoring build and publication controls.
- Registered by platform distributions and CLI tools via `rustok-module.toml`.

## Public surface
- `pub struct ModuleCommandProvider`: Implements `CommandProvider` for the `module` command namespace (`init`, `validate`, `test`, `build`, `package`, `publish`, `inspect`).
- `pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider>`: Registration factory.

## Minimum Contract Set

### Input DTOs/Commands
- `module init`: Scaffold new WASM Component module project.
- `module validate`: Validate module manifest, dependencies, and WIT interfaces.
- `module test`: Execute local sandbox scenarios.
- `module build` / `module package`: Deterministic offline compilation and USTAR CAS archiving.
- `module publish`: Stage module build receipts and trigger publication pipelines.

### Domain Invariants
- Deterministic archive creation enforces canonical file ordering, fixed timestamps, and length-framed limits.
- Offline and sandbox isolation for local compilation and verification runs.
- Does not bypass governance admission gates.

### Events / Outbox Side Effects
- Remote publication and build commands commit immutable outbox states through `rustok-modules`.

### Errors / Failure Codes
- Validation, build, and archive failures produce typed `CliCoreError` / `ModuleAuthoringBuildError` outcomes.
