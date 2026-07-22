# rustok-distribution

## Purpose

`rustok-distribution` assembles the module registry selected by a RusToK
distribution build.

## Responsibilities

- Own compile-time module selection and `ModuleRegistry` composition.
- Generate the deterministic Cargo dependency fragment, promoted-module
  registry source, and machine-readable composition manifest consumed only in
  immutable static-distribution CI workspaces.
- Provide the same selected registry to HTTP hosts and standalone operations.
- Keep routing, lifecycle, command providers and domain logic outside this crate.

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
