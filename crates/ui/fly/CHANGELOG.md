# Changelog

All notable changes to the Fly page builder crates (`fly`, `fly-ui`, `fly-web`, `fly-browser`,
`fly-leptos`, `fly-dioxus`).

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Fly is pre-1.0 and
versioned with the host workspace, so until it is extracted (see the `rustok-ui-i18n` blocker in
`standalone-Cargo.toml`) entries are grouped by date rather than by released version.

## [Unreleased]

### Added

- **Slider / Carousel interactive primitive (`slider` and `switch_slide`).**
  Added built-in `slider` component definition to `fly::builtin_blocks` under the
  `"interactive"` category, complete with carousel track, multi-slide templates,
  previous/next button controls, and pagination dot indicators. Extended
  `ComponentAction` with `SwitchSlide { slider_id, target_slide, direction, index }`
  and `SlideDirection` (next, prev). `fly-actions.js` runtime gained `switchSlide`
  handling active slide visibility, loop navigation, aria-hidden / dot states,
  and native touch/swipe gesture support (`touchstart`/`touchend` with horizontal delta).
  Integrated `switch_slide` into SSR action forms editor in `rustok-page-builder-admin`.

- **Workspace hardening: error channel, history-aware toolbar, scoped styles.**
  The Leptos workspace gained an `on_error` callback that receives every
  failed local command (insert/pages/properties/styles/assets/tokens) instead
  of the previous silent `let _ =` swallowing, reactive undo/redo disabled
  states from the editor history, and `FlyStyleRules` — responsive
  (media-query) and interaction-state (pseudo-class) style editing for the
  selected component with values seeded from the document's style-rule
  catalog and a breakpoint preview that constrains the canvas preview width.
  The browser intent protocol grew to 52 kinds with `upsert_style_rule`
  (mutating), which hosts apply as `StyleRuleCommand::UpsertComponentRule`
  after `style_patch` validation. On that parity the Page Builder admin
  deleted `ResponsiveStylePanel`/`StateStylePanel`, its last duplicated
  style panels. A capability-consistency pass then aligned every workspace
  panel on `Option<CapabilityState>`: page tabs gate page creation/removal
  on Edit, page properties gate apply/move on Properties, the design-token
  editor gates on Styles, and `FlyStyleRules` was fixed from a bare
  `CapabilityState` prop (a latent type mismatch with the workspace) to the
  shared optional form. Selection failures now ride the `on_error` channel,
  and the unused `viewport_label` message field was removed.
- **Workspace panel parity and the admin duplicate deletion** (audit P0-1/P0-2).
  The `fly-leptos` workspace now covers everything the admin's duplicated
  panels did: page tabs with create/remove and an atomic page-properties
  submit (`page_properties` envelope → one server-side page patch), rename/SEO
  metadata/move, a category-grouped palette with a contribution assembly
  summary and `CapabilityState` gating, component properties and styles
  (`FlyComponentProperties` over a neutral `component_property_view`, editing
  through the `patch_component_property` contract), and document assets
  (`FlyAssets` over `upsert_asset`/`remove_asset`/`select_asset`). On that
  parity the Page Builder admin deleted `PageManagerPanel`,
  `PaletteLayersPanel`, `PropertiesAssetsPanel`, the wasm-only
  `asset_section`/`properties_section`/`style_section`/`symbol_section`/
  `property_helpers`, and the `adapter_workspace` opt-in flag — the Leptos
  admin mounts the workspace unconditionally. The interactive iframe canvas
  stays mounted (the workspace does not replace it yet); Dioxus remains parked
  at its previous parity snapshot.

- **Design-token authoring.** `EditorCommand::Theme` (replace/upsert/remove) mutates the
  `flyTheme` token set through the editor's transaction/validation/history path; the Page
  Builder admin gains a Design tokens panel, and `docs/modules/page-builder-backlog.md`
  becomes the single owned Fly/Page Builder backlog (audit P3-19).
