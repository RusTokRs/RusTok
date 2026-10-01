# Fly — Framework-Neutral Visual Editor Monorepo

`fly` is an extensible, framework-neutral visual page builder and component-editor engine for Rust.
It provides a complete visual authoring pipeline with zero UI-framework coupling in its core and first-class thin adapters for **Leptos** and **Dioxus**.

This directory is structured as a **self-contained monorepo** ready to be extracted into an independent repository (`github.com/rustok/fly`):

```text
crates/ui/fly/
├── Cargo.toml                  <-- Root crate `fly` (AST, Document model, Codec)
├── standalone-Cargo.toml       <-- Workspace template when used as independent repo
├── README.md                   <-- Monorepo documentation
│
├── src/                        <-- Core AST, GrapesJS codec, revision tracking, validation
├── fixtures/                   <-- Bidirectional GrapesJS round-trip test fixtures
├── locales/                    <-- Localized authoring labels
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
├── leptos/                     <-- `fly-leptos`: Thin UI adapter for Leptos 0.8
│   ├── Cargo.toml
│   └── src/
│
└── dioxus/                     <-- `fly-dioxus`: Thin UI adapter for Dioxus 0.6
    ├── Cargo.toml
    └── src/
```

---

## Architecture: Fluid Frontend Architecture (FFA)

```
┌─────────────────────────────────────────────────────────────┐
│                     UI Adapters Layer                       │
│     fly-leptos (Leptos 0.8)       fly-dioxus (Dioxus 0.6)   │
│     - FlyFullEditor               - FlyFullEditor           │
│     - FlyInlineEditor             - FlyInlineEditor         │
│     - FlyPreview                  - FlyPreview              │
│     - FlyReadOnly                 - FlyReadOnly             │
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
| `fly-leptos` | `leptos/` | Thin Leptos 0.8 components (`FlyFullEditor`, etc.) | `fly-web`, `leptos` |
| `fly-dioxus` | `dioxus/` | Thin Dioxus 0.6 components (`FlyFullEditor`, etc.) | `fly-web`, `dioxus` |

---

## Standalone Usage

When extracted into a separate repository, rename `standalone-Cargo.toml` to `Cargo.toml`.
All sub-crates reference each other using purely relative paths (`path = ".."`, `path = "../ui"`, `path = "../web"`), ensuring zero coupling to the host project.
