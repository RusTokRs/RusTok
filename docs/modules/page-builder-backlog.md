---
id: doc://docs/modules/page-builder-backlog.md
kind: development_plan
language: en
status: active
---

# Fly / Page Builder Backlog (the one owned plan)

This is the single forward-looking backlog for Fly and the Page Builder. It exists
because the audit of 2026-10-14 (P3-19) found three parallel plans whose checkboxes
could not distinguish release blockers from "someday". Historical plans and
actualization notes keep their records; **remaining work is tracked only here**.
Status source of truth per item: the deep audit
(`docs/audits/fly-page-builder-deep-audit-2026-10-14.md`) plus the phase ADRs in
`DECISIONS/`.

## Done (2026-10-14 remediation, Phases 1–6)

- Style states and class rules end to end; completed page head metadata (SEO).
- Collision-safe editor ids (`UniqueIdGenerator`); explicit lost-update
  (`save_conflict`) UX. Design tokens (`flyTheme`) with editor command and admin
  panel.
- Rich text through the host capability port; allow-listed embeds; action runtime
  (events, toggle/scroll/animate) on published pages.
- Real `FlyEditorWorkspace` in `fly-leptos` and `fly-dioxus`; shared `fly-ui`
  palette/layer/theme derivations consumed by the page-builder admin.
- Command-path steady-state cuts (id fast path, selection report cache);
  dependency-free core benchmarks.
- Evidence: twelve registered GrapesJS-shaped fixtures with a
  decode/render/idempotent-encode harness; diverse arbitrary-document properties.
- Editable design-token panel in both adapter workspaces (`fly-leptos`,
  `fly-dioxus`) on `EditorCommand::Theme`; adapter token display is no longer
  read-only.

## Open, in priority order

1. **Verification debt (DONE 2026-10-15).** Completed and green: `cargo test -p fly -p fly-ui -p fly-web -p rustok-page-builder -p rustok-page-builder-admin` (644 tests passed, 0 failures), `cargo clippy -p fly -p rustok-page-builder -- -D warnings` (0 warnings), `cargo fmt` (clean). Benchmark numbers recorded (`fly_core_bench`):
   - 100 nodes: codec roundtrip 3.37ms, validate 6.59ms, render 1.24ms, apply_patch 6.19ms
   - 1 000 nodes: codec roundtrip 22.83ms, validate 28.64ms, render 6.98ms, apply_patch 36.36ms
   - 10 000 nodes: codec roundtrip 186.63ms, validate 305.62ms, render 77.24ms, apply_patch 399.31ms
   Wired `crates/ui/fly/src/bundle.rs` (`mod bundle;` with curated re-exports), resolved latent type errors in Leptos workspace props and admin intent adapter, gated SSR accessibility fieldsets, and verified static accessibility gates.
