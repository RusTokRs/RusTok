# Page Builder generic admin accessibility actualization — 2026-08-10

Status: `generic-editor-control-accessibility-source-ready / source-guard-ready / browser-accessibility-evidence-pending`.

Base rechecked: `main@3b1ba79619a9c37f7bc90fb773843c7287d2d4ff`.

## Purpose

The provider-health, Pages gate and Forum Wave admission source chain is already complete through maintainer-execution boundaries. The remaining central Page Builder plan still called out generic typed asset/control surfaces and accessibility. Source inspection found a narrower real implementation gap: the controls existed and were capability-gated, but several WASM editor fields were only visually labelled, several asset inputs relied on placeholders, repeated actions exposed only generic names such as `Use`, `Remove`, `Add` or `Drag`, and selected page/layer buttons communicated selection only through CSS.

This slice closes that **source accessibility semantics** gap without changing Fly commands, capability policy, owner transports, persistence, provider health or rollout behavior.

## Source changes

### Properties, styles and responsive styles

- generic property/style/responsive inputs and selects now have programmatic names through enclosing labels or explicit `aria-label` where a sibling action prevents valid label nesting;
- tag/content/attribute actions retain their visible localized labels while repeated or ambiguous actions gain scoped accessible names;
- attribute removal identifies the concrete attribute in its accessible name.

### Assets

- asset id and URL inputs no longer depend on placeholders for their accessible names; they are enclosed by localized labels;
- the Add action is named for the Assets surface;
- repeated Use/Remove actions include the concrete asset name/id in their accessible names;
- existing `Assets` and cross-capability `Properties` gates remain unchanged.

The SSR asset form already encloses its visible fields/select in labels and remains unchanged by this WASM editor slice.

### Traits

- each generic trait control uses the stable `TraitSchema.id` as a source-visible row identity and the owner-provided trait label as its accessible name;
- Boolean, Select, Multiline, Number, Text and URL controls share the same naming rule;
- repeated Apply/Clear actions include the trait label in their accessible names.

No trait schema, validation or patch semantics moved into the UI.

### Pages, palette and layers

- active page and active layer buttons expose `aria-pressed` in addition to visual styling;
- the add-page name input has an explicit localized accessible name rather than relying on its placeholder;
- current page name/id fields are enclosed by their visible labels;
- repeated palette Add/Drag buttons include the concrete block label in their accessible names.

Selection, insertion, drag/drop and page lifecycle commands are unchanged.

## Anti-drift source guard

`scripts/verify/verify-page-builder-admin-accessibility.mjs` source-locks the generic editor accessibility boundary across:

```text
asset_section.rs
style_section.rs
properties_section.rs
responsive_styles.rs
trait_panel.rs
page_manager.rs
palette_layers.rs
toolbar.rs
```

The guard requires programmatic field names, object-scoped repeated action names, selected-state semantics and the existing toolbar/live-status accessibility baseline. It rejects the former placeholder-only asset fields and selected visual-only label patterns.

## Plan reconciliation

The central plan item `Complete remaining generic typed asset/control surfaces and accessibility evidence` is now split conceptually:

- generic typed controls and programmatic accessibility semantics: **source-ready**;
- executable keyboard/screen-reader/browser/accessibility evidence: **maintainer execution pending**.

No runtime/browser evidence is claimed by source inspection.

## Boundaries

This slice does not:

- change Fly document, command, history, trait, style, asset or page semantics;
- alter capability gates or provider-health narrowing;
- add a second editor or alternate persistence path;
- change Pages or Forum contribution ownership;
- change SSR asset request validation;
- claim WCAG conformance from static source markers;
- execute a browser, screen reader, accessibility scanner, Playwright, Node verifier, Cargo command, formatter, build, workflow or CI;
- accept any provider-health, Pages gate or Forum Wave evidence;
- promote FFA/FBA.

## Next cursor

The generic editor accessibility source gap is closed. Remaining accessibility work is executed evidence: keyboard navigation/focus behavior, accessible-name/state inspection, disabled capability behavior and browser/screen-reader checks on the built admin surface.

The broader rollout cursor remains maintainer-owned:

```text
exact provider-health execution
-> Pages gate decision
-> Forum evidence/admission
-> observed control-plane Wave
-> accepted rollout evidence
```

Tests and verifiers were intentionally not run in this source-authoring slice.
