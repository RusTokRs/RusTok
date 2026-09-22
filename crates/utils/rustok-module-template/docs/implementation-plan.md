# Implementation Plan for `rustok-module-template`

## Scope

Own the independently versioned canonical Rust component project template.
Filesystem mutation, Cargo execution, builds, publication, admission, and
runtime execution remain outside this crate.

The template may document host integration boundaries, but it must not invent
host capabilities or link host-only crates into a standalone Component Model
guest.

## Current State

- [x] Render a standalone Rust edition 2024 `cdylib` project targeting native
  `wasm32-wasip2`.
- [x] Pin the Rust toolchain and exact `rustok-module-sdk` release.
- [x] Render the canonical source manifest with bundled command and settings
  schemas, permissions, localization, and a brokered Events capability.
- [x] Render a fail-closed dependency policy and host-side SDK contract test.
- [x] Validate the rendered source manifest through the owner contract.
- [x] Render and validate a bounded local Component sandbox scenario with an
  exact Events grant, fixture response, input, limits, and expected output.
- [x] Render an Index compatibility guide that distinguishes standalone guests
  from native source modules and rejects direct `rustok-index`/PostgreSQL access.
- [x] Guard the absence of an invented `platform.index` capability while no
  admitted standalone Index broker contract exists.
- [x] Connect create-new writes and lockfile generation through the owner-local
  `rustok module init` CLI provider.
- [ ] Compile the rendered fixture with the pinned native component target in
  isolated CI evidence.
- [ ] Add executable standalone Index integration only after a host-owned,
  versioned, capability-constrained broker contract is implemented and admitted.

## Completion Condition

The template is complete when the canonical CLI creates a locked standalone
project, generated documentation accurately describes every available host
boundary, and isolated CI proves that the exact rendered fixture exports only
the admitted WIT surface.
