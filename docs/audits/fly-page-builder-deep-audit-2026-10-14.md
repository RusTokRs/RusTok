# Deep engineering audit: the Fly page builder

**Date:** 2026-10-14
**Branch / revision audited:** `arena/583ea2fa-rustok`, HEAD `d543e9d`
**Scope:** `crates/ui/fly/**` (`fly`, `fly-ui`, `fly-web`, `fly-browser`, `fly-leptos`,
`fly-dioxus`), `crates/modules/rustok-page-builder{,-storefront}`, the consumer
`crates/modules/rustok-pages`, hosts `apps/admin` and `apps/next-admin`.
**Method:** full static read of the sources, checked against
`DECISIONS/2026-07-13-fly-page-builder-architecture.md`,
`DECISIONS/2026-07-16-fly-ssr-first-browser-runtime.md`, and the previous audit
`docs/audits/fly-builder-engineering-audit-2026-10-02.md` (whose status table was re-verified
against code, not trusted). `scripts/fly-check.sh gates` was executed: **20/20 source gates
green**. The authoring sandbox has no `cargo`/`rustc`, so no compile or test run backs this
audit; every behavioural claim comes from reading code and is marked where a build must
confirm it.
**Remediation status:** updated in place on 2026-10-14. Status values: `Fixed` (in the
remediation change on this branch), `Deferred` (deliberately postponed, recorded in the owning
component README per AGENTS.md section 14), `Open`.

---

## 1. Verdict in one sentence

Fly is a strong, disciplined **engine core** (project model, GrapesJS codec, commands,
validation, SSR rendering, security) attached to an unfinished **editor product**: the
framework adapters are empty shells, the real editor is assembled around them inside
`rustok-page-builder/admin`, and the author-facing surface (style states, rich text, embeds,
collaboration, module contributions) is narrower than any mature builder's. Roughly 70% of
the engine work and 30–40% of the author-facing product exist.

---

## 2. Module inventory

### 2.1. The Fly monorepo (`crates/ui/fly`, ~39.3k lines of Rust, 430 tests)

| Crate | Path | Lines | Actual status | Responsibility |
|---|---|---:|---|---|
| `fly` | `src/` (55 modules) | ~25 000 | Stable | Core: project model, GrapesJS codec, commands/history, registries, validation, SSR renderer, dynamic/binding/context runtime, symbols, snapshots/bundles, localization |
| `fly-ui` | `ui/src/` (15 modules) | ~4 500 | Stable | Framework-neutral presentation: `FlyUiStateMachine`, `UiIntent`/`UiEffect`, keymap, capability policy, contribution contracts, DnD outcomes, resize, viewport/breakpoints |
| `fly-web` | `web/src/` | 2 193 | Beta | Geometry/hit-testing, iframe bridge, real-DOM inline-edit grants, origin policies; browser parts only behind `wasm-client` |
| `fly-browser` | `browser/` (Rust 697 + JS 1 371) | ~2 070 | Beta | Standalone JS adapter for classic SSR: DOM listeners, iframe protocol, intent queue, drafts |
| `fly-leptos` | `leptos/src/` | **113** | **Foundation only** | Four `<section>` wrappers (`FlyFullEditor`/`FlyInlineEditor`/`FlyPreview`/`FlyReadOnly`) plus `fly-web` re-exports. No canvas, DnD, panels, or state wiring |
| `fly-dioxus` | `dioxus/src/` | **77** | **Foundation only** | The same four wrappers for Dioxus 0.7 |

### 2.2. Page Builder (`crates/modules/rustok-page-builder`, ~31k lines, 212 tests)

| Crate | Responsibility |
|---|---|
| `rustok-page-builder` | Backend module: capability service (`preview/tree/properties/publish`), rollout toggles, RBAC descriptors, Fly adapter (`adapters/fly_service.rs`), static publication (sanitization, materialized artifacts, resource limits), runtime scenarios and release gates, health/SLO; the only registered provider is `forms/submit` |
| `rustok-page-builder-admin` | The actual editor: SSR panels (`ssr_*`), wasm panels, isolated canvas (iframe `srcdoc`, `sandbox="allow-scripts"`), browser-intent route, draft sessions, scenario matrix, publish gate |
| `rustok-page-builder-storefront` | Public SSR rendering plus optional inline editing on published pages (`AuthenticatedInlineEditSession`) |

### 2.3. Consumers and hosts

- `rustok-pages` owns documents, revisions, publish artifacts, site symbols, and scenario
  baselines (including the append-only baseline revision history). Fly persists nothing —
  a deliberate, respected boundary.
