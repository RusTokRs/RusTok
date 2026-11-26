# Fly — Framework-Neutral Visual Editor Monorepo

For full public interface and engine contract definitions, see [CRATE_API.md](CRATE_API.md).

`fly` is an extensible, framework-neutral visual page builder and component-editor engine for Rust.
Its core owns the project model, the lossless GrapesJS codec, commands, history, validation and SSR
rendering, with zero UI-framework coupling.

## Status

| Package | Path | Status | What exists today |
|---|---|---|---|
| `fly` | `.` | **Stable** | Project model, lossless GrapesJS codec, commands, history, registries, validation, SSR renderer, dynamic/binding/context runtime |
| `fly-ui` | `ui/` | **Stable** | `FlyUiStateMachine`, intents/effects, keymaps, capability policy, contribution model |
| `fly-web` | `web/` | **Stable** | Geometry, hit-testing, iframe origin validation, real-DOM inline grants, cleanup registry, cross-tab confirmed-write fencing with editor-scoped sender IDs |
| `fly-browser` | `browser/` | **Stable** | SSR JS bridge (`assets/fly-browser.js`, `assets/fly-actions.js`), CSRF derivation, `BrowserIntentEnvelope` contract |
| `fly-leptos` | `leptos/` | **Stable** | `FlyEditorWorkspace`, full authoring panels (`FlyPalette`, `FlyLayerTree`, `FlyCanvasPreview`, `FlyPageProperties`, `FlyComponentProperties`, `FlyStyleRules`, `FlyAssets`, `FlyThemeTokens`), dual runtime support (`ssr + browser-js` default, opt-in `wasm-client`) |
| `fly-dioxus` | `dioxus/` | **Active** | Dioxus 0.7 workspace plus Page Builder host integration for an isolated iframe canvas, pointer/hover/focus/keyboard messages, palette and layer drag/drop, bounded resize and cross-tab write fencing; mounted browser verification remains pending. |

RusToK Page Builder admin integrates both `fly-leptos` and `fly-dioxus`: Leptos uses the canonical `ssr + browser-js` runtime with optional WASM dispatch, while the Dioxus host supplies an isolated iframe canvas and routes geometry, drag/drop and resize through the shared controller. Cross-tab notifications are advisory; server revision checks remain authoritative.

### Extraction readiness

This directory is laid out so it can become an independent repository
(`github.com/rustok/fly`). The runtime dependency has been inverted, but manifest extraction still requires preparation:

- **Platform i18n dependency — inverted.** Locale identity goes through the `LocaleResolver`
  trait (`src/locale_resolver.rs`). `rustok-ui-i18n` is now an optional dependency behind the
  `platform-i18n` feature, which is **on by default**, so host builds are unchanged.
- **Workspace template — source-checked.** `standalone-Cargo.toml` supplies `[workspace.package]`,
  `[workspace.dependencies]` and `[workspace.lints]`, and
  `scripts/verify/verify-fly-standalone-workspace.mjs` checks inherited pins and lint policy.
  It does not build an extracted workspace. Cargo resolves inherited optional dependencies
  even when their features are disabled; a standalone export must resolve or remove the
  platform dependency and its feature wiring before Cargo can load the manifests.

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
scripts/fly-check.sh gates      # source guards and action runtime Node tests: no Rust toolchain
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
├── leptos/                     <-- `fly-leptos`: full Leptos 0.8 authoring workspace
│   ├── Cargo.toml
│   └── src/
│
└── dioxus/                     <-- `fly-dioxus`: Dioxus 0.7 workspace and web/desktop adapter
    ├── Cargo.toml
    └── src/
```

---

## Architecture: Fluid Frontend Architecture (FFA)

```
┌─────────────────────────────────────────────────────────────┐
│                     UI Adapters Layer                       │
│     fly-leptos (Leptos 0.8)       fly-dioxus (Dioxus 0.7)   │
│     workspace + SSR/browser       workspace + web/desktop  │
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
| `fly-web` | `web/` | Browser geometry, hit-testing, iframe bridge, peer-write channel, real DOM inline | `fly`, `fly-ui`, `web-sys` (WASM) |
| `fly-browser` | `browser/` | Standalone JS bridge asset for classic SSR | Pure Rust |
| `fly-leptos` | `leptos/` | Leptos 0.8 authoring workspace and SSR/browser adapter | `fly-web`, `leptos` |
| `fly-dioxus` | `dioxus/` | Dioxus 0.7 workspace and web/desktop adapter | `fly-web`, `dioxus` |

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
  Action, attribute and head URL checks share parsed HTTP(S) authority validation. SSR and
  document validation share bounded srcset candidate checks, including numeric descriptor
  uniqueness, positive density/width, one descriptor family and unambiguous comma boundaries.
  A bare candidate URL requires whitespace on at least one side of a following comma; encode
  literal URL commas as %2C. CSS url(), image(), src(),
  image-set() and -webkit-image-set() resource strings are checked against the same
  resource policy, including nested image functions; escaped or malformed strings fail closed.
  Original quoted resource URLs are validated before token masking, and SSR and static
  publication share a single source-order resource scan with atomic URL tokens.
  Masking uses disjoint original UTF-8 ranges without rescanning modified text. CSS variable substitution is also blocked
  in reflection masks (`-webkit-box-reflect`).

