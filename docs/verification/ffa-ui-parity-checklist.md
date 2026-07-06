---
id: doc://docs/verification/ffa-ui-parity-checklist.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# FFA UI Migration: parity checklist (Phase A baseline)

This document captures the mandatory baseline checklist for migration tasks under
the `docs/research/dioxus-ffa-ui-migration-plan.md` plan.

## Purpose

The checklist is used as evidence for phase-gates `A -> B`, `B -> C`, `D -> E`
and to ensure that the dual-path contract (`native #[server]` + GraphQL fallback)
does not degrade during FFA decomposition.

## Scope

- module-owned UI packages `crates/rustok-*/admin` and `crates/rustok-*/storefront`;
- host wiring in `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`;
- verify scripts in `scripts/verify/*` when contract rules change.

## Mandatory Checks for Each Migration Task

### 1) Contract parity

- [ ] Native path (Leptos SSR/hydrate) works for the target scenario.
- [ ] GraphQL fallback works for the same scenario.
- [ ] Headless host path (Next/mobile/external) is not broken.
- [ ] GraphQL/REST surface is not removed or weakened.

### 2) FFA layering

The target structural shape is set to one of:

- `none` — code FFA split not yet started;
- `docs_boundary` — boundary/docs track synchronized but UI split not yet started;
- `core_only` — framework-agnostic `core.rs` or `core/` already owns the view-model/request/policy fragment;
- `core_transport` — module-owned `transport/` facade/adapters added;
- `core_transport_ui` — has `core`, `transport` and explicit `ui/leptos.rs` or `ui/leptos/` adapter.

`core.rs` is allowed for a small slice; when multiple subdomains appear (`view_model`, `policy`, `error`, `ports`, `identifiers`), the module should transition to `core/`. Similarly `ui/leptos.rs` is allowed for a single render adapter file, and `ui/leptos/` is used when the adapter layer grows.

- [ ] UI layer does not own transport/business logic.
- [ ] UI adapter accesses transport only through module-owned facade; request/command/state construction and business/policy remain in core ports/helpers.
- [ ] Core layer does not depend on `leptos*`.
- [ ] Transport adapters are separated by role: native and GraphQL fallback, or a temporary single-adapter state with a next-step parity plan is explicitly documented.
- [ ] Host-visible UI status/error contracts have stable machine-readable codes and documented locale keys.

### 3) i18n/tenant/request context

- [ ] Uses host-provided effective locale, without package-local fallback chains.
- [ ] `RequestMeta`/tenant scope is not lost between native and GraphQL paths.
- [ ] Route/query contract does not diverge between Leptos and headless hosts.

### 4) Tests & verification evidence

- [ ] `cargo xtask module validate <slug>` executed.
- [ ] `cargo xtask module test <slug>` executed.
- [ ] When host/UI wiring changed:
  - [ ] `npm run verify:i18n:ui`
  - [ ] `npm run verify:i18n:contract`
  - [ ] `npm.cmd run verify:storefront:routes`
- [ ] `npm run verify:ffa:ui:migration` executed.
- [ ] For changed error/status contracts, a list of stable codes and locale keys is attached.
- [ ] Actual verification output is included in the PR.

### 5) Documentation double-check

- [ ] Local docs of affected modules updated.
- [ ] Central docs in `docs/` updated.
- [ ] `docs/index.md` updated if a doc node was added/changed.
- [ ] Pass 1 completed: code and wording match.
- [ ] Pass 2 completed: outdated transport wording removed/fixed.

## Evidence template (insert into PR)

```md
### FFA parity evidence
- Module: <slug>
- Task slice: <one-task-per-iteration description>
- Native path: PASS/FAIL
- GraphQL fallback: PASS/FAIL
- Headless path: PASS/FAIL
- Structural shape: none/docs_boundary/core_only/core_transport/core_transport_ui
- Contract guard (GraphQL/REST retained): PASS/FAIL
- Docs double-check: PASS/FAIL
- Error/status contract (if changed): `<code>` -> `<locale key>`

Commands:
- cargo xtask module validate <slug>
- cargo xtask module test <slug>
- npm run verify:i18n:ui
- npm run verify:i18n:contract
- npm.cmd run verify:storefront:routes
- npm run verify:ffa:ui:migration
```

