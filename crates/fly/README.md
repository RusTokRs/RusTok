# Fly

`fly` is the framework-neutral editor engine defined by the Page Builder implementation plan.
It owns the canonical editor state, a lossless `grapesjs_v1` codec, component-tree commands,
undo/redo history, registries, validation, clipboard fragments, revision tracking, and
missing-provider preservation.

The crate deliberately has no dependency on Leptos, Dioxus, browser APIs, RusTok modules,
transport selection, persistence, or rich-text implementations.

## Compatibility contract

`GrapesJsV1Codec` decodes the project object produced by GrapesJS `getProjectData()` and emits a
semantically equivalent object suitable for `loadProjectData()`. Known fields have typed accessors;
unknown top-level, page, component, provider, plugin, and future fields are retained through
`serde(flatten)` or opaque values.

The first fixtures are repository-owned structural baselines. They are not labelled as real browser
captures; real captures remain a Phase 0 gate in the programme plan.

## Core flow

```text
project JSON
  -> GrapesJsV1Codec
  -> ProjectDocument
  -> FlyEditor commands/history/validation
  -> GrapesJsV1Codec
  -> project JSON
```

Consumer modules own persistence. Framework adapters and Page Builder surfaces consume this crate
rather than duplicating the project model.

## Compatibility evidence

`fixtures/grapesjs/manifest.json` distinguishes structural fixtures from real browser captures.
Run `node scripts/verify/verify-fly-grapesjs-roundtrip.mjs` from the repository root after installing
the Next admin dependencies and Playwright Chromium. The harness loads every fixture through the
actual GrapesJS `loadProjectData()`/`getProjectData()` cycle and rejects dropped fields or unstable
normalization. Structural fixtures do not satisfy the real-capture phase gate by themselves.