## Runtime locale selection

Runtime values, localized page routes, internal page links and navigation actions share the
same ordered locale fallback chain. Outgoing navigation selects the authored slug with the
runtime value resolver (including explicit field fallback or a single translation), then
checks that the inbound route resolves to the intended page under the same locale. A
shadowed or ambiguous slug is unresolved, not an invitation to use a fallback URL.
A missing translation may use an explicit fallback URL, never an arbitrary first locale.
The runtime pipeline retains the authored route catalog separately from rendered metadata.
Coverage summary lookup also uses the resolver's chain;
required translation coverage still requires an exact locale entry, never a fallback.
A page-targeting interaction requires a unique page ID. Duplicate imported page IDs are
reported as ambiguous by both link and action validation/materialization, and cannot
produce an href even when a fallback URL is configured.
Link, action and form materialization removes all ASCII case variants of the HTML
attributes it owns before validation or regeneration; an authored uppercase spelling
cannot retain a stale destination, method or runtime action marker.

## Runtime binding ownership

Each component slot has one runtime binding owner. HTML attribute and CSS property
names compare ASCII case-insensitively; field names remain case-sensitive.
Ambiguous imported bindings report blocking diagnostics and none of the conflicting
writers execute. Attribute/style writes replace authored case variants. Numeric
transforms retain the original JSON number, and an explicit null fallback survives
serialization. Fallback is used only for a missing path, not a present null or a
failed transform. Structural symbol identity cannot be bound.

The full runtime pipeline resolves bindings whose path begins with a visible
repeater alias per surviving instance (including the repeater root). Nested
aliases shadow outer aliases. Global bindings still run before expansion; local
values are captured in scope and applied after template interpolation, so a
bound string is not interpreted again as a template. Instance writes contribute
to the existing binding counters. Local bindings support content, safe CSS
property names and non-identity attributes; identity/reference and provider
contract writes remain blocked until they can participate in identity proofs.
The standalone `materialize_bindings` function remains the global-only pass.

## Runtime context migration

Migration uses the same blocking field and computed diagnostics as strict runtime
preflight. Tolerating breaking contract changes never suppresses invalid payload
values. Disabling target materialization preserves caller values (apart from
explicitly requested removals), still validates required fields and value types,
and deliberately does not evaluate computed producers.

## Runtime materialization bounds

Runtime context paths are bounded to 2048 bytes, 64 segments and array index 9999.
A single path write may add at most 10,000 array slots across all nested arrays;
invalid writes leave the original context unchanged. Equivalent path spellings are
compared structurally for duplicate field/computed targets and self-references.
Computed values are evaluated in structural dependency order rather than declaration order.
Computed targets discard stale host/default values before evaluation; failed object
slots remain absent, and existing array slots become null without shifting siblings.
Fallbacks are available to dependent expressions, overlapping write targets and cycles
are rejected, and the dependency graph is limited to 100,000 edges. Repeater aliases
must be distinct single context keys in each scope; nested shadowing remains supported.
Repeater expansion reserves work for every node in each copied template before
nested expansion; the existing public counter continues to count repeater instances.

## Identity and history contracts

Unique component IDs retain their existing bounded textual shape and serialized generator
state. Native generation uses clock/process/instance information; browser WASM uses UUIDv4
entropy for the stamp and generator salt. Constructing an editor must not call unsupported
WASM standard-library clock or process APIs.

**Dependency decision — WASM ID entropy.** Reuse the workspace UUID crate with its existing
`v4` and `js` support rather than implement a separate browser RNG or require a browser
framework in the core. This target-specific edge does not add a native dependency or change
the persisted document contract; the workspace lock already contains UUID and its dependencies.
A future alternative must preserve collision resistance across tabs and the identifier limit.
Browser execution and WASM bundle impact remain to be verified; a compile-only check is not
evidence that the entropy path executed successfully.