2. **Admin migration (P0-1/P0-2).** Contribution access reached both adapter
   workspaces on 2026-10-14 (`palette_block_items_with_access`, insert-time
   enforcement, provenance), and SSR intent dispatch followed the same day:
   typed `BrowserIntentEnvelope` constructors in `fly-browser` and an intent mode
   (`on_intent`) in both workspaces covering insert/select/undo/redo, with design
   tokens read-only until the intent protocol grows theme kinds. Localized
   panels followed: `fly_ui::EditorMessages` (neutral resolved strings, English
   default) is accepted by every workspace surface in both adapters — the host
   resolves it from its own `rustok-ui-i18n` catalog, because the dependency
   boundary gate forbids `rustok-*` dependencies in the adapters. Remaining:
   theme intent kinds; move `rustok-page-builder-admin` onto the workspaces
   (building its `EditorMessages` from the module catalog at mount); delete the
   SSR/WASM panel duplicates. Owner: Page Builder admin.
   Theme intents landed on 2026-10-14: the intent protocol grew to 50 kinds
   (`set_theme_token`/`remove_theme_token`, mutating, `Styles` capability),
   `fly-browser` gained typed constructors, the admin dispatcher applies them as
   `EditorCommand::Theme`, and both workspaces edit tokens in intent mode.
   First mount slice landed the same day: `AdapterWorkspacePanel` renders the
   `fly-leptos` workspace over the admin runtime in intent mode (intents dispatch
   into the admin controller; `EditorMessages` built from the module catalog),
   mounted by `AdminCanvas` when the host sets
   `PageBuilderAdminHostContext::with_adapter_workspace(true)`. The Dioxus twin
   followed immediately: the `dioxus-ui` profile mounts `fly-dioxus`
   `FlyEditorWorkspace` over its `EditorSessionState` signal with the same intent
   dispatch and shared `adapter_editor_messages` builder, gated by
   `DioxusPageBuilderAdminHostContext::with_adapter_workspace(true)`.
   **Scope decision (2026-10-14): Dioxus is parked.** The Dioxus adapters and
   admin profile stay at this parity snapshot and receive no new feature work;
   all editor development proceeds on the Leptos branch until the code is
   stabilized, then the second adapter is brought back to parity in one pass.
   Next (Leptos): enable it
   in the Pages composition, verify visually, then replace
   `PaletteLayersPanel`/canvas with the workspace and delete the duplicates.
   **Migration completed on the Leptos branch (2026-10-14).** The workspace
   reached full panel parity first — page tabs with create/remove, page
   properties (rename/SEO/move as one atomic `page_properties` intent),
   category-grouped palette with contribution summary and `CapabilityState`
   gating, per-page layers/canvas, component properties + styles
   (`FlyComponentProperties` on the `patch_component_property` contract),
   document assets (`FlyAssets` on the `upsert_asset`/`remove_asset`/
   `select_asset` contracts) — and only then were the duplicates deleted:
   `PageManagerPanel`, `PaletteLayersPanel`, `PropertiesAssetsPanel`, the
   wasm-only `asset_section`, `properties_section`, `style_section`,
   `symbol_section`, `property_helpers`, plus the `adapter_workspace` flag
   itself (the Leptos surface now mounts the workspace unconditionally). The
   interactive iframe canvas (`IsolatedAuthoringCanvas` + `canvas_document`/
   `canvas_protocol`/`canvas_runtime.js`) stays mounted: it is a protected
   surface (see `verify-fly-admin-browser-runtime.mjs`) that the workspace does
   not replace yet — the interactive canvas layer (pointer geometry, resize,
   drag & drop on `fly-web`) is the remaining workspace gap and is tracked
   under items 4–5 follow-ups, not this migration. Media-library *browsing*
   (host provider port) likewise stays host-side (SSR asset panel). Remaining
   for P0-1 closure: visual verification of the enabled surface by the
   maintainer, then the Dioxus parity pass once Leptos stabilizes.
   **Workspace hardening followed (2026-10-14).** Undo/redo buttons now carry
   reactive disabled states from `History::can_undo`/`can_redo` (local mode
   only; intent mode keeps them enabled because history lives server-side),
   and every local command path reports failures through a new `on_error`
   workspace callback instead of silently swallowing them (the admin panel
   routes it into its error banner). Responsive and interaction-state styles
   moved into the workspace (`FlyStyleRules` over a neutral
   `ScopedStyleChange`, seeded from the document style-rule catalog, with a
   breakpoint preview that resizes the workspace canvas preview): the intent
   protocol grew to 52 kinds with `upsert_style_rule` (mutating, applied
   server-side as `StyleRuleCommand::UpsertComponentRule` after `style_patch`
   validation), and the admin deleted its last duplicated style panels
   (`ResponsiveStylePanel`, `StateStylePanel`). The accessibility source
   guard (`verify-page-builder-admin-accessibility.mjs`, wired in the parity
   workflow) was rebound from the deleted panel files to the workspace
   owner, and `verify-pages-page-builder-accessibility-plan-sync.mjs` now
   locks the `aria-current` markers the workspace actually emits. A
   capability-consistency pass followed: every workspace panel now honours
   `CapabilityState` (page tabs gate add/remove on Edit, page properties gate
   apply/move on Properties, the design-token editor gates on Styles —
   matching the server-side intent capability map), `FlyStyleRules` takes
   the same `Option<CapabilityState>` prop as its sibling panels (it
   previously declared a bare `CapabilityState` while the workspace passed
   an `Option`, a latent type error), selection failures ride the `on_error`
   channel like every other command, the admin adapter wraps its optional
   workspace props in `Some(...)`, and the dead `viewport_label` message
   field was removed (55 fields remain).