- **First admin mount of the adapter workspace** (audit P0-1). The Page Builder
  admin gains `AdapterWorkspacePanel`: it renders the `fly-leptos`
  `FlyEditorWorkspace` over the admin runtime in SSR intent mode (typed intents
  dispatch into the admin controller, which stays the source of truth) with an
  `EditorMessages` set built from the module's message catalog. Hosts opt in via
  `PageBuilderAdminHostContext::with_adapter_workspace(true)`; the classic SSR
  panels remain mounted alongside for now. The admin's `dioxus-ui` profile mounts
  the `fly-dioxus` workspace twin over its session signal with the same intent
  dispatch and message builder.
- **Theme intents** (audit P0-1). The browser intent protocol grows to 50 kinds:
  `set_theme_token` / `remove_theme_token` (mutating, gated by the `Styles`
  capability through the admin capability preflight). `fly-browser` gains typed
  constructors; the page-builder admin dispatcher applies both as
  `EditorCommand::Theme`; both adapter workspaces now edit design tokens in SSR
  intent mode instead of rendering them read-only.
- **Localized workspace panels** (audit P0-1). `fly-ui` gains `EditorMessages`:
  the neutral, host-resolved string set for every workspace surface (toolbar,
  palette, layers, token editor, canvas), English by default. Both adapter
  workspaces and their sub-panels accept it; hosts resolve it from their own
  message catalogs, because the dependency-boundary gate forbids `rustok-*`
  dependencies in the adapters.
- **SSR intent dispatch in the adapter workspaces** (audit P0-1). `fly-browser`
  gains typed `BrowserIntentEnvelope` constructors (`select`, `insert_block`,
  `undo`, `redo`, `with_page_context`) whose intent names and payload shapes match
  the server dispatch contract by construction. Both `FlyEditorWorkspace`
  implementations accept `instance_id` + `on_intent`: in intent mode palette
  inserts, selection changes, and undo/redo are emitted as envelopes (contribution
  access still enforced first) and nothing mutates locally — the host re-renders
  with the server-returned document. Design tokens render read-only in intent
  mode until the intent protocol grows theme kinds.
- **Contribution access in the adapter workspaces** (audit P0-1). `fly-ui` gains
  `palette_block_items_with_access`: the palette listing is filtered by
  `PaletteBlockAccess` and each `PaletteBlockItem` carries its contribution
  provenance (`contribution_ids`, serde-defaulted). Both `FlyEditorWorkspace`
  implementations (Leptos and Dioxus) accept an optional contribution assembly,
  render `data-fly-contribution-ids`, and enforce access again at insert time —
  the same policy the page-builder admin applies.
- **Token editor in the adapter workspaces.** `fly-leptos` gains `FlyThemeTokenEditor`
  (list, remove, upsert with inline `fly` validation) wired into `FlyEditorWorkspace`;
  the `fly-dioxus` workspace gains the same editing surface. Adapter token display is
  no longer read-only (audit P0-3 remainder).

- **Evidence corpus and diverse property coverage** (ADR
  `DECISIONS/2026-10-14-fly-evidence-corpus-and-property-coverage.md`). The
  GrapesJS-shaped fixture corpus grows to twelve honestly-labelled structural
  baselines (forms, rich text, embeds, interactions with tokens, frames, a
  661-component catalog) with a decode/render/validate/idempotent-encode harness,
  and the arbitrary-document property generator now covers typed blocks, actions,
  embeds, rich text, and hostile URLs with round-trip and render-safety invariants.
  Real browser captures and Playwright/a11y evidence remain open (need a browser).

- **Command-path performance and explicit save conflicts** (ADR
  `DECISIONS/2026-10-14-fly-performance-selection-cache-and-save-conflicts.md`).
  `ensure_stable_ids` skips its repair walk (and per-id clones) for already-stable
  documents; selection commands return the cached validation report instead of
  re-validating the whole document per click. Saves rejected as lost updates now
  surface as a dedicated conflict state (`RevisionState`/`DirtyState.save_conflict`,
  `UiIntent::SaveConflict`, localized toolbar status in the admin) instead of a
  generic failure. Also fixed: action targets (toggle/scroll/animate) now receive a
  plain `id` in published renders so the action runtime can find them without editor
  instrumentation.

