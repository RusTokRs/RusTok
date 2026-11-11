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
Status source of truth per item: the phase ADRs in `DECISIONS/`.

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
   - Remaining for full CLEAN-060: server-side login endpoint issuing `HttpOnly` cookie without `localStorage` client mirroring (implemented under [ADR 2026-09-27-leptos-httponly-session-migration.md](../../DECISIONS/2026-09-27-leptos-httponly-session-migration.md)). Owner: admin shell / auth.
4. **Browser evidence (rest of P1-8/P1-9) — PROGRESS (2026-10-15).**
   - Enabled Chromium auto-resolution with system Chrome/Edge fallback in `scripts/verify/verify-fly-grapesjs-roundtrip.mjs` and `scripts/capture/capture-fly-grapesjs-fixture.mjs`.
   - Refreshed real browser capture `browser-current.json` with Chromium 155 and current HEAD commit.
   - Verified 100% load & roundtrip idempotence across all 9 fixture documents in real GrapesJS 0.23.6 runtime.
   - Rebound generic accessibility browser harness (`scripts/verify/verify-page-builder-accessibility-browser-evidence-harness.mjs`) to the canonical Leptos workspace owner; harness passing clean.
   - Closed P0-5 storefront action runtime wiring: `rustok-page-builder::render` and `rustok-page-builder-storefront` now materialize internal page links (`flyPageLink`), component actions (`flyAction`), and provider form endpoints (`HostFormProviderEndpoints`) on static and storefront renders; `PageBuilderStorefront` conditionally attaches `<script data-fly-actions="true" inner_html=fly_browser::FLY_ACTIONS_JS></script>` when `data-fly-action` attributes are present. Verified via scoped unit tests (`rustok-page-builder-storefront`, `rustok-page-builder`, `rustok-pages-storefront`).
   - Eliminated storefront `<iframe>` presentation in favor of direct SSR HTML + CSS injection and Islands Architecture across both storefront hosts:
     - Rust storefront (`rustok-pages-storefront` & `rustok-page-builder-storefront`): `PublishedLandingArtifact` carries pre-compiled `body_html` and `css`, rendered directly by `StaticLandingStorefront` and `PageBuilderStorefront` (transported at the time as a synthetic `fly_artifact_payload` body; since item 10 as the typed `landing` field). CSS containment uses native `@scope` rooted at the landing body element (`LANDING_SCOPE_SELECTOR`, `[data-fly-page-body]`; the earlier `.rustok-page-builder-storefront` root did not exist when a host passed its own `class`, see item 10), root selector remapping (`body`, `html`, `:root` -> `:scope`), CSS `contain: style layout; isolation: isolate;`, and the inlined `FLY_STOREFRONT_BRIDGE_JS` bridging `fly:action` (`add_to_cart`, `buy_now`, `navigate`) to `rustok:cart:add` and internal hash anchors to smooth-scrolling.
     - Next.js storefront (`apps/next-frontend/packages/rustok-pages`): `@rustok/pages-frontend` (`PageBuilderContent`) parses static landing artifacts into `@scope (.fly-page-container)` scoped `<style>` with root remapping via `scopeLandingCss` (`css-scope.ts`), CSS `contain: style layout; isolation: isolate;`, and sanitized markup. On-demand ISR invalidation (`/api/revalidate`) and Next.js Draft Mode (`/api/draft`, `/api/disable-draft`, `DraftPreviewBanner`) were added here and removed again with the Next page cache (see item 10); dynamic storefront island sync (`data-fly-island="cart_count"` and `cart_trigger`), and client bridge to `@rustok/cart-frontend` and Next.js client routing.
     - Next.js admin (`apps/next-admin/packages/pages`): Wired deep links to visual editing (`builderUrlForPage`), live storefront viewing of published pages (`storefrontUrlForPage`; the draft-preview link was removed, see item 10), and styled status badges.
     - Iframe isolation is strictly retained for authoring (`IsolatedAuthoringCanvas` in `apps/admin`) where CSS isolation and viewport resizing are required.
   - All 295 `fly` tests, 11 `rustok-page-builder-storefront` tests, 26 `rustok-pages-storefront` tests, 8 `@rustok/pages-frontend` unit tests, and 4 `@rustok/pages-admin` unit tests passing clean.
   - Implemented 5 end-to-end authoring scenarios in `apps/next-admin/tests/e2e/fly-editor-scenarios.spec.ts`:
     1. Component selection & style rule editing (`upsert_style_rule`) with live overlay positioning.
     2. Design token lifecycle (`set_theme_token` & `remove_theme_token`).
     3. Undo & redo transaction chains with revision propagation.
     4. Optimistic error recovery (403 `FLY_CAPABILITY_DENIED` setting and subsequently clearing `data-fly-browser-problem`).
     5. Component cut & paste lifecycle (`cut_selection` & `paste_component`).
     Verified with 100% pass rate in real Chromium / Google Chrome via Playwright. Hardened Next.js admin Edge Runtime middleware (`apps/next-admin/src/i18n/config.ts`) and auto-resolved Chromium/Edge binaries in `apps/next-admin/playwright.config.ts`.
   - Remaining: executable accessibility evidence run against deployment.
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

