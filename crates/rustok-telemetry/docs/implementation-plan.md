# Implementation plan for `rustok-telemetry`

Status: telemetry foundation crate already exists, but local documentation and the
boundary contract need to be maintained as rigorously as for other shared modules.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Synchronize the plan with the current code and select the first incomplete item.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Scope of work

- maintain `rustok-telemetry` as a shared observability foundation layer;
- synchronize telemetry helpers, wiring expectations and local docs;
- do not pull domain-specific observability logic into the foundation crate.

## Current state

- crate is already a shared dependency for observability-related wiring;
- shared telemetry helpers already form part of the platform baseline;
- host and module integrations should rely on a single foundation contract;
- local docs and root `README.md` must remain part of the module-standard path.

## Stages

### 1. Contract stability

- [x] lock `rustok-telemetry` as a shared observability foundation;
- [x] keep shared helpers separate from domain-specific metrics semantics;
- [ ] maintain sync between public surface, host wiring and module metadata.

### 2. Boundary hardening

- [ ] continue extracting shared telemetry helpers from host-specific layers if they are truly shared;
- [ ] do not pull module-owned metrics/runbook semantics here;
- [ ] cover new foundation contracts with targeted tests and compatibility checks;
- [ ] contract tests cover all public use-cases of the telemetry foundation.

### 3. Operability

- [ ] document observability foundation changes concurrently with runtime surface changes;
- [ ] keep local docs and `README.md` synchronized;
- [ ] update host/verification docs if shared wiring expectations change.

## Verification

- `cargo xtask module validate telemetry`
- `cargo xtask module test telemetry`
- targeted tests for telemetry helpers, metrics/tracing wiring and compatibility contracts

## Update rules

1. When changing telemetry foundation contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing shared observability wiring, update related host and verification docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
