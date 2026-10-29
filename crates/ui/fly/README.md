# Fly — Framework-Neutral Visual Editor Monorepo

For full public interface and engine contract definitions, see [CRATE_API.md](CRATE_API.md).

`fly` is an extensible, framework-neutral visual page builder and component-editor engine for Rust.
Its core owns the project model, the lossless GrapesJS codec, commands, history, validation and SSR
rendering, with zero UI-framework coupling.

## Status

Read this before planning against the architecture diagram below: the layers differ sharply in maturity.

| Package | Path | Status | What exists today |
|---|---|---|---|
| `fly` | `.` | **Stable** | Project model, lossless GrapesJS codec, commands, history, registries, validation, SSR renderer, dynamic/binding/context runtime |
| `fly-ui` | `ui/` | **Stable** | `FlyUiStateMachine`, intents/effects, keymaps, capability policy, contribution model |
| `fly-web` | `web/` | **Beta** | Geometry, hit-testing, iframe bridge, real-DOM inline grants, cleanup registry |
| `fly-browser` | `browser/` | **Beta** | SSR JS bridge (`assets/fly-browser.js`) and its Rust-side config contract |
| `fly-leptos` | `leptos/` | **Foundation only** | Four layout shells (`FlyFullEditor`, `FlyInlineEditor`, `FlyPreview`, `FlyReadOnly`) that render a `<section>` wrapper, plus re-exports of `fly-web`. **No canvas, drag & drop, panels or state wiring.** |
| `fly-dioxus` | `dioxus/` | **Foundation only** | The same four layout shells for Dioxus 0.7. **No canvas, drag & drop, panels or state wiring.** |

Authoring surfaces that need a working editor today drive `fly` and `fly-ui` directly and render
through SSR plus `fly-browser`; see `crates/modules/rustok-page-builder/admin`.

### Extraction readiness

This directory is laid out so it can become an independent repository
(`github.com/rustok/fly`). Both of the blockers that previously prevented it are resolved:

- **Platform i18n dependency — inverted.** Locale identity goes through the `LocaleResolver`
  trait (`src/locale_resolver.rs`). `rustok-ui-i18n` is now an optional dependency behind the
  `platform-i18n` feature, which is **on by default**, so host builds are unchanged.
- **Workspace template — completed.** `standalone-Cargo.toml` supplies `[workspace.package]`,
  `[workspace.dependencies]` and `[workspace.lints]`, and
  `scripts/verify/verify-fly-standalone-workspace.mjs` fails when it drifts from the host.

### What a standalone build gives up

Building with `--no-default-features` substitutes `BasicLocaleResolver`, a self-contained
structural BCP-47 implementation. It canonicalizes subtag casing and peels specificity layers
correctly, but it has no CLDR data, so it **cannot infer likely subtags**:

| Input | With `platform-i18n` | Without |
|---|---|---|
| `zh-TW` fallback | `zh-TW`, `zh-Hant`, `zh` | `zh-TW`, `zh` |
| `iw` | canonicalized to `he` | left as `iw` |

A project relying on script-level translation fallback would silently select different
translations. If full CLDR behaviour is needed standalone, implement `LocaleResolver` over
`icu_locid` and install that instead. The `fly` crate is compiled and tested in both
configurations by CI.

## Checking a change

The workspace has dozens of crates and `scripts/verify` has over 1300 scripts, so "run the tests"
is not a useful instruction for a Fly change. One command runs exactly what Fly CI runs:

```bash
scripts/fly-check.sh            # everything
scripts/fly-check.sh gates      # source guards only: node, no toolchain, ~2 seconds
scripts/fly-check.sh test       # crate tests
scripts/fly-check.sh features   # non-default feature combinations, including wasm32
scripts/fly-check.sh lint       # clippy -D warnings, rustfmt
scripts/fly-check.sh consumers  # the Page Builder / Pages crates that depend on Fly
```

