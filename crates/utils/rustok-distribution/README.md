# rustok-distribution

## Purpose

`rustok-distribution` assembles the module registry selected by a RusToK
distribution build.

## Responsibilities

- Own compile-time module selection and `ModuleRegistry` composition.
- Own explicit compile-time cross-module adapter selection. The
  `ai-translation` feature requires both owner modules and publishes only the
  Translation-owned lazy machine-translation factory through neutral runtime
  extensions. The production server profile selects this bridge; when the
  deployment result keyring is absent, the factory resolves to no machine
  provider and leaves manual Translation workflows available.
- Generate the deterministic Cargo dependency fragment, promoted-module
  registry source, and machine-readable composition manifest consumed only in
  immutable static-distribution CI workspaces.
- Provide the same selected registry to HTTP hosts and standalone operations.
- Keep routing, lifecycle, command providers and domain logic outside this crate.
- Keep executable hosts capability-neutral: they call
  `build_runtime_extensions(...)` and never import selected bridge types.

## Interactions

- `apps/server` uses the registry for HTTP host composition.
- Standalone operational adapters can use the same registry without importing
  `apps/server`.
- `rustok-installer` receives the trusted `composition_identity()` through its
  CLI and HTTP hosts; installer plans never accept this identity from a wizard.
- Module CLI adapters remain owner-local and are aggregated separately by
  `rustok-cli-registry`.
- `rustok-modules` supplies an owner-validated, digest-pinned distribution work
  item to `generate_static_distribution`; the generator performs no source
  fetching, compilation, publication, or runtime mutation.
