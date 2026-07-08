---
id: doc://docs/research/dioxus-ffa-pilot-connectivity-map.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# FFA/Dioxus Pilot Connectivity Map (Phase A baseline)

This document captures the execution of steps **A1 (pilot selection)** and **A2 (connectivity map)**
from `docs/research/dioxus-ffa-ui-migration-plan.md`.

## A1. Selected Pilots

### Pilot 1 (medium complexity): `rustok-pages`

Selection reasons:
- limited storefront read/edit surface;
- fewer cross-module dependencies compared to commerce/search;
- good candidate for first `core -> transport -> ui/leptos` extraction.

### Pilot 2 (high complexity): `rustok-search`

Selection reasons:
- pronounced search-state logic (query/filter/pagination/sort);
- sensitivity to route/query parity and locale/tenant context;
- presence of selected-path branches and runtime branching for SSR/GraphQL path.

## A2. Connectivity Map

## `rustok-pages`

### Leptos-specific points
- `#[component]` storefront/admin surfaces;
- router/query hooks and navigation binding layers;
- reactive state and signal derivatives for page selection/render.

### Transport binding points
- native `#[server]` handlers for SSR/hydrate path;
- GraphQL selected path through module-owned transport adapters;
- `cfg(feature = "ssr")` branching for runtime split.

### Layer mixing risks
- direct transport calls from UI components;
- mixing view-state and domain mapping in Leptos hooks;
- error mapping duplication between native and GraphQL path.

## `rustok-search`

### Leptos-specific points
- `#[component]` search surfaces and filters UI;
- routing/query state binding (including URL-driven selection state);
- reactive derived state for paging/sorting/empty/error views.

### Transport binding points
- native `#[server]` read/search path in SSR/hydrate;
- GraphQL selected-path adapters for headless/CSR-compatible flow;
- runtime conditional branches for selected-path/degradation mode.

### Layer mixing risks
- binding transport payload format directly to UI model;
- implicit policy/validation checks in UI layer;
- query normalization divergence between native and GraphQL path.

## Phase A deliverables status

- [x] A1 pilot selection captured.
- [x] A2 connectivity map captured.
- [x] A3 contract freeze evidence fully attached: parity checklist captured, verify command `npm run verify:ffa:ui:migration` added to mandatory evidence path.

## Next step (one-task-per-iteration)

Next iteration: for `rustok-blog` extract **one** target use-case and perform a
structural slice of `core/transport/ui` without changing the product dual-path contract,
with mandatory evidence per checklist:
`docs/verification/ffa-ui-parity-checklist.md`.

## Related documents

- `docs/research/dioxus-ffa-ui-migration-plan.md`
- `docs/verification/ffa-ui-parity-checklist.md`
- `docs/UI/graphql-architecture.md`

## Execution status by module (Phase B tracking)

- [x] `rustok-pages` — first decomposition slice completed: `core` layer extracted in `storefront`
  for selected-page presentation logic; Leptos UI delegates this logic to `core`.
- [x] `rustok-search` — slices #1-#9 completed per storefront/admin core extraction plan.
- [x] `rustok-blog` — storefront slice #1 completed: formatting/fallback helper logic extracted to `storefront/src/core.rs`, UI uses `core::*` without changing dual-path transport contract.

### What has already been done in `rustok-pages`

- added `crates/rustok-pages/storefront/src/core.rs`;
- `SelectedPageCard` migrated to `core::*` functions;
- Leptos storefront render/bind layer moved to `crates/rustok-pages/storefront/src/ui/leptos.rs`, crate root only wires modules/re-exports `PagesView`;
- dual-path transport contract (`native #[server]` + GraphQL selected path) not changed.

### Double-check after completion

- [x] Pass #1 (code/docs consistency):
  - `rustok-pages/storefront` actually uses the extracted `core` layer for selected-page logic and explicit `ui/leptos.rs` adapter for render/bind;
  - dual-path transport (`native #[server]` + GraphQL selected path) preserved without removing the GraphQL surface.
- [x] Pass #2 (cleanup stale wording):
  - current central docs for this step contain no wording contradicting the `core` slice in `rustok-pages`.

### Next module (new iteration)