It does not stop at the first failure: one run reports every problem, with the tail of each
failing log, so a fix cycle is one pass rather than one pass per defect. Steps that need a
toolchain you do not have are skipped rather than failed, so `gates` is useful on its own.

## Layout

```text
crates/ui/fly/
├── Cargo.toml                  <-- Root crate `fly` (AST, Document model, Codec)
├── standalone-Cargo.toml       <-- Workspace template when used as independent repo
├── README.md                   <-- Monorepo documentation
│
├── src/                        <-- Core AST, GrapesJS codec, revision tracking, validation
├── fixtures/                   <-- Bidirectional GrapesJS round-trip test fixtures
│
├── ui/                         <-- `fly-ui`: State machine, UiIntent, keymaps, capabilities
│   ├── Cargo.toml
│   └── src/
│
├── web/                        <-- `fly-web`: DOM geometry, hit-testing, iframe bridge, real DOM inline
│   ├── Cargo.toml
│   └── src/
│
├── browser/                    <-- `fly-browser`: Standalone JS bridge asset for SSR
│   ├── Cargo.toml
│   ├── assets/
│   └── src/
│
├── leptos/                     <-- `fly-leptos`: layout shells for Leptos 0.8 (foundation only)
│   ├── Cargo.toml
│   └── src/
│
└── dioxus/                     <-- `fly-dioxus`: layout shells for Dioxus 0.7 (foundation only)
    ├── Cargo.toml
    └── src/
```

---

## Architecture: Fluid Frontend Architecture (FFA)

```
┌─────────────────────────────────────────────────────────────┐
│                     UI Adapters Layer                       │
│     fly-leptos (Leptos 0.8)       fly-dioxus (Dioxus 0.7)   │
│     foundation only               foundation only          │
│     - FlyFullEditor  (shell)      - FlyFullEditor  (shell)  │
│     - FlyInlineEditor (shell)     - FlyInlineEditor (shell) │
│     - FlyPreview     (shell)      - FlyPreview     (shell)  │
│     - FlyReadOnly    (shell)      - FlyReadOnly    (shell)  │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│             fly-web (Browser & DOM Runtime)                 │
│  - Geometry: BrowserPoint, BrowserRect, CoordinateTransform │
│  - Hit-Testing: hit_test_drop_targets, DropPosition         │
│  - Iframe Bridge: IframeBridgeMessage, IframeSubscription   │
│  - Real DOM Inline: AuthenticatedInlineEditGrant            │
│  - Event Lifecycle: EventListenerHandle, CleanupRegistry    │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│              fly-ui (State Machine & Intents)               │
│  - FlyUiStateMachine, UiIntent, CommandCapability           │
│  - Shortcut maps, Presentation mode, RBAC capability table  │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│                fly (Core Domain & Codecs)                   │
│  - AST: ComponentNode, PageDocument, ProjectFragment        │
│  - Codecs: Lossless GrapesJS bidirectional encode/decode    │
│  - History: Reversible command transactions, project hashes │
│  - Validation: TraitSchemaRegistry, schema validators       │
└─────────────────────────────────────────────────────────────┘
```

---

## Packages in this Monorepo

| Package | Path | Responsibility | Dependencies |
|---|---|---|---|
| `fly` | `.` | Core AST, GrapesJS codec, document model, revision tree | Pure Rust |
| `fly-ui` | `ui/` | Presentation-neutral state machine, intents, shortcuts, capabilities | `fly` |
| `fly-web` | `web/` | Browser geometry, hit-testing, iframe bridge, real DOM inline | `fly`, `fly-ui`, `web-sys` (WASM) |
| `fly-browser` | `browser/` | Standalone JS bridge asset for classic SSR | Pure Rust |
| `fly-leptos` | `leptos/` | Leptos 0.8 layout shells (foundation only) | `fly-web`, `leptos` |
| `fly-dioxus` | `dioxus/` | Dioxus 0.7 layout shells (foundation only) | `fly-web`, `dioxus` |

---

## Security model

