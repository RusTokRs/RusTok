# rustok-module-template

## Purpose

`rustok-module-template` renders the canonical standalone Rust module
component source tree used by owner-local CLI authoring flows.

## Responsibilities

- Version the template independently from `rustok-module-sdk`.
- Render a strict source manifest, native WASI P2 project, pinned Rust
  toolchain, dependency policy, tests, localization, schemas, and a brokered
  capability example.
- Validate the rendered source manifest and bounded local sandbox scenario
  through their canonical contracts before returning files.

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
