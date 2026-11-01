# rustok-module-template — Crate API

Pure renderer for the canonical Rust module component source tree.

## Public surface

- `ModuleTemplateInput` — validated template input (slug, version, display name, ...).
- `RenderedModule` — rendered output with `files()` and `file(path)` accessors.
- `TemplateFile` — a single rendered file (path + contents).
- `ModuleTemplateError` — rendering failures.
- `render(input)` — renders the canonical source tree.
- `TEMPLATE_VERSION` / `RUST_TOOLCHAIN` — pinned template identity and toolchain.

## Invariants

- Rendering is pure: no filesystem access, no network, no clocks; identical inputs produce identical outputs.
- The rendered tree is the canonical authoring scaffold; owner-local CLI flows must not post-process it beyond copying files.