- **Design tokens and animation actions** (ADR
  `DECISIONS/2026-10-14-fly-design-tokens-animations-and-admin-convergence.md`). Projects can
  carry a `flyTheme` token set (`fly::theme`): validated tokens render as `:root` custom
  properties for `var(--name)` references, with `theme_token_invalid` at validation time.
  Actions gain the closed `animate` kind (allow-listed Web Animations keyframes in
  `fly-actions.js`, reduced-motion aware); the admin action form covers it. `fly-ui` now owns
  the layer/palette derivations with parent/index metadata, and the page-builder admin
  controller consumes them instead of its local copies.

- **Rich text, embeds, and an action runtime** (ADR
  `DECISIONS/2026-10-14-fly-rich-text-embed-action-runtime-and-adapters.md`). Components can
  carry a `flyRichText` payload: `fly` validates its shape, projects plain text and an escaped
  fallback, and hosts render production HTML through the `RichTextHtmlRenderer` port
  (`render_page_with_rich_text`); RusTok binds `rustok-content::richtext` in
  `rustok-pages-storefront`. The `embed` block renders sandboxed iframes only for
  allow-listed providers (`fly::embed`: YouTube, Vimeo, Google Maps) and degrades to a safe
  link otherwise; static publishing refuses `fly_embed`. Actions gain `ToggleVisibility` and
  `ScrollTo`, and `fly-browser` ships `fly-actions.js` (`FLY_ACTIONS_JS`), which dispatches
  `fly:action` events for `emit_event`/`provider_action` and implements the new behaviours
  on published pages.
- **Real adapter workspaces.** `fly-ui` gains the shared layer-tree/palette view-models;
  `fly-leptos` and `fly-dioxus` each ship a `FlyEditorWorkspace` (palette, layers, canvas
  preview, undo/redo) on the shared editor contracts, next to the original shell components.

- **Pseudo-class style states, end to end** (ADR
  `DECISIONS/2026-10-14-fly-style-states-class-rules-and-head-metadata.md`). `StyleRuleScope`
  gains `State` and `MediaState`, mapped to the GrapesJS `state` field; `SUPPORTED_PSEUDO_CLASSES`
  and `normalize_pseudo_class` are the single allow-list gate shared by codec identity, renderer,
  validation (`unsupported_pseudo_class` warning), and the static publish policy. The Page Builder
  admin gains an "Interaction states" panel, and `fly-ui` exposes `builtin_style_states()`.
- **Class rules now render.** Unbound GrapesJS class rules were parsed and silently dropped; the
  renderer now emits them when every selector is a plain class name, and skips anything else
  instead of letting it become a selector. The static landing policy still requires
  component-bound rules and now says so accurately.
- **Completed page head metadata.** Twitter cards (with Open Graph/title fallback), `og:type`,
  JSON-LD `structured_data` with script-breakout escaping, and validated `hreflang` alternates
  (`PageHead::with_alternate_locale_links`).
- **`UniqueIdGenerator` is the editor default.** Session-sequential ids collided across
  concurrent editing sessions; minted ids now mix timestamp, process id, instance salt, and a
  counter while staying inside the identifier charset. `SequentialIdGenerator` remains for
  explicit deterministic use.
- **Core benchmarks.** `benches/fly_core_bench.rs` (dependency-free, `harness = false`) covers
  codec round-trip, validation, rendering, and `FlyEditor::apply` at 100 / 1 000 / 10 000 nodes.

Engineering audit remediation — see
`docs/audits/fly-builder-engineering-audit-2026-10-02.md` for the full findings register.

### Security