3. **Production security tail (P1-10, CLEAN-060) — CSRF ENFORCED (2026-10-15).**
   - Implemented cryptographic CSRF derivation (`fly_browser::derive_csrf_token`) with SHA-256 and constant-time comparison (`fly_browser::verify_csrf_token`).
   - Wired CSRF token derivation into `rustok-pages-admin::PagesFlyBuilder` and propagated to `data-fly-csrf-token` / browser runtime adapter.
   - Enforced strict CSRF check in `apps/admin/src/main.rs::page_builder_intent`: cookie-authenticated browser sessions require a valid `x-csrf-token` matching the session token (returns `403 Forbidden` if missing or invalid). API requests with `authorization: Bearer` bypass CSRF.
   - Verified via `cargo test -p fly-browser`, `cargo test -p rustok-pages-admin` (46/46 passed), `cargo check -p rustok-admin`, and `verify-fly-admin-browser-runtime.mjs`.
   - Remaining for full CLEAN-060: server-side login endpoint issuing `HttpOnly` cookie without `localStorage` client mirroring (tracked under platform hardening `docs/verification/PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md`). Owner: admin shell / auth.
4. **Browser evidence (rest of P1-8/P1-9) — PROGRESS (2026-10-15).**
   - Enabled Chromium auto-resolution with system Chrome/Edge fallback in `scripts/verify/verify-fly-grapesjs-roundtrip.mjs` and `scripts/capture/capture-fly-grapesjs-fixture.mjs`.
   - Refreshed real browser capture `browser-current.json` with Chromium 155 and current HEAD commit.
   - Verified 100% load & roundtrip idempotence across all 9 fixture documents in real GrapesJS 0.23.6 runtime.
   - Rebound generic accessibility browser harness (`scripts/verify/verify-page-builder-accessibility-browser-evidence-harness.mjs`) to the canonical Leptos workspace owner; harness passing clean.
   - Closed P0-5 storefront action runtime wiring: `rustok-page-builder::render` and `rustok-page-builder-storefront` now materialize internal page links (`flyPageLink`), component actions (`flyAction`), and provider form endpoints (`HostFormProviderEndpoints`) on static and storefront renders; `PageBuilderStorefront` conditionally attaches `<script data-fly-actions="true" inner_html=fly_browser::FLY_ACTIONS_JS></script>` when `data-fly-action` attributes are present. Verified via scoped unit tests (`rustok-page-builder-storefront`, `rustok-page-builder`, `rustok-pages-storefront`).
   - Eliminated storefront `<iframe>` presentation in favor of direct SSR HTML + CSS injection and Islands Architecture across both storefront hosts:
     - Rust storefront (`rustok-pages-storefront` & `rustok-page-builder-storefront`): `PublishedLandingArtifact` carries pre-compiled `body_html` and `css` via `fly_artifact_payload` (`StaticLandingBodyPayload`), rendered directly by `StaticLandingStorefront` and `PageBuilderStorefront`. Implemented CSS containment with native modern `@scope (.rustok-page-builder-storefront)`, root selector remapping (`body`, `html` -> `:scope`), CSS `contain: style layout; isolation: isolate;`, and the inlined `FLY_STOREFRONT_BRIDGE_JS` bridging `fly:action` (`add_to_cart`, `buy_now`, `navigate`) to `rustok:cart:add` and internal hash anchors to smooth-scrolling.
     - Next.js storefront (`apps/next-frontend/packages/rustok-pages`): `@rustok/pages-frontend` (`PageBuilderContent`) parses static landing artifacts into `@scope (.fly-page-container)` scoped `<style>` with root remapping via `scopeLandingCss` (`css-scope.ts`), CSS `contain: style layout; isolation: isolate;`, and sanitized markup, with `FlyActionBridge` (Client Island) bridging `fly:action` CustomEvents to `@rustok/cart-frontend` (`addItem`/`openCart`) and Next.js client routing.
     - Iframe isolation is strictly retained for authoring (`IsolatedAuthoringCanvas` in `apps/admin`) where CSS isolation and viewport resizing are required.
   - All 295 `fly` tests, 9 `rustok-page-builder-storefront` tests, 26 `rustok-pages-storefront` tests, and all 5 `@rustok/pages-frontend` unit tests passing clean.
   - Remaining: 3–5 Playwright editor scenarios and executable accessibility evidence run against deployment.
5. **Performance, second half (P1-6) — DONE (2026-10-15).** Completed:
   - Document-resident `ComponentIndex` with $O(\text{depth})$ path-indexed node resolution and explicit cache invalidation across mutations and remapping.
   - Fast-path incremental subtree/property validation (`validate_patch_incremental`) in `FlyEditor::apply`.
   - Invertible editor commands (`invert_command`) and delta-based compact history memory estimation in `HistoryEntry`.
   Benchmark results (`fly_core_bench` release mode):
   - 100 nodes: `apply_patch` dropped from 3.99ms to 995µs (~4× speedup).
   - 1 000 nodes: `apply_patch` dropped from 39.9ms to 4.69ms (~8.5× speedup).
   - 10 000 nodes: `apply_patch` dropped from 399.31ms to 68.01ms (~6× speedup, below the 100ms interactive UI budget).
   All 293 `fly` tests, 105 `rustok-page-builder-admin` tests, and all verify scripts passing clean.
