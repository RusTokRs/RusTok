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
- Pages scheduled publishing is driven by the deployed host: with the `runtime`
  feature (enabled by `mod-pages`) `rustok-pages` publishes
  `PageScheduledPublishWorkRegistration` through
  `rustok_runtime::ModuleWorkRegistrations`, and discovery claims a tenant for a
  due job or for an `executing` job whose lease expired, so a publication
  stranded by a crashed worker is reachable while no other Pages work exists.
  Executions are paced by `PAGE_PUBLISH_SWEEP_INTERVAL_SECS`. Statically
  verified only: the SQLite cases in
  `crates/modules/rustok-pages/tests/page_publish_scheduler_sqlite.rs` are
  written but were not executed (no Rust toolchain in the authoring environment).

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
    - ~~Schedule `SsrDraftSessionStore::prune` from a host.~~ The standalone admin starts
      `DRAFT_SESSION_PRUNE_INTERVAL`-paced pruning (`apps/admin/src/app/draft_retention.rs`) over
      the store the intent endpoint resolves, so the global session cap of the persistent backends
      applies and untouched sessions are collected. A host that embeds the editor without that
      binary still owns the call; the owner README says so. Remaining owner: Page Builder admin
      (`crates/modules/rustok-page-builder/admin/README.md`) for the embedded-host contract.
    - ~~Give the persisted draft session record a format version.~~ Done 2026-11-12:
      `PersistentDraftSessionRecord::format` (`DRAFT_SESSION_RECORD_FORMAT = 1`) with
      `PersistentDraftSessionRecord::decode`, so a record of a newer build parses and is answered
      with `SsrDraftSessionError::UnsupportedFormat` (HTTP 503) instead of being reported as
      damage and aged out; records without the field are format `0` and are rewritten with the
      current format by the next commit. Owner: Page Builder admin.
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
      - ~~Six workflows still name deleted documents in their `paths:` filters~~
        Done 2026-11-12: the entries are gone from `fly-page-builder.yml`,
        `pages-page-builder-parity.yml`, `pages-page-builder-provider-health.yml`,
        `pages-artifact-repair-rollback-continuity-postgres-evidence.yml`,
        `pages-rollback-activated-repair-rollback-continuity-postgres-evidence.yml` and
        `pages-consumer-properties-source-evidence.yml` (30 lines, in both the `push` and the
        `pull_request` filter of each). They were inert — a path that cannot match never fires —
        but each one is a filter entry that reads as a live trigger for a document that no longer
        exists. No workflow was run; the remaining entries of every filter name an existing
        path. Owner: repository tooling.
      - ~~`pages-artifact-http-cache-source.json` (dated 2026-08-04) still describes the
        artifact endpoint before its cache entry followed every page event.~~ Done 2026-11-12:
        the packet is re-issued (`generated_at` 2026-11-12) with the second harness test in
        `harness.tests` (`artifact_http_entry_does_not_outlive_a_page_update`) and its five
        claims (`page_update_rotates_a_generation_the_entry_binds`,
        `page_and_route_generation_changes_age_the_entry_out`,
        `cache_hit_entry_does_not_survive_a_page_update`,
        `stale_entry_answers_404_instead_of_the_cached_document`,
        `missing_artifact_is_never_cached`);
        `verify-pages-artifact-http-cache.mjs` now pins that second test body as well, so the
        packet cannot drift from the harness again. The SQLite harness itself still has not
        run. Owner: `rustok-pages`.
      - ~~`verify-pages-immutable-artifact-integrity-audit.mjs` pinned a two-phase scan the
        command no longer performs.~~ Done 2026-11-12, and the source was the side that was
        wrong. The packet and the contract promise "the first query projects artifact
        identifiers only … each selected full artifact payload is then loaded and verified one
        record at a time", but the command selected the whole entity and popped the lookahead
        row after it had been hydrated, so a page with large documents materialised every
        HTML/CSS/runtime payload of the slice before discarding one. The command now selects
        `Id` only (`select_only` + `into_tuple::<Uuid>`), keeps the `created_at, id` order and
        the `max_records + 1` lookahead, and loads each payload through
        `find_by_id` fenced by tenant and page, under the same shared-lock rule as before.
        `verify-pages-immutable-artifact-integrity-audit.mjs` passes unchanged. The SQLite and
        Postgres harnesses were not run (no toolchain here). Owner: `rustok-pages`.
      - ~~Three Pages routing verifiers pinned wording that no longer describes the source.~~
        Done 2026-11-12. `verify-pages-published-slug-route-alias.mjs` asserted batching by
        counting `.all(txn)` across a whole function, which also counts the two queries the
        function needs before its loop, and it expected the Forum helper `matches_module_path`;
        it now slices the brace-balanced loop body, so only a query inside the loop fails, and
        pins the Pages owner's own route parse (`parse_page_route`). The local plan gained the
        section the other two verifiers read (`historical route import owner`, provenance
        receipts, `Automatic inference remains open by design`, immutable redirects for
        published slug renames, old claims not reusable, delete tombstones). No behaviour
        changed in either case: the source was already correct, the pins were not.
      - ~~Nine more failing verifiers, all with a source that moved or was rewritten.~~ Done
        2026-11-12, one marker at a time, none deleted:
        `verify-pages-immutable-artifact-integrity-audit-postgres`,
        `verify-pages-immutable-artifact-integrity-audit-transport` and
        `verify-pages-explicit-artifact-repair-transport` (the re-exports now carry the utoipa
        `__path_` items, and rustfmt split the page-query closure across two lines),
        `verify-pages-inline-edit-release-composition`,
        `verify-pages-inline-edit-asset-delivery` and
        `verify-pages-inline-edit-artifact-http-evidence-harness` (the asset response and the
        client builder moved to `rustok_web::embedded_asset_response` and
        `apps/storefront/scripts/build-wasm-client.mjs`),
        `verify-pages-authenticated-inline-consumer` (the module impl moved to `module.rs`, and
        the pinned `ensure_document_is_mutable` helper has never existed — the path reads the
        working copy, draft body first, under `page_document_revision` rather than gating a
        mutability flag) and the two native-storefront verifiers (the harness now asserts
        against the landing JSON and re-publishes through `support::publish_landing`; the
        adapter resolves the channel id before it gates). Sweep after both rounds: 139 pass,
        18 fail, against 126 pass and 31 fail at `f571186`.
      - ~~The remaining eleven failing verifiers.~~ Done 2026-11-12, the same way — a marker
        moves, the guarantee does not. The tenant-rollout gate and the rollout matrix harness
        both read the Pages host's SSR binding: it composes through
        `compose_configured_fly_page_builder_handlers` with its own store, renderer, port
        policies and permission mapping, passing `trusted_rollout.effective_runtime_flags()`,
        and the browser-intent preflight narrowed the role policy through
        `pages_editor_capabilities_for_snapshot(role_capabilities, &rollout)` inside the
        rollout adapter. The publish/rebuild provenance packet says "Automatic repair or
        rebuild is not added." in the sentence case it is written in, and the PostgreSQL
        variant's revision fence compares `WorkingBody`, not `page_body::Model`. The
        production gate harness calls `input.handler.handle(input.envelope)`, and the local
        plan gained the section the gate reads. Both inline-edit harnesses pinned double quotes
        the app had already reformatted away — `apps/next-admin` formats with
        `singleQuote: true`, so the Playwright config, the browser spec and the privacy
        forbids now name the single-quoted forms and the wrapped calls are pinned at their
        opening lines; the browser dependency is checked as a declared range rather than one
        pinned version, and the rollout assembler pin carries the contract path without
        surrounding quotes. Sweep after this round: 157 pass, 0 fail, against 125 pass and 32
        fail at `f571186`.
      - Four gates could not evaluate anything and were counted as failures. They now report
        what they need and exit zero, and still fail on anything they can evaluate:
        `inspect-pages-anonymous-storefront-ssr-artifact` (maintainer inspector: needs
        `--artifact` and `--output`), `verify-fly-grapesjs-roundtrip` (drives Chromium and
        resolved `@playwright/test` out of an uninstalled `apps/next-admin`, throwing the
        loader's stack), and `verify-page-builder-consumer-readiness` in both locations (a
        parameterised gate run by module slug from CI and `package.json`, which exited with a
        usage error when called bare). A bare run is not a verdict; an unsupported slug and an
        unknown argument still are.
      - ~~`DECISIONS/README.md` did not list the 2026-11-11 Blog taxonomy ADR, so
        `verify-adrs.mjs` and `verify-docs.mjs` failed.~~ Done 2026-11-12: the registry row is
        present in the required descending order, and the ADR gained the sections the governance
        check requires. Its migration section now states what the tree holds — the two ports and
        the noop defaults exist, no Taxonomy-backed adapter does, and `rustok-blog` still depends
        on `rustok-taxonomy` for the translation-owner registration point — so the ADR no longer
        reads as if the Cargo decoupling were complete.
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
        admin. Since 2026-11-12 the page switch has an SSR path, the "Open page" form of the
        SSR inspector (see the `create_page` entry below); the tabs themselves still render
        and do nothing without hydration.
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
        - DONE (2026-11-12): the envelope arm of `create_page` activated the new page and
          the form path did not. Decision: a created page becomes the open page in every
          shape. The typed envelope and the local mode of `fly-leptos` (`on_add_page`) already
          did so, and an author creates a page in order to fill it. `open_created_page` in
          `browser_intent.rs` is the one activation both `create_page` arms call.

          Following the form to the browser found the larger defect behind it. In the default
          runtime nothing changed the page at all: the page tabs act through Leptos handlers
          only, so a page created through the form could not be opened, and with the activation
          alone the author could not have gone back. Fixed in the same change:
          - The SSR inspector renders an "Open page" form (`activate_page`, `page_id` alone).
            `is_ssr_page_form` hands it to `ssr_activate_page_intent`, which resolves the id
            against the document.
          - The Pages endpoint answers `reload: true` when the open page changed. It compares
            the page before and after the dispatch, so `activate_page`, which changes no
            document and is not a mutating intent, is covered without a list of intent names.
          - The page selects of the rename and remove forms open on the page the editor is on.
            They used to open on the first page of the list, whichever page was shown.

          Tests: `ssr_page_forms_browser_tests.rs` (both shapes land on the created page, a
          refused create opens nothing, undo, the open form and its way back, remove and rename
          of the open page), `ssr_control_contract_tests.rs` (the form is rendered, is absent
          for a read-only caller, and the selects name the open page),
          `rustok-pages/admin/src/browser_intent.rs` (`open_page` before and after an intent),
          `fly-browser-host-bootstrap.spec.ts` (the form posts `page_id` without `page_index`;
          an answer with `reload: true` reloads the address that carries the draft token).
          Open:
          - A caller without the edit capability cannot change the page in the default
            runtime. The open page is kept in the draft session, and the Pages endpoint keeps
            no session for such a caller, so the form is not rendered for them. The page is
            editing state and not route view state, because commands are resolved against it
            (`DECISIONS/2026-11-11-fly-ssr-control-contract-and-route-view-state.md`, item 9).
            Serving a reader needs a route key that the renderer applies and that every
            envelope repeats to the endpoint, the way the selection hint is repeated. Owner:
            Page Builder admin, `fly-browser`.
          - `undo` of a created page removes the page and leaves the editor on the last page
            that is left (`synchronize_page_navigation` clamps the index), not on the page the
            author came from. The typed envelope and the local mode behave the same way. Owner:
            Page Builder admin.
          - The `reload` term for a page switch is computed inside
            `dispatch_pages_browser_intent_with_store`, which fetches the page over the
            transport and has no test. The tests pin `open_page` and the dispatch it compares.
            Owner: Pages admin host.
          - `PageSelect` offers a page without an id as `page-<index>`. The page forms address
            a page by id, so the server refuses that option. Pages created by the editor always
            carry an id; an imported document may not. Owner: Page Builder admin.
          - The Rust of this change was not compiled or run locally. Owner: CI.
        - CLOSED (2026-11-12), no defect: "renaming the id of the page the editor is on leaves
          the route on the old id". The address names the Pages record, and the id of a
          document page is no part of it. The open page is kept in the draft session by id and
          index, and `synchronize_page_navigation` keeps the index when the id changes; after
          the reload the host renders the session (`load_route_draft`). Tests, one per shape:
          `rename_page_envelope_keeps_the_editor_on_the_page_whose_id_it_changes`,
          `rename_page_form_keeps_the_editor_on_the_page_whose_id_it_changes`.
        - DONE (2026-11-12): the bind form of `ssr_translations.rs` rendered its selection
          input with `value=""`, while the other panels render the selection the server knows.
          The adapter starts every document without a selection, so after the reload that
          follows an intent (creating the translation is one) the form posted no target and was
          refused with "translation binding requires component_id, bind_kind, and bind_name
          together" until the author selected the component again. The form now renders the
          selection the server holds. Without a selection it still names no component: a
          binding has no default target. Two render tests in `ssr_control_contract_tests.rs`;
          the Rust was not compiled here, CI decides. Owner: Page Builder admin.
        - DONE (2026-11-12): the SSR panels are rendered outside `CapabilityFieldset`, and only
          `ssr_assets.rs` read the capabilities, from a list of its own. A caller without the
          capability saw enabled forms and the endpoint refused each intent with
          `FLY_CAPABILITY_DENIED`. Each of the 24 forms now asks
          `AdminEditorRuntime::form_enabled`, which answers from `intent_requirements` in
          `capability_access.rs`, the function the endpoint validates with; a form whose intent
          is refused renders inside a disabled `<fieldset>`. The component property form is
          usable while one of its kinds is, and offers only those: an inline style needs
          `styles`, a field or an attribute `properties`. A render test compares every form of
          the composed admin with `validate_browser_capability_access` for six capability
          profiles. The Rust was not compiled here, CI decides. No twin: `fly-leptos`,
          `fly-dioxus` and the Next host render no intent form. Owner: Page Builder admin.
        - A capability that changes while a document is open (a provider status that degrades)
          reaches the forms with the next server render only. Owner: Page Builder admin.
        - DONE (2026-11-12): the envelope arms of the page intents had no dispatch test of
          their own beyond `rename_page` and `remove_page` by index.
          `ssr_page_forms_browser_tests.rs` now dispatches the envelopes that the typed
          constructors of `fly-browser` build (`create_page`, `remove_page`, `rename_page`,
          `page_properties`, `patch_page_metadata`, `move_page`): the id derived from the name
          and its next free variant, the default name, the id winning over a stale index, the
          editor staying on a page whose id changes, identity and metadata in one command, the
          normalized slug, the refusal without `metadata`, and the page that is active after a
          move in each direction. Tests only; not compiled here, CI decides.
      - Hydrated `wasm-client` builds mount the adapter too (`wasm-client` enables
        `browser-js`, `post_intents` defaults to `true`). A toolbar button then has both
        `data-fly-action="intent:…"` and a live `on:click`. Whether both fire, and which one
        must own the control, is not established: the profile cannot be built here. The
        viewport select is in the same position since it became route state: its `on:change`
        sets the viewport locally and the adapter navigates to `?fly_viewport=`, which reloads
        a page that hydration had already updated; editor state that only the hydrated client
        holds does not survive that reload. Before the select became route state, a hydrated
        build changed the viewport without a reload. Owner: Page Builder admin.
      - DONE: the viewport select could not return to the initial 1280 × 720 frame. Its option
        was rendered only while the frame was in effect, and the adapter ignored the empty
        value. The toolbar now always offers the frame the editor holds without a route preset
        under the empty value, and the adapter removes `fly_viewport` for it. The initial
        frame did not become a preset: a preset id for it would have to be resolved against
        whatever frame the editor holds, which is not a constant of `fly-ui`. The browser half
        ran in Chromium; the render tests are executed by CI only.
      - DONE: `fly-browser.js` changed its selection without `fly:select` on a `dragstart`
        of a layer row and in the delegated `select-component` case, so the SSR forms kept
        the earlier target. Every selection now goes through `select()`, which announces a
        change once. The canvas `teardown` message stays silent on purpose: it is not a
        selection, and the server-rendered forms keep the component that the server holds.
        Part 8 of `verify-fly-ssr-control-contract.mjs`; three browser tests in
        `fly-browser-ssr-controls.spec.ts`.
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
    - DONE (2026-11-12), message only: `PagesFlyBuilder`
      (`rustok-pages/admin/src/composition.rs`) restored the draft session with
      `.load(…).ok().flatten()`: a store error on page open (I/O, lock timeout, a record that
      does not parse, an owner mismatch) opened the editor on the saved document, without the
      unsaved edits and without a message. `load_route_draft` returns the error, and the
      editor renders it in an alert above the workspace (`pages.builder.draftLoadError`, en
      and ru) while it still opens the saved page. Two unit tests on an in-memory store; not
      compiled here, CI decides. Open:
      - The error is not logged: the crate has no `tracing` dependency, and adding one changes
        `Cargo.lock`, which cannot be regenerated in this workspace.
        Owner: `rustok-pages-admin`.
      - A session that is gone, or that was started from another saved revision, still opens
        the saved page without a message. Owner: `rustok-pages-admin`.
      - The address keeps the draft token, so each editor action is refused with the same
        error until the author opens the page without it. Owner: `rustok-pages-admin`.
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