- **The runtime-scenario release baseline hash is now a SHA-256 digest.** `digest.rs` states that
  every gate answering "is this payload the one that was approved?" uses `ContentDigest`, and names
  the runtime-scenario release baseline as one of them — but
  `RuntimeScenarioReleaseBaseline::computed_hash` still returned a `ProjectHash` (FNV-1a 64)
  fingerprint. FNV-1a's `xor`/`multiply` chain is invertible, so a payload can be constructed to
  match a chosen 64-bit value, and this hash is what `runtime_scenario_baseline_hash_invalid`,
  `RuntimeScenarioReleaseMode::BlockBroken` and the `expected_baseline_hash` compare-and-swap rest
  on. It now produces `sha256:<64 hex>`, verified with a constant-time comparison.

  `RuntimeScenarioRenderSnapshot::snapshot_hash` deliberately stays on `ProjectHash`. It is not an
  approval anchor: a snapshot is always carried inside an envelope that digests it with sha256 (the
  baseline payload above, and the materialization identity's `runtime_snapshot_hash`), and
  snapshots are persisted inside `page_static_landing_artifact` and
  `page_publish_rebuild_source`, where the rebuild path requires a rebuild to reproduce them *byte
  for byte* — changing the algorithm would fail every retained artifact with "rebuilt runtime
  evidence does not exactly reproduce retained provenance". The same reasoning kept the per-case
  `html_hash`/`css_hash`/`document_hash` on FNV-1a.

- **Baselines persisted in the old form keep working and are upgraded on first read.** The legacy
  FNV-1a fingerprint is still recognised, so no row is retroactively rejected and no publishing
  path breaks on upgrade; `rustok-pages` rewrites `baseline_hash` the first time such a row is read
  (contents untouched, compare-and-swap on the old hash, a concurrent writer's row wins and is
  returned instead). A value that matches no recorded fingerprint — an altered envelope, exactly
  what this hash exists to catch — is never repaired, and new captures never produce the legacy
  form.
- `EditorCommand::RestoreSnapshot` now restores through `restore_verified`. `restore` verifies
  the content digest only when one is present, falling back to `project_hash` — a cheap FNV-1a
  change-detection token that is trivial to forge. Nothing untrusted reaches that path today, but
  `EditorCommand` is `Deserialize`, so the first API accepting a command would have made the
  downgrade reachable. Everything `capture` produces carries a digest, so requiring one costs
  nothing.

- **Inline edit contract hardened.** The authorization proof was compared with `!=` inside an
  `||` chain, leaking timing both per byte and by which field mismatched; it is now compared with
  `constant_time_eq`, separately from the public fields. `validate_request` normalized the value
  into a discarded copy, so a request deserialized straight from JSON could pass validation and
  carry a carriage return into stored content; a value that is not already in normal form is now
  rejected. Plain text rejected only NUL, letting every other C0/C1 control and the
  bidirectional override characters (`U+202E` and friends) through — these survive HTML escaping
  and can make rendered text read differently from what was stored and reviewed.

- **Opaque components no longer bypass validation.** `ComponentNode::visit` stopped at every
  `Opaque` node, so a subtree the typed model could not parse counted toward neither
  `maximum_nodes` nor `maximum_depth`, and the ids inside it were checked neither for duplication
  nor for a valid character set. Validation now descends into opaque subtrees — and into opaque
  *child lists*, a second blind spot where `children()` reported them as empty. `Opaque` itself is
  retained: lossless round-tripping of unknown providers is the point of the codec.
- **Stylesheet and attribute escaping rewritten.** Component ids reached `<style>` selectors
  unescaped; `escape_css_attribute` now works from an allow-list and `escape_style_element_text`
  prevents element breakout.
- **URL policy consolidated.** Five diverging implementations (two of them byte-for-byte copies
  differing only in an infix) replaced by `safe_url`, with `UrlAttributeKind` and `UrlPolicy`.
- **Same-origin enforcement in the browser bridge.** `fly-browser.js` validates intent endpoints
  and response-driven navigation against the document origin.
- **Decode depth is bounded** (`MAXIMUM_DECODE_DEPTH`), checked iteratively so that a hostile
  document cannot overflow the stack during the check itself.
- **Identifier charset enforced**: non-empty, ≤128 bytes, `[A-Za-z0-9_.:-]`.
- **Snapshot integrity** via `ContentDigest` (SHA-256, constant-time comparison) and
  `restore_verified`.
- `#![forbid(unsafe_code)]` in `fly`, `fly-ui` and `fly-browser`; `#![deny(unsafe_code)]` in the
  crates whose framework macros emit their own `allow` attributes.
- Added `SECURITY.md` describing the trust boundaries and the threat model.

- **The browser adapter's expected origin is fail-closed and has no default.** `expectedOrigin` used
  to fall back to the literal `"null"` — the opaque origin a sandboxed `srcdoc` iframe reports — so
  a host that never configured an origin silently accepted messages from the sandboxed canvas
  instead of refusing to mount. `BrowserAdapterConfig::expected_origin` is now `Option<String>`
  with no default, the shipped adapter throws `EXPECTED_ORIGIN_REQUIRED` from `start()` when
  neither `expectedOrigin` nor `data-fly-expected-origin` is set, and the empty value and the `*`
  wildcard resolve to "not configured" rather than to "accept anything" — the same rule
  `fly_web::normalize_expected_origin` applies on the Rust side. The Page Builder admin host, whose
  canvas is rendered `sandbox="allow-scripts"` with `srcdoc`, states the matching origin explicitly
  through `PAGE_BUILDER_CANVAS_EXPECTED_ORIGIN`.

- **The browser adapter no longer reads the admin token or the tenant slug from `localStorage`.**
  It used to scrape `rustok-admin-token` and `rustok-admin-tenant` and send them as
  `authorization: Bearer` and `x-tenant-slug` on every intent request, although the request already
  runs with `credentials: "same-origin"` and the admin session cookie is revalidated server-side
  before it becomes a request-scoped auth snapshot. Copying a credential into script-readable
  storage widened the XSS surface, and a client-supplied tenant was a claim the adapter had no
  business making. Requests now authenticate solely through the session cookie; `x-csrf-token` is
  still forwarded when the host configures one.

### Fixed

- **Blurring an untouched inline-edit text no longer counts as an edit.** The real-DOM adapter
  commits on `focusout`, and it emitted a request for whatever the element contained at that
  moment — so entering a text and leaving it unchanged still produced a request the session
  answered with `NoContentChange`, which the host surfaces as a validation error: a stray red
  message and a round trip for every click-through. The adapter now keeps the text it last saw in
  each editor and stays silent while it is unchanged (a rejected request is still retryable by
  editing again, because the baseline only moves when a request is emitted). The same pass closes
  the gap between what the DOM can contain and what the contract accepts: the value now goes
  through `normalize_plain_text` before it is bound, so a pasted tab, carriage return or control
  character is reported against the element that holds it instead of failing request validation in
  the host.
- **A form that submits nowhere now says so.** `ComponentForm` has three mutually exclusive
  submission shapes — a native `action_url`, a provider action a host resolves, or `method: dialog`
  for a host-driven dialog — and when none is present the browser falls back to the document URL,
  so the field values land as query parameters on a page reload (the default `get`) or the page is
  POSTed to itself. The palette's own form block is exactly that shape, so the first form most
  authors build appeared finished and did nothing (functional audit G-7, "the form goes nowhere").
  Validation now reports `form_without_submission_target` as a warning — for a block with a
  `flyForm` contract and no target, and for a bare `form` component with no `action` attribute —
  while exempting dialog forms and forms already wired to a host through
  `ComponentAction::SubmitForm`. GET-to-self stays legal for search and filter forms, which is why
  this is a warning rather than an error.

- **The `raw_html` block no longer advertises a capability the engine does not have.** The palette
  offered a block called "Raw html" whose content the renderer escapes and prints as text: the
  engine has no raw HTML sink at all, so a pasted `<iframe>` published as visible markup with
  nothing anywhere explaining why (functional audit `G-28`). The type stays — the codec must
  round-trip documents authored elsewhere — but its palette label now reads "Raw HTML (escaped as
  text)", its template text says the same, and `validation` reports `raw_html_not_rendered` as a
  warning for any document carrying visible content in one. A warning rather than an error, so
  documents that already use the block keep publishing.