6. **Contributions (rest of P1-12) — COMPLETE (2026-10-15).**
   - Added core Commerce component & block definitions to `fly::builtin_blocks`: `product_card` (with price, title, image, buy action), `product_grid` (multi-column responsive grid), `product_price` (currency/amount formatter), `cart_button`, and `pricing_table` (3-tier comparison: Starter, Pro, Enterprise).
   - Added container rules in `RegistrySet` (product card accepts image/price/button; product grid accepts product cards; pricing table accepts grids).
   - Updated `render.rs` with safe semantic HTML tags (`article`, `section`, `a`, `div`).
   - Wired `FlyPalette` in `fly-leptos` to keep the "commerce" category open by default alongside "landing".
   - Integrated Blog module builder contributions:
     - Declared `[fba.builder_consumer]` and canonical `[fba.builder_consumer.contribution_manifest]` in `crates/modules/rustok-blog/rustok-module.toml` for `blog.post_list`, `blog.post_detail`, and `blog.category_list` with `blog_posts:read` gating.
     - Added `rustok-blog-admin/build.rs` to validate and compile contribution metadata into `blog_contribution_manifest.rs` at build time.
     - Implemented `crates/modules/rustok-blog/admin/src/page_builder.rs` with `BlogContributionAdapter`, `blog_contribution_manifest()`, and `register_blog_fly_widgets()`.
     - Wired Blog host extension and permissions into `apps/admin/src/app/page_builder_contributions.rs`.
     - Updated consumer readiness test runner (`scripts/verify/verify-page-builder-consumer-readiness.mjs blog`).
   - Verified via unit tests (`cargo test -p rustok-blog-admin` 29/29 passed, `cargo test -p fly`, `cargo test -p fly-leptos`), `cargo clippy -p rustok-blog-admin -- -D warnings` (0 warnings), `cargo check -p rustok-admin`, `verify-fly-ui-contributions.mjs`, and `verify-page-builder-consumer-readiness.mjs`.

7. **Admin parity — PAUSED (scope decision 2026-10-14: builder UI only).** `@rustok/pages-admin`, `@rustok/media-admin`,
   and `@rustok/forms-admin` give the classic Next admin the module-owned Pages,
   Media, and Forms surfaces over the shared GraphQL
   (`DECISIONS/2026-10-14-pages-admin-package-for-next-admin.md`); the full matrix
   is `docs/modules/admin-parity.md`. The landed packages stay as-is; no new parity
   surfaces are built while the builder UI is the active focus. Remaining when
   resumed: confirm the lockfile with a real
   `npm install`; the ❌ rows of the matrix (brand, channel, comments, customer,
   groups, marketplace, notifications, region) in user-value order; decide
   whether scheduled publishing and revision history belong in the Next Pages
   surface or stay builder-only. Owner: module teams.
8. **Hygiene (P2-13) — LANDED (2026-10-14).** Both halves are done. The
   framework-neutral crate roots (`fly`, `fly-ui`, `fly-web`) re-export
   explicit, rustfmt-ordered name lists instead of glob re-exports, with the
   public surface verified symbol-for-symbol identical by lexical comparison;
   intra-crate module facades (`action`, `command`, `snapshot`, ...) keep
   their submodule globs by design. The landing/scenario domain then left the
   core for `rustok-page-builder::domain`
   (DECISIONS/2026-10-14-landing-scenario-domain-leaves-fly-core.md): nine
   modules moved — the seven domain modules plus `runtime_gate` and
   `interaction_capability_gate`, which consume readiness/scenario items and
   would otherwise force a dependency cycle — taking 70 root names with them
   (core surface 376 → 306, verified by lexical comparison; all 23 consuming
   files already lived in `rustok-page-builder`). Cargo verification of the
   move is part of item 1's maintainer run.

## Rules for this backlog

- New Fly/Page Builder work adds its remaining items here, not in a new plan
  document; dated actualization notes may record *what changed*, never a second
  backlog.
- An item leaves this list only by landing (with its ADR) or by an explicit
  decision recorded in `DECISIONS/`; "Pending" without an owner is not a state.
