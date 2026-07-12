# rustok-distribution

## Purpose

`rustok-distribution` assembles the module registry selected by a RusToK
distribution build.

## Responsibilities

- Own compile-time module selection and `ModuleRegistry` composition.
- Provide the same selected registry to HTTP hosts and standalone operations.
- Keep routing, lifecycle, command providers and domain logic outside this crate.

## Interactions

- `apps/server` uses the registry for HTTP host composition.
- Standalone operational adapters can use the same registry without importing
  `apps/server`.
- Module CLI adapters remain owner-local and are aggregated separately by
  `rustok-cli-registry`.