- **`fly-web` can deliver outbound iframe messages to the sandboxed canvas.** `IframeMessagePort`
  validated its destination with the inbound rule, which accepts `"null"` — the origin a sandboxed
  `srcdoc` iframe reports — and then handed that same string to `postMessage`. The platform accepts
  only `*`, `/`, or an absolute URL as `targetOrigin`, so the call raised `SyntaxError: Invalid
  target origin`, and substituting a real origin instead makes the browser drop the message because
  it can never equal the recipient's reported origin. An opaque destination is now sent to `*`
  through `outbound_target_origin`, a host-tested decision in `origin.rs` next to the inbound rule
  it is deliberately asymmetric with; `OPAQUE_ORIGIN` names the value both rules turn on.

- **`fly-web` geometry rejects non-finite input.** Browser geometry crosses the iframe bridge as
  JSON, where an out-of-range literal deserializes to an infinity rather than an error. An
  infinite rectangle contained every point, passed the size guard, and then yielded a NaN ratio
  that made `resolve_drop_position` answer `After` for a rectangle that does not exist; a single
  non-finite transform offset turned every converted coordinate into NaN; and candidate ranking
  used `partial_cmp(..).unwrap_or(Equal)`, a comparator that is not a total order and that
  `slice::sort_by` may panic on. Ranking now uses `total_cmp`, unusable scores sink to
  `f32::MIN`, and unreal rectangles are dropped at the boundary.

