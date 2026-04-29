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

- Model install plans, profiles, database policy, seed profiles, and tenant
  enablement inputs.
- Track install state transitions and resumable step receipts.
- Redact secrets and distinguish secret references from plaintext setup input.
- Provide deterministic checksums for idempotent step skipping.
- Provide preflight policy checks that are independent from any specific UI.

## Interactions

- `apps/server` calls this crate from `rustok-server install ...`; future
  `/api/install/*` endpoints should reuse the same contracts.
- `xtask install-dev` remains a dev convenience wrapper and delegates bootstrap
  to `rustok-server install apply`.
- The current CLI adapter resolves local secret refs (`env`, `file`,
  `mounted-file`, `dotenv`) during `apply`; external secret managers remain
  contract-level references until an external resolver is added.
- Migrations remain owned by `apps/server/migration`; installer schema-selection
  must not pretend to omit module-owned schema while the server migrator is still
  globally composed.

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
```
