# Fly

`fly` is the framework-neutral editor engine defined by the Page Builder implementation plan.
It owns the canonical editor state, a lossless `grapesjs` codec, component-tree commands,
undo/redo history, registries, validation, clipboard fragments, revision tracking, and
missing-provider preservation.

The crate deliberately has no dependency on Leptos, Dioxus, browser APIs, RusTok modules,
transport selection, persistence, or rich-text implementations.

## Compatibility contract

`GrapesJsCodec` decodes the project object produced by GrapesJS `getProjectData()` and emits a
semantically equivalent object suitable for `loadProjectData()`. Known fields have typed accessors;
unknown top-level, page, component, provider, plugin, and future fields are retained through
`serde(flatten)` or opaque values.

The fixture manifest distinguishes two independent contracts:

- browser fixtures are loaded through real GrapesJS in Chromium and must survive a bidirectional
  `loadProjectData()` / `getProjectData()` cycle;
- Fly-codec fixtures must survive exact Rust decode/encode even when GrapesJS itself does not retain
  the represented unknown extension fields.

`browser-current.json` is generated from `baseline.json` by the installed GrapesJS, preset and
Playwright Chromium versions. `unknown-provider.json` remains a Fly-codec fixture because GrapesJS
0.23.2 drops unknown top-level and extension fields; treating that loss as an allowed browser
normalization would hide a real boundary difference.

## Core flow

```text
project JSON
  -> GrapesJsCodec
  -> ProjectDocument
  -> FlyEditor commands/history/validation
  -> GrapesJsCodec
  -> project JSON
```

Consumer modules own persistence. Framework adapters and Page Builder surfaces consume this crate
rather than duplicating the project model.

## Compatibility evidence

After installing `apps/next-admin` dependencies and Playwright Chromium, run:

```bash
node scripts/capture/capture-fly-grapesjs-fixture.mjs
node scripts/verify/verify-fly-grapesjs-roundtrip.mjs
cargo test -p fly grapesjs_browser_capture_round_trip_is_exact
```

The capture command records exact GrapesJS, preset, Chromium, plugin, source-commit and timestamp
metadata. The verifier requires at least one real browser capture, rejects stale current-runtime
versions, runs every browser-compatible fixture through the actual editor, and allows structural
normalizations only when explicitly declared by fixture metadata. Real browser captures cannot
declare normalization exceptions.