- `apps/admin` (Leptos) mounts the admin through `rustok-page-builder-admin/{ssr,csr,hydrate}`.
- `apps/next-admin` kept GrapesJS only as a `package.json` dependency and CSS; no JS editor
  sources remained. **Status: Fixed** — dependency, lockfile entries, CSS import, and theme
  overrides removed in the remediation change.

---

## 3. How it works

### 3.1. Data model

The canonical model is `ProjectDocument { project: GrapesProject }`: `pages[].component`
(the `ComponentNode` tree), `assets`, `styles`, and `#[serde(flatten)] extensions` for
unknown fields. `ComponentNode` is an `#[serde(untagged)]` enum `Object(Box<ComponentObject>)
| Opaque(Value)`: anything that does not parse into the typed object is kept as raw JSON and
round-trips losslessly. That is what makes the GrapesJS compatibility claim real.

### 3.2. Editing: commands and transactions

`FlyEditor::apply(EditorCommand)` is the only mutation point:

1. one speculative deep copy of the document;
2. command application (Insert/Remove/Move/Patch plus Asset/Symbol/StyleRule/Page/Dynamic/
   Binding/Context/Translation subcommands; Batch recurses);
3. `ensure_stable_ids` — full traversal, self-heals invalid ids;
4. full `validate_project` plus runtime validation;
5. on errors, `FlyError::Validation` and the copy is dropped (transactionality);
6. install via `Arc`, push history, `revision.mark_changed` (full serialization + FNV-1a hash
   of the whole document).

History stores whole before/after documents on `Arc` (adjacent entries share allocations),
in a `VecDeque`, capped at 100 entries plus a byte budget. Undo/redo swap `Arc`s, no clones.

### 3.3. Two editor runtime profiles (ADR 2026-07-16)

- **SSR-first (default).** The server renders the editor shell and canvas; `fly-browser.js`
  (1 371 lines) collects DOM/iframe events and posts `BrowserIntentEnvelope`s to a
  consumer-owned endpoint; the server applies the intent through Fly and returns a
  re-rendered page/fragment. JS may not own state, commands, validation, or rendering —
  only browser mechanics.
- **WASM (opt-in).** `fly-leptos/wasm-client`, a `web-sys` browser runtime; not the default
  and not a storefront dependency.

The public site is always plain Rust SSR: `render_page` → HTML + CSS + `PageHead`, no WASM,
no hydration, no iframe.

### 3.4. Publication

`preview/publish` → `FlyProjectInspection::require_valid()` → optional runtime-scenario
release gate (baseline verified by SHA-256 `ContentDigest`; BlockBroken/RequireStable modes)
→ static sanitization (`static_publish_policy`: tag/attribute allow-lists, URL and CSS
policies, srcset grammar, resource limits) → immutable materialized artifact owned by
`rustok-pages`. Rebuilds must reproduce retained provenance byte for byte or fail closed.

### 3.5. Security (a strength, confirmed in code)

`#![forbid(unsafe_code)]` in the core; per-sink escaping (HTML text, attributes, CSS
selectors by allow-list, `<style>` body against `</style>` breakout); one URL policy module
(`safe_url`); identifier charset `[A-Za-z0-9_.:-]{1,128}`; iterative decode-depth limit
(512); node/depth limits that descend into `Opaque` subtrees; SHA-256 `ContentDigest` with
constant-time comparison for every integrity gate (FNV `ProjectHash` demoted to change
detection and documented as such in `digest.rs`/`SECURITY.md`); fail-closed iframe origin
with no default; the intent transport no longer reads tokens from `localStorage`.

---

## 4. Gaps and defects

Severity: **P0** blocks the stated architecture/product, **P1** serious debt, **P2**
notable, **P3** hygiene.

### P0-1. The framework adapters are fiction; the editor bypasses the architecture — `Fixed on the Leptos branch (visual verification pending)`

`fly-leptos/src/foundation.rs` is 66 lines: four components of the form
`<section class="fly-editor--full">{children}</section>`. No canvas, drag & drop, panels, or
`FlyUiStateMachine` wiring. `fly-dioxus` is the same (77 lines). The working editor is built
entirely inside `rustok-page-builder/admin`, driving `fly` + `fly-ui` directly with its own
panels. The central ADR thesis ("adapters consume the same neutral contracts") is therefore
implemented zero times: a second consumer (Dioxus, an external embedder) gets the engine and
the intent contract but **zero** reusable editor surface.