`ProjectDocument::clone_as_page` and `ProjectFragment::remap_ids` return `FlyResult`; callers
must handle ambiguous source IDs instead of assuming a copy succeeded. Component IDs and authored
HTML IDs are reissued separately, along with their recognized references and component-bound
style rules. Provider form IDs remain the provider's endpoint identity. A duplicate HTML ID within
one page, a form ID owned by another HTML node, or a duplicate component/form ID prevents an
ambiguous copy; HTML IDs can repeat on separate pages. Arbitrary provider payloads, assets and
independent symbol definitions are not recursively rewritten. For form-vs-node conflict checks,
a valid `flyForm` replaces its own authored HTML `id` when materialized. Duplicate authored
HTML IDs can still make re-identification ambiguous even when a later lowering would replace them.

Runtime repeaters interpolate HTML ID templates in each item's scope and keep local HTML/ARIA
references aligned if a collision requires a fresh ID; literal references to outside nodes are not
retargeted. Resolved HTML IDs may contain Unicode but not whitespace, controls or unresolved
expressions. Form IDs also identify provider resources and are never silently reissued: a
repeater that may produce the same form more than once is left unexpanded with a blocking error.
Scoped conditions, including those in nested repeaters, can prove that at most one copy survives.
Consumers must honor runtime error diagnostics before publishing.

Page identities remain optional for legacy documents, but a page patch that sets `id` must supply a non-empty string; use `PagePatch.remove_fields` to clear an ID. Public document mutation helpers invalidate the cached project hash and component index, and compound theme/symbol commands publish changes only after validation succeeds.

Mutating commands, including component patches, run the same full project validation before committing. A cached report is reused only for selection or an empty transaction; it cannot authorize a changed document. The patch-target validation API delegates to the canonical validator until dependency-complete incremental verification is available.

Snapshot comparisons fingerprint the actual document payload and retain the order and repeated identities of pages, assets and style rules. An entry whose position or payload changes appears in the corresponding changed collection (including shifted siblings); opaque entries retain their positions.

History budgets both undo and redo snapshots plus commands using a serialized-size estimate. It
preserves one oversized entry, and recomputes the accounting after deserialization. The estimate
is not an exact heap measurement and may count shared snapshots more than once.
Batch transactions and content patches that remove scalar text children do not claim a
local inverse; undo and redo restore the retained before/after snapshots only when
the entry is connected to the current document and the destination satisfies current validation
limits and provider policy. Rejection leaves both stacks, selection and revision unchanged. Style inverses
restore the complete original style slot, including absence.

Snapshot capture verifies canonical payload round-trip, derives the fingerprint from that
payload rather than a cached document hash, and commits its sequence only on success.
Exhausted sequences, duplicate IDs and zero-capacity restored catalogs reject capture.

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

