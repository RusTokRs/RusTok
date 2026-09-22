# rustok-build

## Purpose

`rustok-build` owns immutable platform build-plan construction, queued build
execution, and read-only build-history contracts. It owns no production release
head, deployment, activation, or rollback authority.

## Responsibilities

- Define the build SeaORM model, status state machine, execution plans,
  runtime-mode intent, and executor reports.
- Deduplicate successful builds by the complete immutable execution identity, including selected artifact identity, compiled profile, and runtime mode, rather than module manifest alone.
- Build and execute Cargo/Trunk command specifications independently of the server host.
- Execute queued build plans through explicit build-event ports.
- Own bounded build-history and active-build reads so transports do not query
  build persistence entities directly.
- Expose the read-only `BuildControl`/`SharedBuildControl` port and map
  persistence into the framework-neutral `PlatformBuildSnapshot`.

## Interactions

`apps/server` composes build history and event delivery. `RoleBuildPlan` binds
compiled surfaces to `BuildRuntimeMode`. The
`rustok-static-distribution-worker` alone publishes the complete immutable role
bundle; `rustok-modules` alone admits it and owns desired/observed rollout,
serving selection, direct-predecessor recovery, and retention. Runtime
materialization is derived from `rustok-runtime::InstanceLayout` under the
operator-selected `<instance-root>`; this crate never selects a deployment
directory.

See [docs](docs/README.md).