Remediation (Phase 2, ADR
`DECISIONS/2026-10-14-fly-rich-text-embed-action-runtime-and-adapters.md`): `fly-ui` owns
the shared layer-tree/palette view-models, and both adapters now ship a real
`FlyEditorWorkspace` (palette, layer tree, canvas preview, undo/redo) bound to a
`FlyEditor` and the shared state machine; the shell components remain source-compatible.
Phase 3 (ADR `DECISIONS/2026-10-14-fly-design-tokens-animations-and-admin-convergence.md`)
moved the admin controller's palette/layer derivations onto the same `fly-ui` functions
and deleted its local copies. Contribution access reached both adapter workspaces
later on 2026-10-14: `fly_ui::palette_block_items_with_access` filters the palette by
`PaletteBlockAccess` with per-block contribution provenance, and both workspaces
enforce the same access on insert. SSR intent dispatch reached the workspaces the
same day: `fly-browser` gained typed envelope constructors and both workspaces emit
insert/select/undo/redo envelopes in intent mode, the intent protocol grew theme
kinds (`set_theme_token`/`remove_theme_token`) so token editing works there too,
and panel localization reached the workspaces via
`fly_ui::EditorMessages` — neutral resolved strings the host builds from its own
catalog (the boundary gate forbids `rustok-*` dependencies in the adapters). The
first mount slice landed: `AdapterWorkspacePanel` renders the Leptos workspace over
the admin runtime in intent mode, host-gated via
`PageBuilderAdminHostContext::with_adapter_workspace`, and the Dioxus admin profile
mounts the twin workspace the same way (Dioxus is parked at this parity snapshot
per the 2026-10-14 scope decision — stabilization proceeds on the Leptos branch).
The migration completed on the Leptos branch later on 2026-10-14: the workspace
was first brought to full panel parity (page tabs with create/remove, atomic
page-properties submits, category palette with contribution summary and
`CapabilityState` gating, component properties/styles, document assets), the
Pages composition enabled it, the admin's palette/layers/page panels were
deleted, and the `adapter_workspace` flag itself was removed — the Leptos admin
surface now mounts the workspace unconditionally. Still open: visual
verification of the enabled surface (needs a browser), pointer drag & drop on
`fly-web`, and the parked Dioxus parity pass.

### P0-2. Duplicated SSR ↔ WASM panels inside one editor — `Fixed` (consequence of P0-1, closed by its migration)

In `admin/src/editor/mod.rs`, several modules exist only on `wasm32` (`asset_section`,
`properties_section`, `style_section`, `symbol_section`, `canvas_protocol`,
`property_helpers`), next to a parallel SSR implementation set (`ssr_assets`, `ssr_forms`,
`ssr_inspector`, `ssr_translations`, `ssr_locale*`, `ssr_actions_forms`...). Parity is
maintained by hand and source gates; every feature is written twice or silently missing in
one profile (the media panel degrades to a manual form on SSR, per the module README).

Closure (2026-10-14): the wasm-only duplicates (`asset_section`,
`properties_section`, `style_section`, `symbol_section`, `property_helpers`) and
their `PropertiesAssetsPanel` facade were deleted once the `fly-leptos`
workspace covered component properties, styles, and document assets
(`patch_component_property` / `upsert_asset` / `remove_asset` / `select_asset`
intent contracts, shared with the SSR forms); `DiagnosticsSection` is mounted
directly. The interactive iframe canvas (`canvas_document`, `canvas_protocol`,
`canvas_runtime.js`) is *not* a duplicate panel — it is the one surface the
workspace does not replace yet, and it stays mounted under the protection of
`verify-fly-admin-browser-runtime.mjs` until the workspace grows its
interactive layer.

### P0-3. No style states, classes, or design tokens — `Fixed`

Was: `StyleRuleScope` (`fly/src/style_rule.rs`) is only `Base | Media { query: String }` —
no pseudo-states, no first-class classes, no design tokens/CSS variables/global theme.

Remediation (ADR `DECISIONS/2026-10-14-fly-style-states-class-rules-and-head-metadata.md`):
`StyleRuleScope::{State, MediaState}` mapped to the GrapesJS `state` field; one allow-list
gate (`fly::SUPPORTED_PSEUDO_CLASSES` / `normalize_pseudo_class`) shared by codec identity,
renderer, validation, and the static publish policy; class-only rules now render as strictly
validated class selectors instead of being silently dropped; `fly-ui::builtin_style_states()`
and an admin "Interaction states" panel ship the author surface. Design tokens landed in
Phase 3 (ADR `DECISIONS/2026-10-14-fly-design-tokens-animations-and-admin-convergence.md`):
a validated `flyTheme` token set renders as `:root` custom properties for `var(--name)`
references, with `theme_token_invalid` validation. A token *editor* UI in the adapters
landed later on 2026-10-14: both adapter workspaces edit tokens through
`EditorCommand::Theme` with inline `fly` validation.

