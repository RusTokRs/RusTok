# rustok-module-template

## Purpose

`rustok-module-template` renders the canonical standalone Rust module
component source tree used by owner-local CLI authoring flows.

## Responsibilities

- Version the template independently from `rustok-module-sdk`.
- Render a strict source manifest, native WASI P2 project, pinned Rust
  toolchain, dependency policy, tests, localization, schemas, and a brokered
  capability example.
- Render an Index compatibility guide that distinguishes standalone Component
  Model modules from native server modules and fails closed while no admitted
  `platform.index` or event-to-Index bridge exists.
- Validate the rendered source manifest and bounded local sandbox scenario
  through their canonical contracts before returning files.

## Index boundary

The generated crate is a standalone `wasm32-wasip2` component. It must not
link the host's `rustok-index` crate, access PostgreSQL, register
`ModuleRuntimeExtensions`, or treat `platform.events` publication as automatic
indexing. Native source-module integration is documented in
`../rustok-index/docs/module-source-integration.md`; generated components receive
`docs/index-integration.md` with the standalone boundary and readiness
checklist.

## Interactions

The module CLI writes the rendered files and generates `Cargo.lock`. The
isolated build worker validates the same source manifest and creates the final
artifact descriptor only after inspecting the built Component.

## Entry Points

- `render`
- `ModuleTemplateInput`
- `RenderedModule`
- `TEMPLATE_VERSION`

See the [local documentation](./docs/README.md).