- **Project access is cache-safe.** `ProjectDocument::project()` exposes a read-only view;
  `project_mut()` and `component_mut()` invalidate both derived caches before returning a mutable
  borrow. The raw `project` field is private, preventing downstream code from bypassing this invariant.

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
- **Commerce templates are authoring placeholders, not live catalog bindings.** The built-in
  `product_price` contains sample text and the `product_card` button has no default purchase
  action. Authors must replace sample data and configure an explicit Product handoff; the current
  blocks do not select a real SKU or obtain a Pricing quote. See the
  [storefront handoff contract](../../modules/rustok-page-builder/storefront/README.md#purchase-and-cart-bridge).
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
  A `switch_slide` action must choose exactly one selector (`target_slide`, `direction`, or `index`);
  Runtime target lookup stays inside the nearest landing body (or `body` for a standalone
  document) and prefers renderer-owned component hooks over an authored HTML ID. Custom actions
  prevent accidental form submit/reset; native actions retain their own browser behavior.
- **Rich text renders through the host's capability, not Fly's fallback.** A component carrying
  `flyRichText` stores a `RichTextPayload`; `fly` validates its shape, projects plain text, and
  can render a conservative escaped fallback. Production HTML must come from the host binding
  the `RichTextHtmlRenderer` port (in RusTok: `rustok-content::richtext` via
  `rustok-pages-storefront` under `ssr`, per `DECISIONS/2026-07-22`). A consumer that binds no
  renderer gets the fallback projection — safe, but deliberately not the rich-text capability.
  Browser editing of the payload (Tiptap/ProseMirror) remains host-side preview tooling; the
  save path re-enters through the same payload contract and validation.
- **Leptos is the production Page Builder workspace; Dioxus remains an optional source-level host.**
  `fly-leptos` supplies the mounted SSR/browser authoring surface and optional WASM dispatch.
  The Dioxus host source includes `FlyEditorWorkspace`, an isolated iframe canvas,
  host-validated geometry, palette and selected-layer drag/drop, and bounded resize patches,
  but the standalone `apps/dioxus-admin` shell is a dormant concept and is not a production
  integration. Cross-tab messages fence stale editors but do not merge documents; the server
  revision check remains authoritative. Mounted Dioxus browser/runtime verification is still
  a release gate.
- **Session lifecycle of the standalone admin.** Cookie-authenticated intent writes already
  require the server-verified `x-csrf-token` derived from the authenticated session ID; Bearer API
  requests must provide both an explicit Authorization header and an unambiguous `x-tenant-slug`
  header, and never inherit a tenant from the browser session. The remaining `CLEAN-060` gap is session
  issuance: the admin shell still mirrors a `localStorage` token into a script-readable cookie
  instead of using a server-issued HttpOnly session. See `SECURITY.md`.
- **Embeds use an explicit provider catalog; there is no custom-code block.** Built-in providers
  cover YouTube, Vimeo and Google Maps. Hosts can extend the catalog with `EmbedProviderRegistry`,
  configure the same registry on `FlyEditor::with_embed_registry`, and pass it to
  `render_page_with_embed_registry` or `render_page_with_runtime_context_with_embed_registry`.
  URLs are parsed as HTTPS, credentials and non-default ports are rejected, and normalized paths
  must remain within an exact provider path boundary. The iframe sandbox permits scripts and
  same-origin access for player compatibility, so custom registrations must use dedicated
  cross-origin provider hosts and must never include the storefront/admin origin. Static publishing
  refuses `fly_embed` explicitly — embeds go through dynamic routes. Arbitrary unregistered embeds
  and custom code stay out by design, while `raw_html` stays escaped (above). Owner: `fly` embed registry.
- **Design tokens exist with usage checks and reference rewriting.**
  A project carries a `flyTheme` extension (`fly::theme`): validated tokens render as `:root`
  custom properties and style declarations reference them with `var(--name)`. References must
  target declared Fly tokens; var() is disallowed in resource-bearing properties because host
  CSS can override custom properties through the cascade. Theme values also reject resource-bearing
  url(), image(), src() and image-set() strings. Token mutations
  are editor commands (`EditorCommand::Theme`), and both adapter workspaces (`fly-leptos`,
  `fly-dioxus`) ship an editable token panel on the same command. Renaming a token
  (`ThemeCommand::Rename`) rewrites exact custom-property arguments in CSS `var()` functions,
  including nested fallbacks, across component styles, style catalog rules, and sibling token
  definitions. Quoted strings, comments and unrelated text remain untouched. Renames retain
  full undo/redo reversibility, and `find_token_usages` provides pre-flight usage discovery. `ThemeCommand::Replace`
  rejects duplicate token names rather than silently discarding one. Site-wide style
  inheritance remains cascade-driven by design. Owner: `fly` theme model.
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

The workspace template is a manifest fragment, not a replacement for Fly's root package manifest.
Merge its workspace tables into the existing root `Cargo.toml`, retaining the package,
dependencies, targets and features. Before building outside RusToK, provide a resolvable
`rustok-ui-i18n` workspace dependency or remove the platform-specific dependency and
feature-gated implementation from the export. Merely passing `--no-default-features` does
not resolve a missing inherited dependency, and disabling the root feature alone does not
prevent a sibling crate from enabling it.

The standalone structural resolver is already implemented; producing and verifying a
self-contained export (including all sibling packages) remains an explicit delivery gap.
All sub-crates reference each other using purely relative paths (`path = ".."`, `path = "../ui"`, `path = "../web"`).

Validated no-op commands (empty batches and idempotent document changes) no longer create
history entries or advance the document revision. A no-op still validates the resulting project,
and an empty batch still requires edit permission at the UI capability gate; it cannot mutate a
read-only session or silently clear redo history.

Composed component style patches preserve sequential merge semantics: a non-object style
replaces the entire slot, and subsequent object merges cannot restore discarded properties.
The composed patch remains a single atomic, reversible editor command.

The enclosing `RenderPolicy` also gates iframe resources and rich-text navigation links.
Fly filters link marks before either the built-in fallback or a host-supplied
`RichTextHtmlRenderer` receives the payload; the host port remains responsible for safe HTML.