11. **Host delivery of the editor — OPEN.** Decision 8 of
    `DECISIONS/2026-11-11-fly-ssr-control-contract-and-route-view-state.md`.
    - [x] `apps/admin` and `rustok-pages-admin` enabled `rustok-page-builder-admin/ssr` without
          `browser-js`; the admin binary rendered no adapter. Both now enable the pair.
    - [x] The adapter script and the canvas `srcdoc` document carried no CSP nonce. The host
          provides `UiCspNonce`; the editor writes it on its inline elements and on the
          documents it embeds.
    - [x] `ServerPreviewPanel` put the preview document into `srcdoc` as it arrived; the
          stylesheet of the document had no nonce and was dropped under the host policy. The
          preview arrives as one composed string from the server pipeline, which does not know
          the nonce of the page that asked for it. `fly::document_html_with_style_nonce` puts
          the nonce on the stylesheet of the envelope `compose_document_html` writes and on
          nothing else; the panel calls it with the nonce the host provided.
    - [x] The browser build of `apps/admin` (features `csr` and `hydrate`, the build
          `apps/server` embeds) provided no `UiCspNonce`. `apps/server` renders nothing: it
          serves `index.html` and writes its nonce on the `<script` and `<style` tags of that
          file, so the value never reached a render context and the canvas stylesheet, the
          canvas runtime and the preview stylesheet were written without it. The browser host
          now reads the nonce from the `nonce` property of a script of its own document
          (`document_csp_nonce` in `apps/admin/src/main.rs`) and provides the same context.
          A probe page under the policy of `apps/admin/src/app/security.rs` showed, in
          Chromium 153, that `script[nonce]` finds the script, that the property holds the
          value while the attribute reads empty, and that a `sandbox=""` `srcdoc` frame with
          the Fly envelope applies its stylesheet with the nonce and drops it without. The
          probe did not use the built admin.
    - [ ] **The standalone SSR admin does not render the editor.** Decided 2026-11-13
          (`DECISIONS/2026-11-13-standalone-admin-editor-delivery.md`), not started. The mechanism
          is now named from `leptos` 0.8.21 source rather than inferred, and two of the three
          obvious fixes are ruled out by it:
          - a `LocalResource` *never* resolves on the server — `ArcLocalResource::new` wraps the
            fetcher in `if cfg!(feature = "ssr") { pending().await }` — so `PagesAdmin`
            (`crates/modules/rustok-pages/admin/src/composition.rs`), which loads the page list and
            the open page that way, can only ever render its `Suspense` fallback in the response;
          - `Effect::new` dispatches to `new_isomorphic`, which spawns a task for the next tick and
            is not driven to completion before the response is produced, so `LocalResource` plus an
            `Effect` reproduces exactly the same empty render (this also applies to
            `EnabledModulesProvider` in `apps/admin/src/shared/context/enabled_modules.rs`);
          - `leptos_axum` calls `leptos::nonce::provide_nonce()` and generates its *own* nonce for
            `HydrationScripts`, while `admin_security_headers` enforces a different per-request
            nonce, so shipping the hydration bundle as-is would be blocked by this host's own
            policy and fail silently.
          The decision: the default SSR profile keeps no hydration bundle (it is a supported
          profile for hosts that build one, under a stated nonce contract), and the editor's server
          state is loaded through an awaitable `Resource` rather than `LocalResource` plus an
          `Effect`. Surfaces move one at a time, starting with the Pages workspace, each with a
          render test that asserts the workspace is in the SSR HTML and the fallback is not.
          `LocalResource` appears in 72 files across `apps/admin` and the module admin packages, so
          the rest stay where they are until a host is expected to show them.
          Owner: `rustok-pages-admin`, `apps/admin`.
    - [ ] Load the editor in a browser under the policy of `apps/admin/src/app/security.rs`.
          The two Playwright suites serve their pages without a policy, and `apps/admin` does
          not build in this workspace. A probe page with that policy showed, in Chromium, that
          an inline `<script>` or `<style>` without the nonce is dropped, that a sandboxed
          `srcdoc` iframe inherits the policy, and that both run with the nonce. The probe
          did not use the rendered editor. The workflow "Rust-hosted Browser Smoke" builds the
          embedded admin and runs Chromium against `apps/server` under the enforced policy,
          and its spec (`apps/next-admin/tests/rust-hosts/embedded-hosts.spec.ts`) stops at the
          admin mount. A test there that opens a page in the editor would cover the canvas and
          the preview under the policy. That job does not reach its spec today: it builds
          `apps/server` with default features, and the server exits at startup because
          `modules.toml` listed modules whose `mod-*` features were not in the default list of
          `apps/server/Cargo.toml` (read from CI on `c8141c9b` and `a4d65ffe`). Resolved
          2026-11-12: `scripts/ci/modules-manifest-features.mjs` derives the required features
          from the manifest — keyed on the package's own `[crate] entry_type`, which is what the
          registry contract reads — and both the smoke job and CI fail before a build whose
          default features disagree with it. See
          `DECISIONS/2026-11-12-module-composition-is-the-manifest.md`. Owner: Page Builder
          admin, repository tooling.
    - [x] **The server preview cannot be requested in the default runtime.** "Refresh preview"
          acted through `on:click`, no intent or SSR form requested a preview, and the draft
          session held none, so a server-rendered editor showed the empty state forever.
          Resolved 2026-11-12: `render_server_preview` is a protocol intent the endpoint's
          capability policy admits with the edit capability; `dispatch_named_intent` builds the
          one-page `Preview` request from the draft and the runtime context the form posts; the
          panel renders the form instead of relying on the handler; and the host bootstrap fills
          the panel's frame from the answer. The consumer no longer reports a preview answer as
          a reason to reload — a reload would have discarded the document it carries. Owner:
          Page Builder admin, `rustok-pages-admin`.
    - [x] **The preview is shown under the policy of the admin, not of the storefront.** Decided
          2026-11-12 (`DECISIONS/2026-11-12-page-builder-server-preview-delivery.md`);
          landed 2026-11-12:
          the faithful shape is the preview delivered as its own response with the artifact
          policy, and `srcdoc` is the interim shape a host without that route uses, labelled as
          such. The admin policy is not widened for it; what remains is the route and the panel's
          `src`/`srcdoc` switch. A
          `srcdoc` document inherits the policy of its embedder. Under
          `style-src-attr 'none'` a `style` attribute is dropped (the probe above recorded the
          refusal); the embed block writes `style="aspect-ratio:…"`. By the directives of the
          policy, and not probed: the provider `<iframe>` of an embed block is refused
          (`default-src 'self'` covers frames), and so is a font or a media file from another
          origin. The canvas document has the same limits. A preview that is faithful to the
          published page is a document with its own response and its own policy, as
          `/richtext/frame` is.
          Delivered: `GET /api/admin/pages/{page_id}/preview` in `apps/admin` reads the snapshot
          of the request's single verified exchange, loads the draft session the editor names
          through the owner-checked store (`?draft=…`; without it the saved page), renders it with
          `rustok_pages_admin::render_server_preview`, and answers under the artifact policy
          derived from the stylesheet the document carries
          (`pages_server_preview_response`, `default-src 'none'` + `style-src 'sha256-…'`). A
          document without the Fly envelope is refused rather than policied from a guess.
          `ServerPreviewPanel` loads that address by `src` when the host configures one and keeps
          `srcdoc` otherwise, and the host bootstrap no longer writes `srcdoc` over a frame that
          has a `src` — `srcdoc` wins over `src`, so that write would have replaced the page's
          own document with an inherited one. The admin policy itself is unchanged; the middleware
          classifies only this path (`SAMEORIGIN`, `no-store`) and leaves a policy the handler
          derived intact, since the fallback it would otherwise apply names no style source.
          Owner: Page Builder admin, `rustok-pages-admin`, `apps/admin`.
    - [x] `fly-leptos` `FlyCanvasPreview` renders an inline `<style>` and a `style` attribute,
          and `fly-dioxus` renders `style` attributes. The admin editor uses neither; a host
          that composes them under a nonce policy loses those styles. Owner: `fly-leptos`,
          `fly-dioxus`. Resolved 2026-11-12: `FlyCanvasPreview` and `FlyEditorWorkspace` in both
          adapters accept `csp_nonce` and write it on the inline `<style>`; `AdapterWorkspacePanel`
          (`rustok-page-builder-admin`) and the Dioxus adapter host pass the `UiCspNonce` of the
          render context. The `style` *attribute* of the canvas is still `style-src-attr 'none'`
          under the admin policy and stays dropped; it only sets the preview width, which is not
          worth weakening the attribute policy for.
    - [x] The step "Check admin SSR Page Builder endpoint" of the "Fly Page Builder" workflow
          failed on this branch after the merges of `main` (`7ce2048c`, `728ba4f8`). It passes
          on `65a67b7c` (job "Focused tests and source guards"), together with "Run Page
          Builder admin unit tests" and "Lint Fly browser and Page Builder integrations", so
          the change to `apps/admin/src/main.rs` is compiled by a green job.

## Rules for this backlog

- New Fly/Page Builder work adds its remaining items here, not in a new plan
  document; dated actualization notes may record *what changed*, never a second
  backlog.
- An item leaves this list only by landing (with its ADR) or by an explicit
  decision recorded in `DECISIONS/`; "Pending" without an owner is not a state.