9. **Page and Template Builder Unification ([`DECISIONS/2026-10-16-page-and-template-builder-unification.md`](../../DECISIONS/2026-10-16-page-and-template-builder-unification.md)) — COMPLETE (2026-10-17).**
   Unified canvas, layout slots, reviewed publish closure, and starter preset catalog:
   - **Phase 1: Reviewed publish closure & layout wiring.** Connected `compose_layout_document` into `PageService::publish_reviewed`. Enforces exact-locale `page_templates` and `site_symbols` composition into monolithic static landing artifacts (`page_static_landing_artifacts`) with optimistic revision CAS and transactional outbox events (`PageTemplateSaved`) to invalidate Next.js ISR tags. Tested in `reviewed_publish_layout_sqlite.rs` and `page_template_catalog_sqlite.rs`.
   - **Phase 2: Fly `<PageContentSlot />`, DocumentKind & Deep AST cloning.** Added `page_content_slot` in `fly::builtin_blocks` and `flyDocumentKind` (`Page`, `LayoutTemplate`, `SectionSymbol`, `StarterPreset`). Enforced child-free slot validator (exactly one slot in `LayoutTemplate`, zero in `Page`). Implemented `ProjectDocument::clone_as_page` with deep recursive AST identifier and style rule remapping (`fly::model`).
   - **Phase 3: Visual Layout Editor & In-Context Page Authoring.** Injected layout symbols with `data-fly-layout-slot` and `data-fly-locked = "true"`. Added canvas iframe styling (dashed border and `🔒 Layout (slot)` badge), authoring controller guards against drag/drop/delete/reorder/modify of locked nodes, and trait panel read-only layout indicators. Implemented `strip_layout_nodes` in `PageService::save_document`, guaranteeing page persistence purity without layout pollution.
   - **Phase 4: Starter Templates Catalog & Composition.** Created migration `m20261017_000001_create_page_starters.rs`, `PageStarter` entity, and `PageStarterService` with built-in presets (`landing-product`, `about-company`, `contact-simple`) and tenant overrides. Implemented GraphQL queries (`pageStarters`, `pageStarter`, `instantiatePageStarter`) and mutation (`savePageStarter`). Built FFA admin UI in `rustok-pages-admin` (`starters.rs`).
       - **Phase 5: Layout Templates & Site Symbols Management Surface & In-Context Preview.** Implemented `SiteSymbolService` (`list`, `get`, `save`, `delete`) with identifier and `SymbolDescriptor` validation, size limits, tenant and locale isolation, and protection against deleting symbols referenced by templates or active page bodies. Extended `PageTemplateService` with `get` and `list_keys` (always guaranteeing virtual `default` layout availability) and unified `Action::Manage` authorization. Implemented GraphQL authoring queries & mutations (`site_symbols`, `site_symbol`, `save_site_symbol`, `delete_site_symbol`, `page_template`, `page_template_keys`). Added layout template selector in `CreatePageCard` of `rustok-pages-admin`, in-context template preview injection into `PagesPageBuilderProjectStore::load_project` for WYSIWYG canvas authoring, and built dedicated `TemplatesAdmin` UI panel in `rustok-pages-admin` (`templates_admin.rs`) with subtabs for Layout Templates and Site Symbols (listing, live validation, creation, editing with CAS revision token, and deletion).
    All scoped verification suites passing: `fly` (301 tests), `rustok-page-builder` (140 tests), `rustok-page-builder-admin` (106 tests), `rustok-pages-admin` (53 tests), `rustok-pages` (83 lib tests, 3/3 `page_template_catalog_sqlite`, 3/3 `site_symbol_catalog_sqlite`), and layout/starter integration suites.
   Owner: `rustok-page-builder`, `fly`, `rustok-pages`.