- **Integrity vs. change detection.** `ProjectHash` is FNV-1a 64: a cheap fingerprint for dirty
  tracking and optimistic concurrency only. Every gate that answers *"is this payload the one that
  was approved?"* uses `ContentDigest` (SHA-256) — see `ProjectSnapshot::content_digest`,
  `ProjectBundle::content_digest` and `BundleDecodePolicy::verified()`.
- **Identifiers are allow-listed.** Page and component ids must match
  `[A-Za-z0-9_.:-]{1,128}` (`validate_identifier`). They are interpolated into HTML attributes and
  into CSS selectors inside a raw `<style>` element; escaping is the primary defence and the
  charset is the backstop. `ensure_stable_ids` rewrites non-conforming ids rather than preserving
  them, so a bad id can never deadlock the editor.
- **Decode is depth-bounded.** `GrapesJsCodec` rejects payloads nested deeper than
  `MAXIMUM_DECODE_DEPTH` before any recursive code touches them.
- **Rendering escapes at every sink.** HTML text, attribute values, CSS attribute selectors and the
  `<style>` element body each have their own escape; URLs are filtered by `RenderPolicy`.

## Site symbols

`SymbolDescriptor` definitions are stored in `project.extensions.flySymbols`; a
component with `symbolId` is a child-free reference. `EditorCommand::Symbol`
(upsert/remove) manages definitions, and `convert_component_to_symbol`/
`insert_symbol_instance` generate ordinary editor commands for the authoring UI.
`resolve_symbol_instances` expands referenced definitions with deterministic,
unique ids before rendering or publication; unknown ids, invalid definitions,
and reference cycles fail closed. Definition content has its own size and count
caps. The `fly` crate has no cross-page store: consumer modules such as
`rustok-pages` own the site catalog, revision checks, and re-publish workflow
([ADR](../../../DECISIONS/2026-10-09-site-symbols-shared-definitions.md)).

## Known Limitations / Pending Implementation

`CHANGELOG.md` carries the release-level *Known gaps*. This section lists the boundaries an
integrator can hit today, each implemented to a stated point and deliberately no further. An
unrecorded incompleteness is treated as a defect in this repository, so every entry below names
the owner of the missing half.

- **`raw_html` has no raw HTML sink.** The type exists so the codec can round-trip documents
  authored elsewhere, and the renderer escapes its `content` like any other text: no code path
  emits author-supplied markup, and none is planned while embeds stay out of the static publish
  policy (`DANGEROUS_COMPONENT_TYPES` in `rustok-page-builder`). The palette block is labelled
  `Raw HTML (escaped as text)` and validation reports `raw_html_not_rendered` (warning) for a
  document that carries visible content in one.
- **Provider interaction capabilities are a contract the host fills.** `fly` ships
  `InteractionCapabilityRegistry`, `evaluate_runtime_publish_gate_with_capabilities` and
  `FormProviderEndpointResolver`; a bare `fly` consumer keeps `InteractionCapabilityPolicy::Allow`
  and marker-only form rendering. The RusTok host does fill it:
  `rustok-page-builder::interaction_providers` registers the `forms`/`submit` form capability
  (routed to `rustok-forms`), blocks the gate for unregistered provider forms, and binds registered
  providers to a native `action` in the storefront, preview, static-artifact and scenario-matrix
  renders through `HostFormProviderEndpoints`. Release-baseline captures stay marker-only (baseline
  and candidate are compared inside the same renderer). Draft editing and GrapesJS round-tripping
  stay permissive by design.
- **The action runtime covers events, behaviours, and allow-listed animations.**
  `fly-browser` ships `fly-actions.js` (`FLY_ACTIONS_JS`): it turns `emit_event` and
  `provider_action` clicks into `fly:action` CustomEvents on the document, and implements the
  `toggle_visibility`, `scroll_to`, and `animate` actions (aria-synchronized visibility,
  smooth scroll, closed Web Animations keyframe sets) against the server-materialized
  `data-fly-action` contract. Hosts include the asset on published pages and listen for
  `fly:action`; without the asset the buttons stay inert by design. Interactive primitives
  such as tabs (`switch_tab`), accordion (`toggle_accordion`), and slider / carousel (`switch_slide`
  with swipe/touch gestures and dot indicators) are implemented in `builtin_blocks` and `fly-actions.js`.
