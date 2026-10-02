# Fly — Framework-Neutral Visual Editor Monorepo

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
| `fly-dioxus` | `dioxus/` | **Foundation only** | The same four layout shells for Dioxus 0.6. **No canvas, drag & drop, panels or state wiring.** |

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
└── dioxus/                     <-- `fly-dioxus`: layout shells for Dioxus 0.6 (foundation only)
    ├── Cargo.toml
    └── src/
```

---

## Architecture: Fluid Frontend Architecture (FFA)

```
┌─────────────────────────────────────────────────────────────┐
│                     UI Adapters Layer                       │
│     fly-leptos (Leptos 0.8)       fly-dioxus (Dioxus 0.6)   │
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
| `fly-dioxus` | `dioxus/` | Dioxus 0.6 layout shells (foundation only) | `fly-web`, `dioxus` |

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

## Standalone Usage

See [Extraction readiness](#extraction-readiness) first, in particular what a standalone build
gives up on locale fallback.

Rename `standalone-Cargo.toml` to `Cargo.toml` and build the root crate with
`default-features = false`.
All sub-crates reference each other using purely relative paths (`path = ".."`, `path = "../ui"`, `path = "../web"`), ensuring zero coupling to the host project.