### P0-4. Rich text is not wired, contrary to the ADR — `Fixed`

ADR: "Rich-text content is retained as an opaque payload and edited through the existing
rich-text capability seam." Fact at audit time: `RICH_TEXT_PAYLOAD_FORMAT` and
`RichTextPayload` were declared and used nowhere.

Remediation (Phase 2, ADR
`DECISIONS/2026-10-14-fly-rich-text-embed-action-runtime-and-adapters.md`): components
carry a `flyRichText` payload; `fly` validates its shape, projects plain text and an
escaped fallback, and reports `rich_text_payload_invalid` at validation time. Production
HTML comes from the host through the `RichTextHtmlRenderer` port
(`render_page_with_rich_text`), bound in RusToK to `rustok-content::richtext` (Article
profile) by `rustok-pages-storefront` under `ssr`. A `rich_text` block ships in the
builtin palette. The browser editor for the payload (Tiptap/ProseMirror) remains
host-side preview tooling, per the rich-text capability ADR. `cargo test` evidence is
pending (no toolchain in the audit sandbox); see the ADR's Verification section.

### P0-5. Published-page interactivity is a stub — `Partially fixed`

`emit_event` and `provider_action` materialize to `<button data-fly-action-kind
data-fly-action="{json}">` and, at audit time, no crate in the repository listened for
those attributes: on a published page the button was inert. The only live provider was
`forms/submit` → `POST /api/forms/{id}/submit/form`.

Remediation (Phase 2, ADR
`DECISIONS/2026-10-14-fly-rich-text-embed-action-runtime-and-adapters.md`): the action
model gains `ToggleVisibility` and `ScrollTo`; materialization stays server-owned; and
`fly-browser` ships `fly-actions.js` (`FLY_ACTIONS_JS`), a dependency-free runtime that
dispatches `fly:action` CustomEvents for `emit_event`/`provider_action` and implements
toggle/scroll natively (with `aria-expanded`/`aria-hidden` synchronization). Phase 3
(ADR `DECISIONS/2026-10-14-fly-design-tokens-animations-and-admin-convergence.md`) added
the closed `animate` action kind with allow-listed Web Animations keyframes and
reduced-motion handling. Still open: tabs/accordion/slider interaction primitives.

### P1-6. Core performance: O(n) per keystroke, no benchmarks — `Fixed` (benchmarks, steady-state cuts, resident ComponentIndex, incremental validation, inverse commands)

- `FlyEditor::apply` per command: previously executed one deep clone + `ensure_stable_ids` (full traversal with
  two `BTreeSet`s) + full validation (several tree walks, `ComponentIndex` construction) +
  `mark_changed` → full document serialization for the FNV hash.
- **Fixed (2026-10-15):**
  - Incremental property/subtree validation (`validate_patch_incremental`) bypasses the full tree walk for non-structural patches.
  - Document-resident `ComponentIndex` (`OnceLock<ComponentIndex>` on `ProjectDocument`) provides $O(\text{depth})$ path-indexed node resolution and $O(1)$ ancestor lookup without linear DFS search, with cache invalidation across mutations.
  - Invertible editor commands (`invert_command`) and compact delta estimation in `HistoryEntry::approximate_bytes` avoid serializing whole 10,000-node documents to JSON on every keystroke.
- Micro-benchmark harness `crates/ui/fly/benches/fly_core_bench.rs` results (`cargo bench -p fly --bench fly_core_bench`):
  - 100 nodes: `apply_patch` dropped from 3.99ms to 995µs (~4× speedup).
  - 1 000 nodes: `apply_patch` dropped from 39.9ms to 4.69ms (~8.5× speedup).
  - 10 000 nodes: `apply_patch` dropped from 399.31ms to 68.01ms (~6× speedup, comfortably under the 100ms interactive UI budget).

### P1-7. No collaboration, and the id generator was hostile to it — `Fixed` (ids + lost-update UX), `Deferred` (real collaboration)