- **Rich text renders through the host's capability, not Fly's fallback.** A component carrying
  `flyRichText` stores a `RichTextPayload`; `fly` validates its shape, projects plain text, and
  can render a conservative escaped fallback. Production HTML must come from the host binding
  the `RichTextHtmlRenderer` port (in RusTok: `rustok-content::richtext` via
  `rustok-pages-storefront` under `ssr`, per `DECISIONS/2026-07-22`). A consumer that binds no
  renderer gets the fallback projection — safe, but deliberately not the rich-text capability.
  Browser editing of the payload (Tiptap/ProseMirror) remains host-side preview tooling; the
  save path re-enters through the same payload contract and validation.
- **The framework adapters have workspaces; the page-builder admin has not migrated yet.**
  `fly-leptos` and `fly-dioxus` ship a real `FlyEditorWorkspace` (palette, layer tree, canvas
  preview, undo/redo) on the shared `fly`/`fly-ui` contracts, alongside the original section
  shells. The RusTok page-builder admin still runs its own SSR/WASM editor panels (audit P0-2);
  converging it onto the adapter workspaces is planned follow-up work, not done. Pointer
  geometry and drag & drop remain `fly-web` (wasm) concerns layered on top of these
  workspaces.
- **Session and CSRF posture of the standalone admin.** The intent route authenticates with the
  same-origin session cookie; issuing that session server-side and adding an explicit server-side
  CSRF check are the admin shell's change, tracked as `CLEAN-060` — see `SECURITY.md`.
- **Embeds exist for allow-listed providers; there is no custom-code block.** The `embed`
  palette block renders a sandboxed iframe only for the registered providers in `fly::embed`
  (YouTube, Vimeo, Google Maps; HTTPS, exact hosts, fixed path prefixes) and degrades to a
  safe link otherwise. Static publishing refuses `fly_embed` explicitly — embeds go through
  dynamic routes. Arbitrary third-party embeds and custom code stay out by design, while
  `raw_html` stays escaped (above). Owner: `fly` embed registry.
- **Design tokens exist, with one authoring surface; rename-with-usage-checks is still open.**
  A project carries a `flyTheme` extension (`fly::theme`): validated tokens render as `:root`
  custom properties and style declarations reference them with `var(--name)`. Token mutations
  are editor commands (`EditorCommand::Theme`), and the Page Builder admin has a Design tokens
  panel, and both adapter workspaces (`fly-leptos`, `fly-dioxus`) ship an editable token
panel on the same command. Renaming a token does not
  rewrite `var(--name)` references in style declarations — hosts must check usages first —
  and there is no site-wide style *inheritance* beyond the cascade itself. Owner: `fly` theme
  model.
- **Collaboration is out of scope for the engine.** No presence, locking, OT/CRDT, or merge:
  concurrent editors coordinate through the consumer's revision checks. Editor-minted ids are
  collision-resistant across sessions (`UniqueIdGenerator`), which removes accidental id
  clashes, but conflict resolution remains the consumer's contract.
- **`fly` does not migrate project documents.** The codec decodes straight into the current
  model; additive fields are the only in-major evolution. A future field rename must ship a
  consumer-side document migration (context contracts already have one in
  `context_migration.rs`; the project model does not).

## Standalone Usage

See [Extraction readiness](#extraction-readiness) first, in particular what a standalone build
gives up on locale fallback.

Rename `standalone-Cargo.toml` to `Cargo.toml` and build the root crate with
`default-features = false`.
All sub-crates reference each other using purely relative paths (`path = ".."`, `path = "../ui"`, `path = "../web"`), ensuring zero coupling to the host project.