- **Nested repeaters work.** They previously failed in *both* processing orders: expanding the
  outer repeater first renamed the inner repeater's target out of existence, and expanding the
  inner one first evaluated it against the global context so every outer copy received identical
  data. Replaced by a single depth-first expansion in which each clone expands its own nested
  repeaters against its own local context.
- **Conditions inside repeater templates.** A condition on `item.*` was evaluated once in a global
  pre-pass where the path does not resolve, so the component was judged falsy and deleted from the
  template for every item. Such conditions are now deferred to each item's local context.
- `safe_style` rejected every `url(`, which made `background-image` unusable. Safe references are
  allowed, validated against the same resource policy as `src`; `behavior`, `-moz-binding`,
  `-ms-behavior` and `expression` are rejected by property name.
- An empty `EditorCommand::Batch` computed an empty capability requirement, and the state
  machine skips its entire guarded branch when nothing is required — so the batch bypassed the
  read-only check and still reached `FlyEditor::apply`, which records history and marks the
  revision changed. An empty batch now requires `Edit`, failing closed.
- The renderer classified only `href`, `src`, `poster`, `action` and `formaction` as URL-bearing,
  so `cite` and `usemap` reached the output with no URL check at all while static publishing
  rejected the same document. Both are now classified, with a new `Fragment` kind for `usemap`.
- Static publishing judged CSS references by a blanket `url(` ban while already permitting
  `https://` and relative URLs for image attributes, so the same image was accepted through `src`
  and refused through `background-image`. References are now validated per URL with that same
  rule. Layers still differ from each other — publishing refuses plain `http://` — but each layer
  now applies one rule to attributes and references alike.
- `validate_origin` tested `origin.trim()` but returned the untrimmed string, so an expected
  origin configured with a stray space passed validation and then never matched the canonical
  `MessageEvent::origin()` — the iframe bridge silently discarded every inbound message. The rule
  now lives in `web/src/origin.rs`, outside the `cfg(wasm32)` gate that had made this pure string
  logic impossible to test.
- Context schema validation rejected only exact duplicate field paths, so declaring both `user`
  and `user.name` validated clean and then produced a JSON Schema that depended on declaration
  order: one order yields a string carrying `properties` (which validators ignore), the other
  silently overwrites the nested field. Ancestor/descendant conflicts are now reported as
  `runtime_context_field_path_conflict`, compared by path segment so `user` and `username` stay
  unrelated.
- Four separate copies of id-reference substitution consolidated into `id_reference`, which
  rewrites only declared reference positions instead of any string that happens to match an id.

### Fixed

- `verify-page-builder-runtime-context.mjs` failed on a correct pipeline. Its stage-order pins
  read the first textual match in `runtime_pipeline.rs`, which is the crate's `use` block — where
  rustfmt sorts `materialize_bindings` before `materialize_context` — and `materialize_runtime`
  matched the `materialize_runtime_locale_context` prefix (a different stage). The check now reads
  the pipeline function body and matches call forms; the workflow runs the verifier.