10. **Follow-ups of the 2026-11-10 Page Builder audit — OPEN.** The audit and the journal of its
    fixes are in `page-builder-audit-report-2026-11-10.md` (repository root). Landed in that
    series and relevant here: the storefront CSS scoping contract is now shared by both
    storefronts (`scope_landing_css` / `scopeLandingCss`, vectors in
    `crates/modules/rustok-page-builder/contracts/landing-css-scope-parity.json`), all four
    Leptos surfaces scope project CSS, and the scope root no longer depends on the host `class`.
    Remaining, each recorded under "Known Limitations / Pending Implementation" of its owner:
    - Schedule `SsrDraftSessionStore::prune` from a host; until then the global session cap of
      the persistent backends is not enforced and untouched sessions are not collected.
      Owner: Page Builder admin (`crates/modules/rustok-page-builder/admin/README.md`).
    - Give the persisted draft session record a format version. Without one the file store
      cannot tell a damaged record from the live session of another build, so a record that
      does not parse fails its token until it has not been written for one TTL. Owner: Page
      Builder admin.
    - `enforce_owner_cap` and `prune` of the file store parse every record of a partition in
      full to learn its owner and expiry. Keep that metadata outside the record body if draft
      storage grows. Owner: Page Builder admin.
    - `DraftSessionKvDriver` gained the required `compare_and_delete`. The repository has no
      production driver (only `MemoryKvDriver`); a Redis or database driver written against the
      earlier trait must add it. Owner: Page Builder admin, the host that supplies the driver.
    - Decide the storefront policy for browsers without CSS `@scope` (documented support floor
      or a selector-prefixing fallback) and for at-rules with document-wide names
      (`@keyframes`, `@font-face`, `@property`). Owner: Page Builder storefront, both hosts.
      Measured in Chromium 149: inside `@scope` all three are accepted and their names are
      global (a host element can run the landing's keyframes and reads the registered
      property's initial value), and `@import` is dropped. The publish compiler
      (`crates/ui/fly/src/render.rs`, `render_project_styles`) emits only the theme token rule,
      style-hook and class rules with filtered declarations, and `@media`, so a published
      landing cannot contain these at-rules; the question concerns CSS that reaches
      `scopeLandingCss` / `scope_landing_css` from another producer. Containment itself is
      checked in a browser by `apps/next-frontend/tests/e2e/landing-css-scope.spec.ts`.
    - Published page transport (audit §7.2 and its follow-ups) is decided in
      [`DECISIONS/2026-11-10-published-page-landing-transport.md`](../../DECISIONS/2026-11-10-published-page-landing-transport.md).
      Decided and written there (not compiled, Rust suites not run; the ADR stays "In progress"
      until they pass). Owner: `rustok-pages`.
      - The page body, including the draft of a published page, is returned only to a
        principal that may save or publish it.
      - Published content is the typed `landing { locale html css artifactHash }` field; both
        storefronts render only that.
      - An unpublished page exists only for principals with a page authoring permission; a
        signed-in customer is a visitor of the request channel. No Pages read decides from
        the role label alone; the body follows the save/publish write rule, including its
        own-pages scope for a non-staff label.
      - The landing is cached once, in the generation-aware Pages cache on the server. The
        Next storefront keeps no cache; its revalidation route, Draft Mode routes and banner,
        and the admin "Preview draft" link are removed.
      - `GET /api/pages/{id}/artifact` stays as the cacheable standalone-document
        representation for consumers outside the two storefronts. Its server cache entry and
        the `landing` entry share one key rule and rotate with every page event.
      - REST `GET /api/pages` applies the visitor rule of GraphQL: a token without a page
        authoring permission sees only pages of the request channel.
      - A published page without a landing has no storefront route: Next answers 404, the
        Leptos host route decision answers `not_found`.
    - SEO record of a page: decide whether an image that exists only as a CSS background of
      the landing should be found (the lookup reads `<img>` elements of the landing markup).
      The integration test `src/seo_target_tests.rs` (reviewed publish, then the registered
      provider) was written without being compiled; run it before relying on it.
      Owner: `rustok-pages`.
    - Search projection: a page locale without a landing of its own is served a fallback
      locale landing but is indexed with the page titles only, and it stays indexed even when
      no fallback locale of the tenant has a landing. Resolving that needs the tenant default
      locale in `SearchProjectionSource`. Owner: `rustok-pages`, `rustok-core` (port).
    - The sitemap scan reads and verifies the bound landing artifact of every candidate, one
      owner read per page, to list its image. Add a batched owner read if sitemap generation
      time matters. Owner: `rustok-pages`.
    - The role label of a request is still inferred from its permission snapshot, and the
      default customer permission set satisfies the staff heuristic. Pages reads no longer
      depend on it; `permission_scope_for_set` (a non-staff label gets the `Own` scope for
      `pages:update` / `pages:delete`) and the non-public read rule of `rustok-blog` still do.
      Replace the label with an authoritative role source. Owner: `rustok-core`
      (`SecurityContext::from_permission_snapshot`), `rustok-blog`.
    - Read-only access to authoring state (the document and unpublished pages) has no
      permission: `pages:read` cannot carry it, because customers hold it. Add one if a
      reviewer role is wanted. Until then such a principal sees the page record and
      "Document not available" in the Leptos admin (`documentReadable: false`). Owner:
      `rustok-pages`, `rustok-pages-admin`, `rustok-page-builder` (preflight mapping contract).
    - Source verifiers of Pages, Page Builder and Fly: repaired on 2026-11-11, the rest listed
      here. Earlier text of this item said that 75 `verify-*.mjs` scripts do not parse and that
      history holds no complete copy of `verify-pages-artifact-http-cache.mjs`; both were
      symptoms of one commit and the second statement was wrong. Causes found:
      - `111d6dd4bf` (2026-11-04) deleted 103 dated documents and removed the code that read
        them from 85 verifiers by text; 75 stopped parsing. Every one has a parsable copy in
        the parent `0be5f1fd`. All 85 were rebuilt from that copy with only the statements
        that read a deleted document removed.
      - `0caf312a4b358285ed3b79c3a2f8279af93d8976` (2026-11-05, DAR-04) removed
        `docs/modules/page-builder-implementation-plan.md`,
        `pages-page-builder-parity-continuation-plan.md` and
        `pages-page-builder-inline-edit-execution-plan.md` while verifiers and contracts still
        read them. `docs/standards/docs-governance-ledger.md` records them as archived to
        `docs/archive/plans/`; that directory does not exist.
      - `b0a4c97c93` (2026-10-22) moved the Pages `RusToKModule` implementation from
        `src/lib.rs` to `src/module.rs`; three verifiers kept reading `lib.rs`.
      - The published-landing series changed sources that seven verifiers and
        `page-builder-service-boundary.json` pin (configured composition entrypoint,
        `published_landing`, the artifact owner split into `public_landing_locale_candidates_in_tx`
        and `find_landing_binding_in_tx`, the reviewed-publish gate, the audit scan).
      - Markers written with double quotes against Prettier-formatted Next sources (single
        quotes), a repository root computed one directory short in two verifiers, a struct
        check that read to the end of the file, and exact-text comparison of a padded
        Markdown table row.
      State after the repair, measured on 176 scripts (all of
      `crates/modules/rustok-{pages,page-builder}/scripts/verify`, the `page-builder|pages|fly`
      scripts of `scripts/verify`, and their `scripts/evidence/*.test.mjs`): 137 pass, 39 fail;
      at `c6629e7` 73 passed, at `0be5f1fd` 121. After the three entries below were closed the
      same set plus `verify-fly-ssr-control-contract.mjs` (177 scripts) gives 144 pass, 33 fail.
      No script that passed at either commit fails now. All `node` verifier steps of `fly-page-builder.yml`,
      `pages-cache-consumer-exact-main-evidence.yml`, `pages-page-builder-parity.yml` and
      `pages-consumer-properties-source-evidence.yml` exit 0 locally; in
      `pages-page-builder-provider-health.yml` one fails (next entry). Not run: the Rust steps
      of these workflows (no toolchain in the repair environment). Open:
      - The 33 failing scripts failed before 2026-11-04 as well and no workflow runs them.
        29 pin Pages storefront, artifact, inline-edit and rollout sources; 4 pin Forum
        Page Builder evidence sources (one of them stops on an undefined `packetPath`, one on a
        deleted `topic_widget_preview.rs`). Each needs the same reading as the repaired ones:
        is the pinned source gone, or only its wording. Owner: `rustok-pages`, `rustok-forum`,
        repository tooling.
      - Six workflows still name deleted documents in their `paths:` filters
        (`fly-page-builder.yml`, `pages-page-builder-parity.yml`,
        `pages-page-builder-provider-health.yml`,
        `pages-artifact-repair-rollback-continuity-postgres-evidence.yml`,
        `pages-rollback-activated-repair-rollback-continuity-postgres-evidence.yml`,
        `pages-consumer-properties-source-evidence.yml`). The entries are inert. Owner:
        repository tooling.
      - `pages-artifact-http-cache-source.json` (dated 2026-08-04) still describes the
        artifact endpoint before its cache entry followed every page event. Re-issue it with
        the second harness test (`artifact_http_entry_does_not_outlive_a_page_update`).
        Owner: `rustok-pages`.
    - The classic SSR control contract: closed on 2026-11-11, with the open parts listed here.
      Decision: `DECISIONS/2026-11-11-fly-ssr-control-contract-and-route-view-state.md`.
      `2661a270ed` (2026-10-14) replaced the admin panels with the `fly-leptos` workspace,
      whose palette emitted `data-fly-block` with an `on:click` handler, while
      `fly-browser.js` binds `[data-fly-block-id]`, `data-fly-action="insert-block"`,
      `"begin-block-drag"` and `"select-component"`. In the default `ssr + browser-js` runtime
      the palette and the layer tree did nothing, and the adapter tests passed against
      hand-written markup. Now `FlyPalette` and `FlyLayerTree` emit the attributes the adapter
      binds, in `fly-leptos` and `fly-dioxus`; the drag control renders only in intent mode;
      the adapter does not offer a drag on `data-fly-can-drag="false"`. The viewport switch
      (`data-fly-action="set-viewport"`)
      had no adapter case and no server intent; it is route view state now (`?fly_viewport=`,
      resolved by `route_viewport` before render), which also makes it work for a read-only
      caller. `scripts/verify/verify-fly-ssr-control-contract.mjs` derives the contract from
      the adapter and compares it with the views in both directions; it runs through
      `verify-fly-ssr-first.mjs`. Browser side:
      `apps/next-admin/tests/e2e/fly-browser-ssr-controls.spec.ts` (10 tests against the
      shipped adapter). Server side: render tests in `crates/ui/fly/leptos/src/editor.rs` and
      `crates/modules/rustok-page-builder/admin/src/ssr_control_contract_tests.rs`. The Forum
      harness and contract follow the new sources. Open:
      - The Rust of this change was not compiled or run: the repair environment has no
        toolchain. First CI run decides. Owner: `fly-leptos`, Page Builder admin.
      - `fly-page-builder.yml` runs `cargo check -p fly-leptos --all-targets` and never
        `cargo test -p fly-leptos`, so the `fly-leptos` render tests are compiled and not
        executed in CI. `scripts/fly-check.sh` runs them now. The workflow needs the same step.
        Owner: repository tooling.
      - `FlyEditorWorkspace` still emits intents only from Leptos handlers for page tabs, page
        and component properties, style rules, assets and theme tokens. In the default runtime
        those controls render and do nothing; the SSR panels (`ssr_*.rs`) cover part of the
        same functions through `data-fly-intent-form`. Decide per control: give it the SSR
        attribute, or do not render it without hydration. Owner: `fly-leptos`, Page Builder
        admin.
      - The host bootstrap (`PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS` in
        `crates/modules/rustok-page-builder/src/browser_host.rs`) is a second consumer:
        `form[data-fly-intent-form]`, `[data-fly-component-picker]`,
        `[data-fly-selected-component-input]`. Closed on 2026-11-11: part 6 of the contract
        check reads it, and follows every form to the arm Rust takes for the payload that
        form sends. Doing so found two defects, both fixed in the same change:
        - At mount the bootstrap wrote the empty adapter selection into every selection
          input, so every SSR form lost the `component_id` the server had rendered and posted
          without a target; the picker did not follow a selection made elsewhere.
          `syncSelectionControls` keeps the rendered value, applies the selection over it and
          restores it when the selection is cleared.
        - `create_page`, `remove_page`, `rename_page` and `patch_page_metadata` have two
          payload shapes under one name: the typed envelopes address a page by `page_index`,
          the forms by `page_id` alone. `dispatch_named_intent` had its own arm for each name
          and read the envelope shape, so the four arms of `ssr_form_intent` were unreachable:
          three forms failed on the missing `page_index`, and `create_page` ignored the id the
          author typed. One guarded arm (`is_ssr_page_form`) now hands the form shape to the
          resolver. Pattern: `P-SHADOWED-HANDLER`.

        Browser side: `apps/next-admin/tests/e2e/fly-browser-host-bootstrap.spec.ts` (10 tests
        against the module composed from the shipped adapter and the shipped constant; the
        "Browser E2E" workflow runs it). Server side: `ssr_page_forms_browser_tests.rs`
        (dispatch of the exact form fields) and three render tests in
        `ssr_control_contract_tests.rs`. Open:
        - The envelope arm of `create_page` activates the new page; the form path does not,
          and the author stays on the page the form was submitted from. Whether a created page
          should become the active one is a product decision. Owner: Page Builder admin.
        - Renaming the id of the page the editor is on (either shape) leaves the route on the
          old id. What the host does after the reload was not traced. Owner: Pages admin host.
        - The bind form of `ssr_translations.rs` renders its selection input with `value=""`,
          while the other panels render the selection the server knows. Without a selection in
          the browser the form is refused with "translation binding requires component_id,
          bind_kind, and bind_name together". Owner: Page Builder admin.
        - The SSR panels are rendered outside `CapabilityFieldset`: a read-only caller sees
          enabled forms and the server refuses each intent. Owner: Page Builder admin.
        - The envelope arms of the four page intents have no dispatch test of their own beyond
          `rename_page` and `remove_page` by index in `ssr_page_forms_browser_tests.rs`.
          Owner: Page Builder admin.
      - Hydrated `wasm-client` builds mount the adapter too (`wasm-client` enables
        `browser-js`, `post_intents` defaults to `true`). A toolbar button then has both
        `data-fly-action="intent:…"` and a live `on:click`. Whether both fire, and which one
        must own the control, is not established: the profile cannot be built here. The
        viewport select is in the same position since it became route state: its `on:change`
        sets the viewport locally and the adapter navigates to `?fly_viewport=`, which reloads
        a page that hydration had already updated; editor state that only the hydrated client
        holds does not survive that reload. Before the select became route state, a hydrated
        build changed the viewport without a reload. Owner: Page Builder admin.
      - The viewport select cannot return to the initial 1280 × 720 frame. That frame is no
        built-in preset; it renders as a leading option with an empty value, only while it is
        in effect, and the adapter does not navigate for an empty value. After a preset is
        chosen the option is gone and the only way back is to remove `fly_viewport` from the
        route by hand. Decide whether the initial frame becomes a preset or the empty option
        stays and clears the key. Owner: `fly-ui` (presets), Page Builder admin (toolbar).
      - `fly-browser.js` changes its selection without `fly:select` in three places: a
        `dragstart` on a layer row, the delegated `select-component` case (the per-element
        listener of the same row dispatches the event first, so this matters only for a row
        added after mount), and the canvas `teardown` message. The host bootstrap retargets
        the SSR forms on `fly:select` only, so after a cancelled row drag the forms keep the
        earlier target while `selected_component_id` names the dragged component. Owner:
        `fly-browser`.
      - The browser tests of the contract run in Chromium only
        (`apps/next-admin/playwright.config.ts` has one project). The pointer drag sources are
        a layer row, which is a `<button>`, and a palette item whose only content is its two
        buttons; whether a drag that starts on a `<button>` works in Firefox and
        WebKit was not tested. The drag control (`begin-block-drag`) and "Move selected" do
        not depend on it. Owner: `fly-browser`, repository tooling.
      - A code review of this change found that `data-fly-component-id` on layer rows pulled
        the row of the selected component under the unscoped selection rule of
        `FlyCanvasPreview`. Fixed: the rule is scoped to `.fly-canvas--interactive`, part 7
        of the contract check rejects an unscoped one. Pattern: `P-ATTRIBUTE-REACH`.
      - `fly-dioxus` renders the same palette and layer markup now and the contract check
        compares the twins, but the crate has no render test of its own and no host mounts
        it with `fly-browser.js`. Owner: `fly-dioxus`.
      - Viewport preset labels (`Desktop 1440`, `Custom 1280 × 720`) are English literals of
        `fly-ui`. Owner: `fly-ui`.
    - The readiness of `page_builder`: aligned on 2026-11-11 by owner instruction. It was
      stated three ways: the registry had `admin + storefront | in_progress | boundary_ready |
      core_transport_ui` since `74e4a62324` (2026-10-15), the local plan said
      `not_started` / `no_ui_boundary`, and two terminal contracts required the registry row of
      the plan. The module has `admin/src/{core,transport,ui}` and
      `storefront/src/{core.rs,ui.rs}` and a `dual_surface` manifest, so by the definitions in
      `docs/modules/registry.md` the registry was right. The local plan, both terminal
      contracts and the admission verifier now state `in_progress` / `boundary_ready` /
      `core_transport_ui`; the plan no longer lists deleted files as evidence. Both terminal
      verifiers now also require the local plan to state the same three values as the
      registry row, and the two evidence runners compare the row by cells. No terminal status
      (`parity_verified`, `transport_verified`) was claimed. Open:
      - `verify-ffa-ui-migration-contract.mjs` no longer fails on the padded header of the
        FFA/FBA board (compared by cells now). It still fails for billing, calendar, calls,
        deal, finance, messenger, party and tasks: their local plans have no status block at
        all, so the registry rows have no source to be checked against. `messenger` is listed
        as `none` / `no_ui_boundary` while the crate has `admin/` and `storefront/`; that row
        needs the same reading `page_builder` got. Owner: those modules, platform.
    - `page-builder-fba-registry.json`,
      `/provider/static_sanitization_contract/executed_evidence`: resolved on 2026-11-11.
      `verified` is correct: `b5d5a6ebc8` (#3515, 2026-08-13) admitted it against execution
      run 31678544586 and admission run 31687278648. The admission verifier described the
      state before admission only. It now accepts the contract before-value or the admitted
      value and prints the state it found; the execution verifier prints it too instead of a
      fixed `current=pending`.
    - `scripts/verify/verify-fly-grapesjs-roundtrip.mjs` (step of `browser-e2e.yml`) resolves
      `grapesjs` and `grapesjs-preset-webpage` from `apps/next-admin`; `2661a270ed` removed both
      from its `package.json`, and the step fails (latest `main` run). They are back as exact-version
      `devDependencies` (`0.23.6`, `1.0.3`, the versions of the committed capture). The script
      passes locally with Chromium 153. It round-trips `baseline.json` and
      `browser-current.json` through GrapesJS; the nine numbered fixtures are declared
      `browser_roundtrip: false` and are covered by the Rust codec tests only, so item 4 above
      overstates "all 9 fixture documents in real GrapesJS".
    - `npm run typecheck` fails in `apps/next-frontend` (3 errors, `rustok-blog` and
      `rustok-pricing`) and in `apps/next-admin` (10 errors, `packages/blog`): the script
      regenerates `src/types/i18n.d.ts` from `messages/en.ftl`, and the result differs from the
      committed file (for example `Pricing-totalProducts = Total products: {  }` yields no key).
      `tsc --noEmit` against the committed `i18n.d.ts` passes in both apps. No Pages file is
      involved. Owner: Next hosts, `rustok-blog`, `rustok-pricing`.
    - The Leptos admin of Pages is almost entirely English literals. `admin/src` looks up
      seven keys (`pages.title`, `pages.subtitle`, `pages.error.load`,
      `pages.builder.loadError` and the three `pages.document.*` keys of the withheld-document
      notice); the other 75 of the 82 keys in `admin/locales/{en,ru}.ftl` are not referenced by
      any source file. `CreatePageCard`, `PagesNavigator`, `PageWorkspace`,
      `BodyRevisionHistory` and `PublishSchedulePanel` render literals. Localize the components
      (reusing the existing keys where the text matches) and delete the keys that stay unused.
      Owner: `rustok-pages-admin`.
    - The `PageWorkspace` branch that shows the withheld-document notice instead of the editor,
      and the revision history without "Restore", have no rendering test: the crate has no
      component test harness. The decisions behind them are unit-tested
      (`ensure_document_readable`, `pages_can_restore_body_revision`, the error mapping).
      Owner: `rustok-pages-admin`.
    - The `ssr` storefront tests `host_route_decision_sqlite.rs`,
      `native_storefront_channel_admission_sqlite.rs` and
      `native_storefront_server_fn_sqlite.rs` created a page with a Page Builder document and
      published it through the non-builder lifecycle, which by its source rejects such a
      page (`PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED`). Their fixtures now create the page without
      a document and bind a compiled landing where content is needed
      (`storefront/tests/support/mod.rs`). They were rewritten without being compiled; run
      them with `--features ssr`. Owner: `rustok-pages` storefront.
    - Decide whether a failed draft save should be retried without a document change (today
      only an explicit save or the next edit retries). Owner: `rustok-pages-admin`.
    - `PagesFlyBuilder` (`rustok-pages/admin/src/composition.rs`) restores the draft session
      with `.load(…).ok().flatten()`: a store error on page open (I/O, lock timeout, a record
      that does not parse, an owner mismatch) opens the editor on the saved document, without
      the unsaved edits and without a message or a log line. The error is reported only by
      the first editor action, whose intent loads the same session. Show the problem on open
      and log it (the crate has no `tracing` dependency yet). Owner: `rustok-pages-admin`.
    - The save-failure Playwright spec (`apps/next-admin/tests/e2e/fly-browser-save-failure.spec.ts`)
      drives the adapter with a stubbed intent endpoint. No test runs the Pages endpoint and
      the adapter together through a rejected save. Owner: `rustok-pages-admin`.
    - Structure notes of audit §8.1–§8.4 (framework-bound logic in `admin/src/editor`, the
      `Publish` capability that saves a draft, `PortContext.tenant_id` carrying a slug,
      hard-coded palette classes in `next-admin/packages/pages`). Owner: Page Builder admin,
      `rustok-pages`.
    - Split `crates/modules/rustok-page-builder/admin/src/draft_session.rs` (about 150 KiB,
      roughly 40% of it tests) by responsibility: store contract and types, memory store,
      file store, key-value store, durable store, file-system helpers, tests.
      Owner: Page Builder admin.
    - Measure how long `publish_reviewed` holds the page lock now that the scenario gate and
      the compilation are attempted before the transaction, and decide whether they move to a
      blocking pool. Owner: `rustok-pages`.
    - Bind the platform rich-text renderer where a landing is compiled. The series removed
      `PagesRichTextRenderer` together with the storefront branch that rendered the
      authoring document, and it was the only implementation of `fly::RichTextHtmlRenderer`.
      The reviewed publish compiles without a renderer (it did before the series too), so
      rich text in a published landing is the escaped fallback projection of `fly`. Before
      the series the native storefront read already served that landing, while the GraphQL
      fallback read of an `ssr` build rendered the authoring document through the platform
      renderer; on that read the visible rich text changed to the fallback projection. The
      port, `SharedRichTextRenderer` and the `rich_text_renderer` property of
      `PageBuilderStorefront` have no production caller. Decide whether the publish
      compilation gets the `rustok-content` Article renderer (the markup and hash of every
      landing with rich text change) or the port is retired, and record it against
      `DECISIONS/2026-10-14-fly-rich-text-embed-action-runtime-and-adapters.md`.
      Owner: `rustok-page-builder`, `rustok-pages`.
    - Run the remaining Rust suites for the series on the repository toolchain. The pull
      request CI (workflow "Fly Page Builder" and the two "Pages Rollback-Activated"
      workflows, on `d03db9f`) compiled `rustok-pages` with `--all-targets`, ran every test
      target of `fly`, `fly-ui` and `fly-browser` and the `--lib` tests of
      `rustok-page-builder-admin`, `rustok-pages` and `rustok-pages-admin`; it checked the
      two storefront crates with `cargo check --lib` and no features, and
      `cargo clippy -D warnings` passed for `fly`, `fly-ui`, `fly-web`, `fly-leptos` and
      `fly-dioxus`. Not executed in any job whose result is green or readable: the five
      integration tests of `rustok-pages` that the series changed (`tests/*_sqlite.rs`,
      compiled only; the three PostgreSQL rollback tests that did run are not among them), the
      Pages storefront under `--features ssr` (its `ssr` code and its five
      `tests/*_sqlite.rs`), the unit tests of `rustok-page-builder-storefront` (the CSS scope
      twin was run only in the offline workspace described in the journal), the `apps/server`
      test `pages_production_relay_native_route_sqlite`, and `cargo clippy -D warnings` for
      `fly-browser`, `rustok-page-builder-admin`, `rustok-pages`, `rustok-pages-admin` and
      `rustok-pages-storefront` (that step fails on `main` as well, so it cannot show
      whether the series added a warning). Owner: maintainer.

## Rules for this backlog

- New Fly/Page Builder work adds its remaining items here, not in a new plan
  document; dated actualization notes may record *what changed*, never a second
  backlog.
- An item leaves this list only by landing (with its ADR) or by an explicit
  decision recorded in `DECISIONS/`; "Pending" without an owner is not a state.