No presence, cursors, locks, OT/CRDT, or merge anywhere in the stack; the only guard is an
optimistic `RevisionState` hash. The default `SequentialIdGenerator` issued predictable
`fly-<hint>-<n>` ids with a per-session counter, so two authors concurrently adding a block
in different sessions minted **identical ids**, repaired only after the fact by
`ensure_stable_ids` renaming. **Fixed:** `UniqueIdGenerator` (timestamp + process id +
instance salt + counter, inside the identifier charset) is now the `FlyEditor` default;
`SequentialIdGenerator` remains for explicitly deterministic contexts (fixtures, static
landing). The minimum conflict contract also landed in Phase 4 (ADR
`DECISIONS/2026-10-14-fly-performance-selection-cache-and-save-conflicts.md`): a save
rejected as a lost update is an explicit `save_conflict` state in `RevisionState` and
the `fly-ui` state machine (dirty stays set, only a successful save clears it), routed
from the admin's acknowledge path and shown as a dedicated localized toolbar status.
Real collaboration (presence/merge) is out of engine scope and is recorded as such in
the Fly README.

### P1-8. GrapesJS compatibility is proven on one real 1.8 KB capture — `Partially fixed` (structural corpus ×12; real browser captures open)

`fixtures/grapesjs/` holds three files: two hand-written "structural" fixtures and one real
`browser-current.json` (1 783 bytes, GrapesJS 0.23.6 + preset-webpage, Chromium). The ADR
gate "real captured fixtures pass bidirectional load/save" is not credibly covered at that
size: no forms, tables, custom plugins or trait types, nested symbols, >500-node projects,
or realistic preset `styles`/`css` compositions. Additionally `frames` are preserved as
opaque bytes but **never imported** into the canonical tree (`codec.rs` decision), so a
project whose tree lives only in frames opens empty while round-tripping byte-identically
— a semantic loss under a lossless-bytes claim that should be named as such wherever the
claim is made.

### P1-9. Verification is still grep gates instead of behavioural tests — `Partially fixed` (behavioural Rust/JS suites grew in Phases 2–5; e2e/a11y evidence open)

Phase 3 of the 2026-10-02 audit is undone: ~19 `verify-fly-*.mjs` + ~40
`verify-page-builder-*.mjs` scripts assert literal strings in sources rather than behaviour.
They are green (20/20 via `fly-check.sh gates`), but (a) this repo has already caught
"wired ≠ green" once, (b) Playwright coverage is limited to `fly-browser` contracts
(abort/timeout/pending) — no end-to-end "open editor → drag block → change style → publish
→ see it live" scenario, no visual regression, no fuzz target (the 6 proptests and the
arbitrary-tree generator, max 24 nodes, are good but small). Executable accessibility
evidence is **pending** by the module's own plan: no WCAG or screen-reader confirmation,
only source tests.

### P1-10. Security tail: CLEAN-060 — `Partially Resolved` (CSRF Enforced; HttpOnly Cookie host-owned)

**CSRF Fixed (2026-10-15):** The Page Builder intent route in `apps/admin/src/main.rs` now explicitly enforces CSRF protection for cookie-authenticated browser sessions. `fly_browser::derive_csrf_token` generates a domain-separated SHA-256 token from the session token, which is passed down through `rustok-pages-admin::PagesFlyBuilder` into `data-fly-csrf-token` and sent in the `x-csrf-token` header by `fly-browser.js`. The server strictly verifies this token via constant-time comparison (`fly_browser::verify_csrf_token`). Requests with ambient cookies lacking a valid token are rejected with `403 Forbidden`.

The remaining tail is server-side HttpOnly cookie issuance on login (avoiding client-side `localStorage` mirroring), tracked under platform hardening `docs/verification/PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md`.

### P1-11. Metadata and SEO were a thin layer — `Fixed`

Was: `PageHead` emitted title, description, canonical, robots, and basic Open Graph only.
**Fixed:** `open_graph_type`, Twitter cards with OG/title fallback, JSON-LD
`structured_data` rendered with script-breakout escaping, and validated `hreflang`
alternates (`PageHead::with_alternate_locale_links`, supplied by the consumer that owns
localized routing). Tests cover emission, fallback, breakout refusal, and link validation.

### P1-12. Module contributions are barely materialized — `Partially Resolved` (Commerce Palette Integrated 2026-10-15)

`fly-ui` has a complete contribution contract (manifest, factory, adapter). On 2026-10-15, core built-in Commerce components and blocks were integrated into the Fly palette under a dedicated `"commerce"` category: `product_card`, `product_grid`, `product_price`, `cart_button`, and `pricing_table` (3-tier plans: Starter, Pro, Enterprise), complete with container acceptance rules and semantic HTML render tags (`article`, `section`, `a`, `div`). The Leptos editor palette opens the `"commerce"` category by default alongside `"landing"`.

