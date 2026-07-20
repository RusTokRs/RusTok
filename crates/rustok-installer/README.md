# rustok-installer

`rustok-installer` is the shared installer foundation for RusToK. It owns the
install plan, state-machine, secret-reference, receipt, checksum, and preflight
contracts that CLI, server HTTP, web wizard, and dev bootstrap wrappers should
reuse.

## Purpose

RusToK needs a hybrid installer: CLI-first for repeatable operations and CI/CD,
with a web wizard as a friendly first-run facade. This crate is the source of
truth for the shared installer semantics so those interfaces do not duplicate
bootstrap logic.

## Responsibilities

- Model install plans, profiles, database policy, seed profiles (including
  canonical parsing and the profile module set), and tenant enablement inputs.
- Track install state transitions and resumable step receipts.
- Redact secrets and distinguish secret references from plaintext setup input.
- Resolve local `env`, file, mounted-file and dotenv secret references through
  one reusable installer contract; external secret managers remain explicit
  adapter work.
- Provide deterministic checksums for idempotent step skipping.
- Provide preflight policy checks that are independent from any specific UI.
- With the default `seed-runtime` feature, define a consumer-owned seed
  workflow over narrow tenant, identity, role and module ports, without server
  model dependencies.
- Define versioned topology, trusted composition binding, and neutral
  distributed-role deployment hand-offs without build-provider dependencies.

## Interactions

- `apps/server` is a thin HTTP/setup-wizard adapter over these contracts; it
  must not own a second installation state machine.
- `rustok-installer-cli`, selected by `rustok-cli`, provides `install plan`,
  `install preflight`, `install apply`, `install status`, and `seed apply`
  through the shared executor; it does not import `apps/server`.
- `xtask install-dev` remains a dev convenience wrapper and will delegate to
  the platform CLI/executor rather than the production server binary.
- The current executor adapters resolve local secret refs (`env`, `file`,
  `mounted-file`, `dotenv`) during `apply`; external secret managers remain
  contract-level references until an external resolver is added.
- Migrations are owned by `rustok-migrations`; installer schema-selection
  must not pretend to omit module-owned schema while the server migrator is still
  globally composed.
- Durable SeaORM session and receipt storage is owned by
  `rustok-installer-persistence`; this foundation crate deliberately keeps no
  database adapter.
- `rustok-distribution` binds the selected composition revision/hash before
  preflight and apply. `rustok-build` owns role build/release execution; a host
  adapter will fulfill this crate's `InstallDeploymentPort` without moving
  deployment-provider code into the installer.

## Feature Boundary

- The feature-neutral crate surface contains install plans, state, receipts,
  preflight, deployment hand-offs, secret references, and executor ports. It is
  safe for browser clients that only consume installer contracts.
- `seed-runtime` is enabled by default for native server and CLI consumers. It
  adds the seed workflow and its platform role dependency.
- Browser consumers must disable default features; they do not execute tenant,
  identity, role, or module seed operations.

## Entry Points

The current foundation API is exposed from the crate root:

- `InstallPlan`
- `InstallState`
- `InstallStep`
- `InstallReceipt`
- `PreflightReport`
- `evaluate_preflight`

## Verification

```powershell
cargo test -p rustok-installer
cargo check -p rustok-installer --no-default-features
```