### Added

- `MAX_REPEATER_DEPTH` (8) and `MAX_TOTAL_REPEATED_NODES` (50 000) with
  `runtime_repeater_depth_exceeded` / `runtime_repeater_budget_exceeded` diagnostics.
- `ComponentIndex`: one traversal answering id lookups that previously walked every page per
  query.
- `StyleRuleIdentity`: reads a rule's identity without copying its declarations or raw JSON, for
  the predicate call sites that discarded the full descriptor immediately.
- History memory budget (64 MiB, `with_history_memory_budget`).
- Property-based tests covering escaping, codec round-trips, identifier/CSS-escape agreement and
  node-count completeness.
- `LocaleResolver` trait with a CLDR-backed `PlatformLocaleResolver` (feature `platform-i18n`,
  on by default) and a self-contained `BasicLocaleResolver`. This inverts the last dependency
  blocking extraction into a standalone repository.
- Shared `[workspace.lints]` policy, mirrored into `standalone-Cargo.toml` and checked for drift.
- CI: wasm32 jobs, adapter feature-combination checks, a dependency advisory job, and
  `verify-fly-gates-are-wired.mjs`, which failed loudly on the 7 verification gates that no
  workflow ran — all 4 rotted gates were among those 7.
- Runtime context contract compatibility and migration: `diff_runtime_context_contracts` and
  `diff_runtime_context_contracts_with_policy` compare the contract a runtime context payload was
  authored against with a candidate one, naming every difference (`RuntimeContextContractChange`,
  13 kinds) and deciding per kind which breaking changes stay warnings
  (`RuntimeContextCompatibilityPolicy`); `migrate_runtime_context` moves a payload onto the target
  contract — optionally dropping paths the target no longer declares, then applying the target's
  defaults and computed values through the existing `materialize_context` — and returns the
  compatibility report beside the migrated context. The migration is never silent: breaking
  contract changes and dropped values are diagnostics on the result. Supporting API:
  `RuntimeContextCompatibilityReport`, `RuntimeContextMigrationPolicy`,
  `RuntimeContextMigration`, and `remove_context_path` in `context_schema.rs`, the removal
  primitive the migration needs and the schema module did not have.
- `RuntimeScenarioReleaseBaseline::snapshot_has_valid_hash()`, the named integrity check for the
  embedded scenario render snapshot. `validate()` already reported
  `runtime_scenario_baseline_snapshot_hash_invalid` by calling `self.snapshot.has_valid_hash()`
  inline; the accessor makes the check part of the baseline contract, so a consumer can inspect a
  retained baseline's snapshot integrity without re-deriving the rule, and the release verifier pins
  the name.
- `FormProviderEndpointResolver` and the provider-aware render variants a host uses to close a
  provider form's submission target: `materialize_component_actions_with_providers`,
  `materialize_project_with_runtime_context_with_providers`,
  `render_page_with_runtime_context_with_providers`,
  `render_runtime_scenario_matrix_with_providers` and
  `RuntimeScenarioRenderSnapshot::capture_with_providers`. Without a resolver every existing entry
  point renders exactly as before (marker-only); with one, each form whose `provider.operation`
  the resolver recognizes is lowered to a native `action`, POST and url-encoded, and the authored
  document is never rewritten.

### Changed

- `fly-leptos` has a conventional crate root. `Cargo.toml` declares `[lib] path = "src/root.rs"`, so
  `root.rs` is the crate root; the layout-shell module it used to pull in through
  `#[path = "lib.rs"] mod foundation;` is now the ordinary `src/foundation.rs` module. Nothing
  leaves the public API: the shim `real_dom_inline.rs` duplicated names that `pub use fly_web::*`
  already exported (and needed two `#[allow(unused_imports)]` suppressions to stay quiet), and
  `#![deny(unsafe_code)]` moved to the crate root so it covers the feature-gated browser modules
  rather than only the included file.
- `FlyEditor::apply` performs one deep document copy per command instead of three, and `undo`/
  `redo` perform none, by sharing documents between the editor and adjacent history entries via
  `Arc`. History memory roughly halves. The public API is unchanged.