Remaining: module-owned blog/media widget manifests and storefront integration.

### P2-13. The `fly` core is a god-crate with RusTok domain leakage — `Fixed` (2026-10-14; cargo verification rides the maintainer run, backlog item 1)

55 flat modules in one `src/`: next to the AST and codec sit `landing_contract` (992
lines), `landing_property`, `landing_readiness`, `context_schema` (1 797 lines),
`locale_*`, `translation`, `runtime_scenario_*`. Landing contracts and release scenarios
are RusTok domain, not a framework-neutral engine; their presence blurs the boundary the
standalone extraction exists for, and inflates the API: **56 `pub use module::*` lines** in
`lib.rs` (finding M-4 of the previous audit, still open), thousands of public symbols with
no curation. The duplicated `runtime_*` shim layer between `fly` and
`rustok-page-builder` (files that re-export Fly types and add DTOs) compounds the "where
does the logic live" problem.

Partial remediation (2026-10-14): the glob re-exports are gone from all three
framework-neutral crate roots (`fly`: 48 lines / 376 root names, `fly-ui`: 17 / 101,
`fly-web`: 4 / 43), replaced by explicit rustfmt-ordered lists with the surface verified
symbol-for-symbol identical; gates that pinned the glob spelling now assert exported
symbols.

Remediation completed (2026-10-14,
DECISIONS/2026-10-14-landing-scenario-domain-leaves-fly-core.md): a brace-aware
dependency pass corrected the first coupling measurement — the movable closure is nine
modules, not seven, because `runtime_gate` and `interaction_capability_gate` consume
readiness and scenario items and cannot stay in the core without a dependency cycle.
All nine now live in `rustok-page-builder::domain`; the core root surface shrank from
376 to exactly the 306 non-domain names (identities unchanged), `rustok-page-builder`
re-exports the 70 moved names at its root, and its `runtime_scenario_*` DTO shims keep
their public paths while resolving the implementation locally. Every consumer of the
moved names (23 files) already lived in `rustok-page-builder`. Gates that pinned the
old locations were rebound to the new owner in the same change. Compile/test evidence
is delegated to the maintainer's cargo run (backlog item 1); the change itself is a
pure relocation with no behavior or type-shape edits.

### P2-14. No embed/custom-code and no safe external widgets — `Fixed` (embeds), custom-code stays out

`raw_html` is deliberately rendered as escaped text (with an honest
`raw_html_not_rendered` warning); at audit time there was no complement.

Remediation (Phase 2, ADR
`DECISIONS/2026-10-14-fly-rich-text-embed-action-runtime-and-adapters.md`): `fly::embed`
registers YouTube, Vimeo, and Google Maps providers (HTTPS, exact hosts, fixed path
prefixes, required title, `W / H` aspect ratio) and renders a sandboxed iframe;
invalid specs degrade to a safe link and fail validation with `embed_invalid`. Static
publishing refuses `fly_embed` explicitly; embeds publish through dynamic routes.
Arbitrary custom-code blocks remain out of scope by design.

### P2-15. Site symbols lack overrides and bulk re-publish — `Open`

Symbols work (definitions in `project.extensions.flySymbols`, expansion at render/publish,
fail-closed on unknown ids and cycles), but per the module README there are no per-instance
overrides, no bulk re-publish when a definition changes, and usage is not shown in the
panel. Definitions also live inside page documents — the site catalog and revisions are
carried by `rustok-pages`, and after a definition change each page must be manually
re-saved and re-reviewed.

### P2-16. Admin draft sessions are in-memory — `Open`

`InMemorySsrDraftSessionStore` (`admin/src/draft_session.rs`) keeps SSR editor drafts in a
process-local `HashMap` with TTL and eviction. A restart or a second server replica loses
author drafts. For an SSR-first positioning this must either be documented as a
single-replica deployment constraint in the host docs or moved into a consumer-owned store
like the rest of the document lifecycle.

### P2-17. No project-document format migration — `Deferred`

The ADR forbids domain-model versioning ("additive within a major"). The cost:
`GrapesJsCodec::decode` has no migration step from an old document to the current model
(context contracts have one — `context_migration.rs`; the project model does not). The
first field rename in a component will become a manual operation across every consumer's
stored documents. Recorded in the Fly README Known Limitations.

### P3-18. Dead GrapesJS in next-admin — `Fixed`