- [x] Started and completed the current `rustok-search` pilot slices (#1-#9).
- [x] Iteration goal achieved: core use-cases sequentially extracted into `crates/rustok-search/storefront` and synchronized with `admin` surface without changing the product transport contract.

### Scope matrix for `rustok-search` (to avoid omissions)

- [x] `crates/rustok-search/storefront` (Leptos storefront UI package)
  - [x] first `core` use-case extracted (query/filter input normalization: `parse_csv`, `optional_text`);
  - [x] selected use-case extracted to `storefront/src/core.rs` and used by UI layer.
- [x] `crates/rustok-search/admin` (Leptos admin UI package)
  - [x] impact of the same use-case verified;
  - [x] same `core` approach applied in `admin/src/core.rs` without contract divergence.
- [x] Headless parity (Next/mobile/external)
  - [x] confirmed that GraphQL selected path has not degraded;
  - [x] route/query/i18n contract has no drift relative to host expectations.

### Evidence check before closing `rustok-search` iteration

- [x] `cargo xtask module validate search`
- [x] `cargo xtask module test search`
- [x] docs double-check pass #1 (code/docs consistency)
- [x] docs double-check pass #2 (cleanup stale wording)

### Completed in current iteration (`rustok-search`, slice #1)

- added `crates/rustok-search/storefront/src/core.rs` and `crates/rustok-search/admin/src/core.rs`;
- removed local duplicates of `parse_csv`/`optional_text` in storefront/admin UI and connected `core::*`;
- dual-path transport (`native #[server]` + GraphQL selected path) not modified.

- `rustok-search` slice #2: facet name normalization extracted to core for storefront/admin (`facet_display_name`).
- `rustok-search` slice #3: facet bucket label formatting extracted to core (`facet_bucket_label`) for storefront/admin.
- `rustok-search` slice #4: snippet fallback rendering extracted to core (`snippet_or_fallback`) for storefront/admin.
- `rustok-search` slice #5: score label normalization extracted to core (`score_label`) for storefront/admin.
- `rustok-search` slice #6: entity/source/status labels extracted to core for storefront/admin.
- `rustok-search` slice #7: score template value extraction migrated to core helper (`score_value`) without string hacks in UI.
- `rustok-search` slice #8: error message composition (`<context>: <error>`) extracted to core for storefront/admin.
- `rustok-search` slice #9: score rendering unified across storefront/admin to direct core helpers, removing template/trim coupling in UI.

- `rustok-pages` slice #2: admin form helpers (`slugify`, `parse_channel_slugs`, error composition) extracted to `admin/src/core.rs`.

### Pages completion checklist (Phase B pilot)

- [x] `rustok-pages/storefront` core slice #1 (`selected_page_*`, `summarize_page_content`)
- [x] `rustok-pages/admin` core slice #2 (`slugify`, `parse_channel_slugs`, `error_with_context`)
- [x] `cargo xtask module validate pages`
- [x] `cargo xtask module test pages` (long run completed, evidence attached)
- [x] docs double-check pass #1/#2 for pages
- `rustok-pages` slice #3: status badge class mapping extracted to `admin/src/core.rs` (`status_badge_class`).
- `rustok-pages` slice #4: admin busy-key composition extracted to core (`busy_key_with_id`, `busy_key_for_save`).
- `rustok-pages` slice #6: admin page-list load error rendering migrated to core `error_with_context`.
- `rustok-pages` slice #7: status badge css composition moved to core (`status_badge_css`).
- `rustok-pages` slice #8: busy-key action matching moved to core (`busy_key_matches_action`).
- `rustok-pages` slice #9: raw body summary placeholder rendering moved to storefront core (`raw_body_format_summary`).
- `rustok-pages` slice #10: pages implementation tracker synchronized after double docs verification closure.
- `rustok-pages` slice #11: admin reset-form defaults delegated to core seed helper (`empty_edit_form_seed`).
- `rustok-pages` slice #12: admin table total-count label placeholder rendering moved to core (`count_label`).
- `rustok-pages` slice #13: storefront published-pages total count placeholder rendering moved to core (`count_label`).
- `rustok-pages` slice #14: admin editing-banner `{id}` placeholder rendering moved to core (`label_with_id`).
- `rustok-pages` slice #15: storefront open-link label composition moved to core (`open_link_label`).
- `rustok-pages` slice #16: storefront label/value pair rendering moved to core (`label_value_pair`).
- `rustok-pages` slice #17: storefront cleanup after full pages module-test evidence (remove unused import warning).
- `rustok-pages` slice #19 evidence: admin capability-card presentation helpers (`publish_state_view`, `channel_count_label`, `legacy_block_snapshot_label`) moved into `admin/src/core.rs`; Leptos adapter keeps only signal wiring and callback execution.
- `rustok-pages` slice #20 evidence: storefront published-page link/status presentation (`page_link_href`, `page_status_label`) moved into `storefront/src/core.rs`; dual-path transport contract unchanged.
- `rustok-pages` slice #24 evidence: admin page table row action busy/label mapping (`admin_page_row_action_state`, `admin_page_row_action_labels`) moved into `admin/src/core.rs`; Leptos adapter keeps only render/callback wiring and dual-path transport contract unchanged.

### Double-check after slices #2-#8 (rustok-pages/admin)

- [x] Pass #1 (code/docs consistency):
  - form helper logic, status badge and busy-key in `crates/rustok-pages/admin` extracted to `admin/src/core.rs`;
  - storefront and admin surfaces use `core::*` without changing transport contract.
- [x] Pass #2 (cleanup stale wording):
  - central docs updated/removed formulations where these helper responsibilities were described as inline logic in `lib.rs`;
  - pages tracker synchronized with actual slice #2-#8 state.

### Which modules were modified in this iteration

- `rustok-pages/admin` — core helper extraction and UI call-site alignment.
- `rustok-pages/storefront` — previously completed core slice confirmed by re-verification.
- `rustok-blog/storefront` — new slice started: formatting/fallback helper logic extracted to `storefront/src/core.rs`.

### Pages pilot status (current checkpoint)

- [x] Planned `rustok-pages` pilot slices completed for current helper-extraction scope.
- [x] Validate + module test evidence attached in trackers.
- [x] Documentation double-check completed and synchronized.
- [x] Pilot can be treated as baseline reference sample for following module slices.

### Additional iteration evidence

- blog slice #1 evidence: `crates/rustok-blog/storefront/src/core.rs` used by `crates/rustok-blog/storefront/src/lib.rs`; transport split (`native #[server]` + GraphQL selected path) not changed.
- `rustok-search` slice #10 evidence: admin relevance editor JSON formatting/profile/preset extraction moved from Leptos render module to `admin/src/core.rs`; transport split (`native #[server]` + GraphQL selected path) not changed.
- `rustok-search` slice #11 evidence: admin analytics/diagnostics metric formatting moved from Leptos render module to `admin/src/core.rs`; transport split (`native #[server]` + GraphQL selected path) not changed.
- `rustok-search` slice #12 evidence: admin preview summary/preset rendering and diagnostics fallback text moved from Leptos render module to `admin/src/core.rs`; transport split (`native #[server]` + GraphQL selected path) not changed.
- `rustok-search` slice #13 evidence: admin analytics/dictionaries error messages and timestamp fallbacks now use existing `admin/src/core.rs` helpers; transport split (`native #[server]` + GraphQL selected path) not changed.
- `rustok-search` slice #14 evidence: admin tab and diagnostics/consistency badge CSS class mapping moved from Leptos render module to `admin/src/core.rs`; transport split (`native #[server]` + GraphQL selected path) not changed.
- `rustok-search` slice #15 evidence: admin navigation href, engine option label and rebuild feedback rendering moved from Leptos render module to `admin/src/core.rs`; transport split (`native #[server]` + GraphQL selected path) not changed.
- `rustok-search` slice #16 evidence: admin relevance editor merge and JSON-array validation moved from Leptos render module to `admin/src/core.rs`; transport split (`native #[server]` + GraphQL selected path) not changed.
- `rustok-search` slice #31 evidence: storefront transport split into `transport/native_server_adapter.rs` and `transport/graphql_adapter.rs`; build-profile-selected native/GraphQL orchestration moved to `transport/mod.rs`, while raw `api.rs` keeps the existing native server-function and GraphQL endpoints.
- `rustok-search` slice #32 evidence: storefront suggestions presentation and document-vs-query navigation decision moved into `storefront/src/core.rs`; Leptos adapter renders core-owned suggestion view-models and only executes the prepared navigation target.
- `rustok-search` slice #33 evidence: storefront filter preset chip state/class/next-selection mapping moved into `storefront/src/core.rs`; Leptos adapter renders core-owned chip view-models and keeps only signal wiring/navigation execution.
- `rustok-search` slice #34 evidence: storefront facet display names and bucket labels moved into `storefront/src/core.rs`; Leptos facet cards render core-owned facet view-models without inline facet formatting.
- `rustok-search` slice #35 evidence: storefront result action no-target/open-link state and click-tracking position moved into `storefront/src/core.rs`; Leptos result cards render the prepared action model and only execute click tracking/navigation.
- `rustok-search` slice #36 evidence: storefront empty-state and feature-card title/body presentation moved into `storefront/src/core.rs`; Leptos cards render core-owned view-models without local presentation ownership.
- `rustok-search` slice #37 evidence: storefront results header query label/query/summary/preset/locale presentation moved into `storefront/src/core.rs`; Leptos header renders the core-owned header view-model.