- `maximum_nodes` / `maximum_depth` defaults named and justified (`DEFAULT_MAXIMUM_NODES`,
  `DEFAULT_MAXIMUM_DEPTH`).
- Command history uses `VecDeque`.
- Curated public API for the framework-neutral crates (audit P2-13, API half).
  The crate roots of `fly`, `fly-ui`, and `fly-web` no longer glob-re-export
  their modules (`pub use module::*`): every root re-export is now an explicit,
  rustfmt-ordered list, including the wasm-gated `fly-web` surfaces. The public
  surface is unchanged symbol for symbol (376 + 101 + 43 root names, verified
  by lexical surface comparison; the five verify gates that pinned the glob
  spelling now assert the exported symbols instead). Intra-crate module
  facades (`action`, `command`, `snapshot`, ...) keep their submodule globs —
  the curation covers crate roots, which are the public API.
- Landing/scenario domain extracted from the `fly` core (audit P2-13, domain
  half; DECISIONS/2026-10-14-landing-scenario-domain-leaves-fly-core.md). Nine
  modules — `landing_contract`, `landing_property`, `landing_readiness`,
  `context_scenario`, `runtime_scenario_release`, `runtime_scenario_render`,
  `runtime_scenario_snapshot`, plus `runtime_gate` and
  `interaction_capability_gate`, which consume readiness/scenario items and
  would otherwise force a dependency cycle — moved to
  `rustok-page-builder::domain`. The core root surface shrank from 376 to the
  306 non-domain names (verified identical by lexical comparison);
  `rustok-page-builder` re-exports the 70 moved names at its root, and its
  `runtime_scenario_*` DTO shims keep their public paths over the now-local
  implementation. Pure relocation: no behavior or type-shape changes; cargo
  verification rides the maintainer run (page-builder backlog item 1).

### Known gaps

- `HistoryEntry` still stores whole documents rather than inverse commands, so
  `approximate_bytes` must serialize to measure, and it over-estimates by ~2x now that adjacent
  entries share allocations (which keeps the budget conservative).
- A standalone build (`--no-default-features`) loses CLDR likely-subtag inference: `zh-TW` falls
  back to `zh` rather than `zh-Hant`, and `iw` is not canonicalized to `he`.
- The admin session cookie is still issued by the browser rather than by the server: the admin
  shell mirrors the `localStorage` session into `rustok-admin-session-v1` so SSR can revalidate it,
  which keeps the token reachable by any script on the admin origin. Fly's adapter no longer reads
  or forwards it, but server-issued HttpOnly sessions — and the explicit server-side CSRF check
  that the cookie-only transport now leans on — are the admin shell's change, tracked as
  `CLEAN-060` in `docs/verification/PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md`.
- 48 glob re-exports (`pub use module::*`) remain.
- The provider capability catalog is filled by the RusTok host, not by Fly.
  `rustok-page-builder::interaction_providers` registers the `forms`/`submit` form capability from
  `rustok-forms`, every host gate calls the capability-aware gate (missing provider forms block the
  save/publish gate with the missing capability named, missing provider actions warn), and
  `HostFormProviderEndpoints` binds that provider to the
  storefront, preview, static-artifact and scenario-matrix renders. Fly itself still defaults to
  `InteractionCapabilityPolicy::Allow` with no catalog, so a consumer of the standalone crate keeps
  the permissive contract until it registers its own providers; the two identifiers and the route
  template are kept in sync by `scripts/verify/verify-fly-interaction-capabilities.mjs` rather than
  by a Cargo edge. Release-baseline captures (`RuntimeScenarioReleaseBaseline::capture`) stay
  marker-only: baseline and candidate are compared inside the same renderer, and a provider-aware
  baseline API does not exist yet. See the *Known Limitations* section of `README.md`.
- Custom actions (`emit_event`, `provider_action`) have no runtime here either: materialization
  produces `data-fly-action-kind` plus a JSON `data-fly-action` payload, and no crate listens for
  them, so the button is inert until a host implements the contract. Same section of `README.md`.