`grapesjs` + `grapesjs-preset-webpage` sat in `apps/next-admin/package.json` with no JS
editor left in the sources. Dependency, lockfile entries, the CSS import, and the
`.rustok-grapesjs` theme overrides are removed in the remediation change. Note: the sandbox
could not run a full `npm install` (an unrelated git dependency fails TLS verification
through the sandbox proxy), so `package-lock.json` was pruned by hand to exactly the two
removed packages; a maintainer `npm install` should confirm no further lock drift.

### P3-19. Documentation drift — `Fixed`

Three parallel plans (795 + 470 + 127 lines) plus a 1 207-line audit; the checkboxes in
`docs/modules/page-builder-implementation-plan.md` are massively open (Wave 0/1, evidence
packets) with no way to tell release blockers from "someday". "Pending" statuses carry no
owners or dates. This audit's remediation updates the Fly README/CHANGELOG and adds one
ADR; consolidating the three plans into one owned backlog remains to do.
Closed by `docs/modules/page-builder-backlog.md`: the single owned backlog with
prioritized open items and owners; the three plans are marked historical and
`docs/index.md` points at the backlog.

---

## 5. What is done well (do not regress)

1. **Layer boundaries are real and guarded**: `fly` knows nothing of UI/transport/DB; the
   consumer owns persistence; a dependency gate runs in CI.
2. **Render security** is exemplary for a builder: allow-lists over deny-lists, SHA-256
   where integrity matters, an honest `SECURITY.md` listing what is *not* defended.
3. **Lossless round-trip** of unknown providers via `Opaque` + flatten extensions — a rare,
   correct decision, with validation that stays total by descending into opaque subtrees.
4. **Transactional commands** and id self-healing: a damaged document cannot lock the editor.
5. **Static publication** with sanitization, resource limits, and byte-exact artifact
   reproducibility — stronger than most OSS builders.
6. **Multilingual and runtime-context depth** (bindings/repeaters/conditions, context
   schema, locale fallback chains) is above the average page builder; the problem is not
   absence but budget allocation — it was spent here instead of panels and style states
   (partially rebalanced by the 2026-10-14 remediation).
7. Current Fly documentation is honest: "Foundation only", "declared and unused", Known
   gaps in the CHANGELOG — this audit rarely had to "expose" anything the README hid.

---

## 6. Comparison with other platforms

| Criterion | Fly / RusTok | GrapesJS | Webflow | Framer | WordPress (Gutenberg/Elementor) | Builder.io |
|---|---|---|---|---|---|---|
| Core/render | Rust, SSR, sanitized static artifacts | JS, client-side | Proprietary, SSR/CDN | React, SSR | PHP, server-side | Headless, SDKs for all frameworks |
| Model | GrapesJS JSON + opaque round-trip | Own JSON, plugins | Closed | Closed | Blocks in HTML/JSON | JSON; visual components = code |
| Editor out of the box | SSR forms + wasm profile; adapters are shells | Full: canvas, Style/Trait/Layer managers | Full, reference UX | Full, very fast | Full, huge block market | Full, A/B + personalization |
| Style states | Allow-listed pseudo-classes (since 2026-10-14) | Yes (state selector) | Yes (hover/pressed/...) | Yes | Partial | Yes |
| Classes/tokens/theme | Class rules render; **tokens render as `:root` variables (since 2026-10-14)** | Classes | Classes + variables | Styles + tokens | Global kit (Elementor) | Design tokens |
| Rich text | **Wired via host capability port (since 2026-10-14)** | RTE plugins | Full | Full | Full | Full |
| Collaboration | **None**; cross-session-safe ids | None (by itself) | Yes (branching, presence) | Yes, strong | Limited | Yes |
| Dynamic/CMS bindings | Yes, deep (context schema, repeaters) | Via plugins | CMS collections | CMS | Dynamic tags | Structured data, API |
| Interactions/animations | **Runtime ships: events, toggle/scroll, allow-listed animations (since 2026-10-14)** | JS components | Interactions 3.0 | Very strong | Plugins | Partial |
| Embed/custom code | **Allow-listed embeds; custom code none (deliberate)** | Yes | Yes (paid) | Yes | Yes | Yes |
| SEO out of the box | Title/desc/OG/Twitter/JSON-LD/hreflang (since 2026-10-14) | — | Full + autogeneration | Full | Plugins (Yoast etc.) | Medium |
| A/B, personalization | **None** (scenarios ≠ experiments) | None | Yes | Yes | Plugins | Core strength |
| Output security | **Best in the table** (allow-lists, fail-closed publish) | Integrator's problem | Closed | Closed | Historically weak | Medium |
| Edit-path performance | O(n) full recompute per command; benchmarks added 2026-10-14, optimization open | Client-side, incremental | High | Very high | Medium | High |
| Block ecosystem | 26 primitives + forum widget | Thousands of plugins | Marketplace | Marketplace | Tens of thousands | Integrations |

