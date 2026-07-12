# rustok-build

## Purpose

`rustok-build` owns platform build and release persistence contracts.

## Responsibilities

- Define build and release SeaORM models, status state machines, execution plans, runtime-mode intent, and executor reports.
- Deduplicate successful builds by the complete immutable execution identity, including selected artifact identity, compiled profile, and runtime mode, rather than module manifest alone.
- Build and execute Cargo/Trunk command specifications independently of the server host.
- Execute queued build plans from server workers or `rustok-cli` through explicit event, release-activation, and release-publication ports.

## Interactions

`apps/server` composes workers, event delivery, and deployment adapters around these contracts. `RoleBuildPlan` binds compiled surfaces to `BuildRuntimeMode`, so deployment adapters can pass the same runtime intent to every release target. `ReleasePublisherPort` keeps filesystem, HTTP, and container rollout execution in a host adapter while installer/CLI orchestration consumes a typed owner contract.

See [docs](docs/README.md).