## Current Evidence Notes

- 2026-06-26, `blog`, slice #104: admin shell/list/form scaffold CSS class selection moved into Leptos-free `BlogPostAdminShellClassesViewModel` / `blog_post_admin_shell_classes_view`; page shell, module header, posts card, locale filter, loading/error states, sidebar and form-card classes are prepared by core while the Leptos adapter keeps markup, signals and callbacks only. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `cargo fmt --package rustok-blog-admin`; Cargo compilation was intentionally skipped by request; native/GraphQL transport surfaces were not changed.
- 2026-06-24, `blog`, slice #103: admin posts-table CSS class selection moved into Leptos-free `BlogPostAdminTableClassesViewModel` / `blog_post_admin_table_classes_view`; table shell, header cells, row/cell typography and primary/destructive action button classes are prepared by core while the Leptos adapter keeps markup and callbacks only. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `cargo fmt --package rustok-blog-admin`; Cargo compilation was intentionally skipped by request; native/GraphQL transport surfaces were not changed.
- 2026-06-20, `blog`, slice #101: admin edit-banner and raw-body warning CSS class selection moved into Leptos-free `edit_banner_class` / `raw_body_warning_class` and the existing `BlogPostAdminEditBannerViewModel` / `BlogPostAdminRawBodyWarningViewModel`; Leptos now renders prepared class payloads and keeps only host signal/event plumbing. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `cargo fmt --package rustok-blog-admin`; Cargo compilation was intentionally skipped by request; native/GraphQL transport surfaces were not changed.
- 2026-06-20, `blog`, slice #100: admin body-format change normalization moved into Leptos-free `BlogPostAdminBodyFormatChangeViewModel` / `blog_post_admin_body_format_change_view`; unsupported select event values now fall back to canonical `markdown` in core before Leptos signal mutation, while the existing core-owned select options remain the source of supported formats. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `cargo fmt --package rustok-blog-admin`; Cargo compilation was intentionally skipped by request; native/GraphQL transport surfaces were not changed.
- 2026-06-20, `blog`, slice #99: admin body-format select option policy moved into Leptos-free `BlogPostAdminBodyFormatSelectViewModel` / `BlogPostAdminBodyFormatOptionViewModel` / `blog_post_admin_body_format_select_view`; the Leptos adapter now renders prepared `markdown` / `rt_json_v1` options and selected state without owning body-format option policy. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `cargo fmt --package rustok-blog-admin`; Cargo compilation was intentionally skipped by request; native/GraphQL transport surfaces were not changed.
- 2026-06-20, `blog`, slice #98: admin title-input autoslug decision moved into Leptos-free `BlogPostAdminTitleInputViewModel` / `blog_post_admin_title_input_view`; the Leptos adapter now passes raw title input and current slug to core, then applies only the prepared title and optional slug update. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `cargo fmt --package rustok-blog-admin`; Cargo compilation was intentionally skipped by request; native/GraphQL transport surfaces were not changed.
- 2026-06-20, `blog`, slice #97: admin editor form copy moved into Leptos-free `BlogPostAdminEditorFormCopyViewModel` / `BlogPostAdminEditorFormCopyLabels` / `blog_post_admin_editor_form_copy_view`; the Leptos adapter now resolves localized field labels/placeholders once and renders a prepared copy payload without inline form-label policy in the editor tree. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `cargo fmt --package rustok-blog-admin`; Cargo compilation was intentionally skipped by request; native/GraphQL transport surfaces were not changed.
- 2026-06-19, `blog`, slice #96: admin posts-table header/empty-state/row normalization moved into Leptos-free `BlogPostAdminPostsTableViewModel` / `BlogPostAdminPostsTableLabels` / `blog_post_admin_posts_table_view_from_items`; the Leptos adapter now builds localized table/row labels once and renders prepared rows without constructing row view-models inside the render loop. Evidence: `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `node scripts/verify/verify-blog-admin-boundary.mjs`; Cargo compilation was intentionally skipped by request; native/GraphQL transport surfaces were not changed.
- 2026-06-18, `blog`, slice #95: admin status-badge presentation moved into Leptos-free `BlogPostAdminStatusBadgeViewModel` / `blog_post_admin_status_badge_view`, while the Leptos adapter now memoizes form and issue-banner view-model reads instead of rebuilding those payloads in multiple render closures; fast guardrail markers and fixtures were extended for status-badge ownership. Evidence: `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `node scripts/verify/verify-blog-admin-boundary.mjs`; Cargo compilation was intentionally skipped by request; native/GraphQL transport surfaces were not changed.
- 2026-06-13, `region`, slices #33-#34: admin open-detail success/error outcome mapping moved into Leptos-free `RegionAdminOpenDetailViewModel`, and save-success selected/form/refresh/route-update outcome mapping moved into Leptos-free `RegionAdminSaveSuccessViewModel`; `node scripts/verify/verify-region-admin-boundary.mjs` passed; native/GraphQL transport surfaces were not changed.
- 2026-06-13, `blog`, slices #78-#80: admin editor form-state mapping/reset defaults moved into Leptos-free `BlogPostEditorFormState`, admin table-row display/action state moved into Leptos-free `BlogPostAdminTableRowViewModel`, and archive/delete row action presentation completed inside the same core view-model; `node scripts/verify/verify-blog-admin-boundary.mjs` passed for both slices; long `cargo test -p rustok-blog-admin --lib` was stopped during slice #78 after dependency compilation started to avoid long compile; targeted `timeout 20s cargo test -p rustok-blog-admin --lib table_row_view_model_composes_row_policy_without_ui_runtime` reached the timeout during dependency compilation, so no long compile was allowed; native/GraphQL transport surfaces were not changed.
- 2026-06-14, `blog`, slices #86-#88: admin save-result policy moved into Leptos-free `BlogPostSaveResultViewModel`, selected-post route/query push/replace/clear intent selection moved into Leptos-free `BlogPostAdminRouteQueryIntent`, and edit-banner visibility/copy/action-label presentation moved into Leptos-free `BlogPostAdminEditBannerViewModel`; create/update/delete/open flows receive core-owned query intents and the Leptos adapter now renders prepared edit-banner payloads through signal bindings; `node scripts/verify/verify-blog-admin-boundary.mjs` passed after extending the guardrail for save-result, route/query intent and edit-banner helpers; Cargo compilation was intentionally avoided by request except for short non-compiling checks; native/GraphQL transport surfaces were not changed.
- 2026-06-16, `blog`, slice #94: admin posts-list `BlogPostList` result normalization moved into Leptos-free `blog_post_admin_posts_load_view_from_list`, so the Leptos adapter no longer unpacks list DTOs before invoking the core-owned loaded/empty-contract-unavailable/error envelope; transport-facade contract-unavailable classification, native/GraphQL surfaces and render branching remain unchanged. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `cargo test -p rustok-blog-admin --lib admin_warning_and_posts_load_views_keep_adapter_policy_in_core`.
- 2026-06-15, `blog`, slices #89-#90: admin raw-body warning visibility/message moved into Leptos-free `BlogPostAdminRawBodyWarningViewModel`, and posts-list load-result/contract-unavailable/error presentation moved into Leptos-free `BlogPostAdminPostsLoadViewModel`; the Leptos adapter keeps only transport-facade classification plus render branching, while `node scripts/verify/verify-blog-admin-boundary.mjs` passed after extending the guardrail for both helpers; Cargo compilation was intentionally avoided by request; native/GraphQL transport surfaces were not changed.
- 2026-06-13, `blog`, slices #81-#82: admin write-path issue banner presentation moved into Leptos-free `BlogPostAdminIssueBannerViewModel`, then publish/unpublish, archive and delete action command preparation moved into Leptos-free `BlogPostStatusCommand`, `BlogPostArchiveCommand` and `BlogPostDeleteCommand`; `node scripts/verify/verify-blog-admin-boundary.mjs` passed after extending the guardrail for the new command helpers; Cargo compilation was intentionally not run by request; native/GraphQL transport surfaces were not changed.

## Related Documents

- `docs/research/dioxus-ffa-ui-migration-plan.md`
- `docs/UI/graphql-architecture.md`
- `docs/UI/storefront.md`