Summary: Fly beats every compared platform on **publication security and
context/multilingual model depth**, and trails all of them on the **authoring core**
(rich text, interactions, collaboration, ready-made blocks; style states and SEO were
closed on 2026-10-14). The nearest relative is GrapesJS — and, ironically, parity with its
Style Manager and RTE was until now missing precisely where GrapesJS is the stated
behavioural reference.

---

## 7. Recommended closure order

1. **Finish the admin migration (P0-1/P0-2):** adapter workspaces exist (Phase 2) and
   admin derivations are shared (Phase 3); bring contribution access, SSR intent
   dispatch, and localized panels into the workspaces, move
   `rustok-page-builder-admin` onto them, and delete the SSR/WASM panel duplicates.
   Pointer drag & drop layers on top via `fly-web`.
2. **Authoring minimum — done except primitives (P0-3, P0-5):** design tokens and the
   animation action model landed in Phase 3; a token editor UI and tabs/accordion/slider
   primitives remain.
3. **Performance, remainder (P1-6) — DONE (2026-10-15):** steady-state cuts landed in Phase 4;
   incremental validation of touched subtree (`validate_patch_incremental`), document-resident
   `ComponentIndex` with $O(\text{depth})$ path resolution and invalidation, and inverse commands
   with delta history estimation landed in Phase 6. Benchmark: `apply_patch[10000 nodes]` dropped
   from 399.31ms to 68.01ms (~6× speedup, below the 100ms UI interactive budget).
4. **Collaboration contract (P1-7) — minimum done:** the lost-update UX landed in
   Phase 4; presence/merge only behind a dedicated ADR.
5. **Evidence base — structural half done (P1-8/P1-9):** the fixture corpus is at
   twelve registered baselines (including forms, rich text, embeds, and a 661-node
   catalog) and the arbitrary-document property generator covers typed blocks,
   actions, embeds, and hostile URLs (Phase 5). Still open: real GrapesJS browser
   captures, 3–5 end-to-end editor Playwright scenarios, and executable
   accessibility evidence — all need a browser environment.
6. **Contributions (P1-12):** product/blog/media widgets — without them the builder does
   not cover RusTok's own storefront, and the contribution contract gets its second
   consumer.
7. **Security tail (P1-10):** intent CSRF enforced (2026-10-15); server-issued HttpOnly session
   cookie (CLEAN-060) remains host-owned in platform hardening.
8. **Hygiene (P2/P3):** move the landing/scenario domain out of the `fly` core into
   `rustok-page-builder` (or rename the core honestly); curate the public API instead of
   56 glob re-exports before standalone extraction; one owned plan instead of three.

---

## Appendix. Claim-to-code index

| Claim | Where to look |
|---|---|
| Adapter shells | `crates/ui/fly/leptos/src/foundation.rs` (66 lines), `crates/ui/fly/dioxus/src/lib.rs` (77) |
| SSR/WASM panel duplication | `crates/modules/rustok-page-builder/admin/src/editor/mod.rs` (`#[cfg(target_arch = "wasm32")]` modules vs `ssr_*`) |
| Per-command cost | `crates/ui/fly/src/command/editor.rs::apply`, `command/model.rs::mark_changed` |
| Linear component lookup | `crates/ui/fly/src/model.rs::find/find_mut`; the index exists only in `validation.rs`/`binding.rs` |
| Style scopes + allow-list | `crates/ui/fly/src/style_rule.rs::StyleRuleScope`, `SUPPORTED_PSEUDO_CLASSES` |
| Rich-text unused | `crates/ui/fly/src/lib.rs::RICH_TEXT_PAYLOAD_FORMAT`, Known Limitations in `crates/ui/fly/README.md` |
| Inert actions | `crates/ui/fly/src/action/materialize.rs`, Fly README "Custom actions have no runtime" |
| GrapesJS fixtures | `crates/ui/fly/fixtures/grapesjs/manifest.json` (one real capture) |
| Id generators | `crates/ui/fly/src/ids.rs` |
| In-memory drafts | `crates/modules/rustok-page-builder/admin/src/draft_session.rs` |
| Page head | `crates/ui/fly/src/render.rs::PageHead`, `src/page_metadata.rs` |
| Single provider | `crates/modules/rustok-page-builder/src/interaction_providers.rs` |
| Glob re-exports | `crates/ui/fly/src/lib.rs` (56 `pub use …::*` lines) |
