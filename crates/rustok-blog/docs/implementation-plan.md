# План реализации `rustok-blog`

Статус: contract stability полностью достигнут; модуль перешёл в режим product
hardening и operability rollout. Channel-aware semantics и taxonomy sync подтверждены
интеграционными и unit тестами. GraphQL/REST adapters, Leptos admin/storefront
packages и module metadata синхронизированы.

## Execution checkpoint

- Current phase: ffa_admin_action_command_core_boundary
- Last checkpoint: FFA slice #82 moved admin publish/unpublish, archive and delete action command preparation into Leptos-free core command view-models, so the Leptos adapter dispatches prepared operation/locale/busy-key envelopes through the existing transport facade.
- Next step: Continue with small admin render/command fragments that reduce real coupling without changing the dual-path contract, or add adapter-level parity evidence around transport failure classification.
- Open blockers: None.
- Hand-off notes for next agent:
  1. Продолжать one-task-per-iteration: один helper/use-case -> storefront/admin -> docs double-check.
  2. Не менять dual-path контракт (`native #[server]` + GraphQL fallback) при FFA-декомпозиции.
  3. После каждого slice обновлять parity evidence (`docs/verification/ffa-ui-parity-checklist.md`).
- Last updated at (UTC): 2026-06-13T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Evidence:
  - storefront/admin helper slices продолжают вынос UI decision logic в `core` без изменения dual-path transport contract; storefront shell copy and selected-post route/query state now use framework-agnostic core view-model/state; storefront native and GraphQL transport paths are separated into explicit adapter modules; transport adapters consume core-owned fetch request state instead of raw UI tuples; admin calls now go through a module-owned `admin/src/transport.rs` facade instead of direct `api::*` calls from the Leptos adapter;
  - native `#[server]` + GraphQL fallback остаются параллельными путями, GraphQL removal/replacement не выполнялся;
  - backend boundary пока работает в in-process модели; remote extraction readiness ведётся как эволюционный трек без смены ownership/contract;
  - FFA slice #75 выделила `admin/src/ui/leptos.rs` и `storefront/src/ui/leptos.rs` как явные Leptos render adapters, а crate roots стали тонким module wiring/re-export слоем; FFA slice #76 перенесла admin save-command preparation в `admin/src/core.rs`: required-field validation, validation issue construction, create/update operation selection и busy-key policy теперь Leptos-free, поэтому adapter только собирает form state/локализованный текст и вызывает module-owned transport facade; FFA slice #77 закрепила это быстрым source-level guardrail `scripts/verify/verify-blog-admin-boundary.mjs`; FFA slice #78 перенесла admin editor form-state mapping/reset defaults в `BlogPostEditorFormState` внутри `admin/src/core.rs`, оставив Leptos adapter тонким signal-application слоем; FFA slice #79 перенесла admin table-row display/action state в `BlogPostAdminTableRowViewModel`, оставив Leptos table fragment markup/event-binding слоем; FFA slice #80 завершила row action presentation переносом archive/delete labels и archive visibility в этот же core-owned row view-model; FFA slice #81 перенесла issue banner visibility/class/label/message mapping в `BlogPostAdminIssueBannerViewModel`, оставив Leptos adapter без write-path presentation policy; FFA slice #82 перенесла publish/unpublish, archive и delete action command preparation в core-owned `BlogPostStatusCommand`, `BlogPostArchiveCommand` и `BlogPostDeleteCommand`, оставив Leptos adapter только выбирать transport call по подготовленной операции.
- Last verified at (UTC): 2026-06-13T00:00:00Z
- Owner: `rustok-blog` module team

## Область работ

- удерживать `rustok-blog` как самостоятельный blog domain module;
- синхронизировать post/category/tag/comment contracts, UI packages и local docs;
- развивать channel-aware и taxonomy-aware semantics без возврата к shared content storage;
- обеспечить observability для post lifecycle, visibility filtering и moderation flows.

## Текущее состояние

- blog posts, translations, categories и typed tag relations уже живут в module-owned storage;
- GraphQL/REST adapters и Leptos admin/storefront surfaces уже живут внутри модуля;
- comments runtime contract приходит из `rustok-comments`, а author presentation — из `rustok-profiles`;
- public read-path уже поддерживает module-level и publication-level channel visibility;
- `blog_post_channel_visibility` таблица реализует typed channel allowlists;
- blog services re-validate RBAC локально для posts, categories и tags;
- customer read paths restricted to published posts;
- observability уже частично реализована: `metrics::record_read_path_*` на GraphQL/REST read paths,
  `#[instrument]` на всех сервисных методах, span-трекинг для post lifecycle;
- для storefront UI уже выделен FFA core/transport/ui split: formatting/fallback helper-логика вынесена в `storefront/src/core.rs`, native/GraphQL adapters живут в `storefront/src/transport/`, а Leptos render adapter — в `storefront/src/ui/leptos.rs`; admin UI использует `admin/src/core.rs`, `admin/src/transport.rs` facade и `admin/src/ui/leptos.rs`.

## Этапы

### 1. Contract stability

- [x] закрыть storage split и blog-owned transport boundary;
- [x] перенести tag vocabulary на shared `rustok-taxonomy`, сохранив blog-owned attachments;
- [x] встроить channel-aware public visibility contract;
- [x] удерживать sync между runtime contracts, UI packages и module metadata.

### 2. Product hardening

- [ ] довести rate limiting и performance baseline для public/write paths;
  - infrastructure: `rustok-core::security::rate_limit::RateLimiter` exists (token bucket, IP/key-based);
  - task: wire `RateLimiter` into blog REST/GraphQL public endpoints via middleware.
- [ ] довести search/index integration без размывания blog domain boundary;
  - blog публикует domain events (`blog.post.created/updated/published/archived/deleted/unpublished`);
  - events уже помечены `affects_index() = true` — `rustok-index` consumer обрабатывает их;
  - task: ensure indexer correctly maps events to search schema (проверить маппинг в `rustok-index`).
- [x] удерживать category/tag/comment semantics покрытыми targeted integration tests.
- [x] добавить moderation API endpoints для comment status transitions (approve/spam/trash).
  - добавлен REST endpoint `POST /api/blog/comments/{id}/moderate`;
  - endpoint маршрутизирован через `controllers/` и вызывает `CommentService::moderate_comment`;
  - moderation RBAC закреплён на `BLOG_POSTS_MANAGE`, статус маппится в `rustok_comments::CommentsService::set_comment_status`.

### 3. Operability

- [x] развивать observability для post lifecycle, visibility filtering и moderation flows;
  - `#[instrument]` на всех сервисных методах (`PostService`, `CategoryService`, `TagService`, `CommentService`);
  - `rustok_comments::CommentsService::set_comment_status` также имеет `#[instrument]` (fields: tenant_id, comment_id, status);
  - `metrics::record_read_path_*` на GraphQL/REST read paths;
  - state machine transitions логируются через `tracing::info!` (Draft→Published, etc.);
  - `CommentStatus` transitions существуют в `state_machine.rs` (`approve`, `mark_spam`, `trash`).
- [ ] документировать новые public/runtime guarantees одновременно с изменением сервисов;
- [x] держать локальные docs, README и manifest metadata синхронизированными.

## Проверка

- [x] `cargo xtask module validate blog`
- [x] `cargo xtask module test blog`
- [x] targeted tests для lifecycle, taxonomy sync, channel visibility и UI-facing read contracts
- [x] контрактные тесты покрывают все публичные use-case

## Контрактные тесты (contract surface)

Тесты в `tests/contract_surface.rs` и `tests/integration.rs` покрывают:

- **Post lifecycle**: create → draft → publish → archive → restore
- **Locale fallback**: normalize → requested → en → first available
- **Channel visibility**: typed `blog_post_channel_visibility` allowlists, empty = global
- **Taxonomy sync**: blog tags ↔ `rustok-taxonomy` vocabulary
- **RBAC enforcement**: customer не может создавать/читать draft posts
- **GraphQL read paths**: public vs authenticated channel gating
- **Events**: blog.post.created/updated/published/archived/deleted/unpublished
- **Comments**: thread, locale fallback, status transitions, RBAC
- **State machine**: BlogPost status transitions, CommentStatus transitions

## Правила обновления

1. При изменении blog runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении dependency graph, UI wiring или metadata синхронизировать `rustok-module.toml`.
4. При изменении channel/tag semantics обновлять также связанные module docs и central references.
5. При добавлении новых public use-case добавлять соответствующий contract test в `tests/contract_surface.rs`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.

## FFA pilot migration tracker (rustok-blog)

- [x] Slice 1: storefront formatting/fallback helper extraction (`fallback_text`, `count_label`, `open_link_label`, `label_value_pair`, `error_with_context`, `summarize_content`, `module_href`, fallback wrappers).
- [x] `crates/rustok-blog/storefront/src/lib.rs` переведён на `core::*` helper-слой для выбранного use-case.
- [x] Dual-path transport contract preserved (`native #[server]` + GraphQL fallback).
- [x] Slice 2: admin submit-error banner class fallback moved to core (`issue_banner_class_or_hidden`) without changing transport path.
- [x] Slice 3: admin required draft-field guard moved to core (`has_required_draft_fields`) without changing transport path.
- [x] Slice 4: admin raw-body warning visibility guard moved to core (`is_markdown_format`, `should_show_raw_body_warning`) without changing transport path.
- [x] Slice 5: admin editing-post predicate moved to core (`is_editing_post`) and reused in toggle/archive/delete/table flows without changing transport path.
- [x] Slice 6: admin editing-mode predicate moved to core (`is_editing_mode`) and reused for form mode labels without changing transport path.
- [x] Slice 7: admin edit-banner visibility condition switched from inline `.is_some()` to `core::is_editing_mode` for consistent predicate reuse.
- [x] Slice 8: admin submit-issue visibility predicate switched from inline `.is_some()` to `core::has_issue` for consistent helper reuse.
- [x] Slice 9: admin table empty-state predicate switched from inline `.is_empty()` to `core::has_items` for consistent helper reuse.
- [x] Slice 10: admin submit-issue kind extraction switched from inline `.map(|issue| issue.kind)` to `core::issue_kind` for consistent helper reuse.
- [x] Slice 11: admin slug-autofill predicate switched from inline `!has_non_empty_text` to `core::should_autofill_slug` for consistent helper reuse.
- [x] Slice 12: admin selected-post load predicate switched from inline `Some(post_id) if has_non_empty_text(...)` to `core::should_load_selected_post`.
- [x] Slice 13: admin selected-post id extraction switched from nested `if let Some(post_id)` to `core::selected_post_id_if_loadable`.
- [x] Slice 14: admin publish-toggle next-state predicate switched from inline `!is_published` to `core::next_publish_state`.
- [x] Slice 15: admin publish-path predicate switched from inline `if publish` to `core::should_publish_now`.
- [x] Slice 16: admin submit-issue label mapping switched from inline `issue_kind_label(issue.kind)` to `core::issue_label_for`.
- [x] Slice 17: admin locale argument mapping switched from inline `Some(post_locale[..])` to `core::locale_arg`.
- [x] Slice 18: admin optional text default mapping switched from inline `unwrap_or_default()` to `core::optional_text_or_default`.
- [x] Slice 19: admin tags input formatting switched from inline `post.tags.join(\", \")` to `core::tags_input_value`.
- [x] Slice 20: admin submit-button state selection moved to core (`submit_button_state` + `SubmitButtonState`) to keep UI as thin label mapper.
- [x] Slice 21: admin selected-post effect branch switched to core-owned loadability predicate/normalization flow.
- [x] Slice 22: admin selected-post id normalization moved to core (`loadable_post_id`) and nested UI branch removed.
- [x] Slice 23: admin form title select (`edit/create`) switched from inline conditional to `core::edit_action_label`.
- [x] Slice 24: admin submit button label mapping switched from inline `match` to `core::submit_action_label`.
- [x] Slice 25: admin selected-post load fallback reset path deduplicated into one helper closure (`reset_form_to_defaults`) to keep UI effect branch thinner.
- [x] Slice 26: admin delete-path reset predicate switched from inline `is_editing_post(...)` to `core::should_reset_form_after_delete(...)`.
- [x] Slice 27: admin selected-post/effect default reset switched from inline `reset_form(...)` to shared `reset_form_action.run(())` callback.
- [x] Slice 28: admin delete success branch reset switched from inline `reset_form(...)` to shared `reset_form_action.run(())` callback.
- [x] Slice 29: admin selected-post effect request mapping switched from inline `(post_id, default_locale)` tuple to `core::selected_post_request(...)`.
- [x] Slice 30: admin posts fetch locale option switched from inline `Some(locale)` to `core::locale_arg(...)` for helper consistency.
- [x] Slice 31: admin selected-post fetch locale option switched from inline `Some(requested_locale)` to `core::locale_arg(...)`.
- [x] Slice 32: admin save-path create/update branch switched from inline `match Option` to `core::is_editing_mode(...)` guard.
- [x] Slice 33: admin save-path update-id extraction switched to `core::editing_post_id_if_editing_mode(...)` and removed inline `expect(...)`.
- [x] Slice 34: admin editing-banner label mapping switched from inline `map(...).unwrap_or_default()` to `core::label_with_optional_id(...)`.
- [x] Slice 35: storefront tags/list empty-state predicates switched from inline `.is_empty()` to `core::has_items(...)` for collection guard reuse.
- [x] Slice 36: storefront status-badge label/css composition switched from inline mapping to `core::status_presentation(...)`.
- [x] Slice 37: storefront body summary/fallback composition switched from inline `Option::map(...)` flow to `core::summarized_body_or_fallback(...)`.
- [x] Slice 38: storefront post-link (`href` + localized open label) composition switched from inline pairing to `core::post_link(...)`.
- [x] Slice 39: storefront selected-post meta (`slug/locale/published`) label/value composition switched from inline calls to `core::post_meta_pairs(...)`.
- [x] Slice 40: storefront selected-post fallback mapping (`slug/excerpt/published_at`) switched from inline fallbacks to `core::selected_post_fallback_fields(...)`.
- [x] Slice 41: storefront published-post card summary (`excerpt` + `href` + open label) switched from inline composition to `core::list_post_summary(...)`.
- [x] Slice 42: storefront published-post locale meta label/value composition switched from inline call to `core::list_post_locale_meta(...)`.
- [x] Slice 43: storefront published-post card field composition (`excerpt` + `href` + open label + locale meta) unified via `core::list_post_card_fields(...)`.
- [x] Slice 44: storefront selected-post meta separator switched from inline literal to `core::meta_separator(...)`.
- [x] Slice 45: storefront selected-post meta row (`slug/locale/published` + separator) unified via `core::selected_post_meta_row(...)`.
- [x] Slice 46: storefront published-post card payload (`status` + `excerpt` + `href` + open label + locale meta) unified via `core::list_post_card_view(...)`.
- [x] Slice 47: storefront status-badge owned status mapping switched to `core::status_badge_view(...)`.
- [x] Slice 48: storefront selected-post tags visibility/data mapping switched to `core::selected_post_tag_items(...)`.
- [x] Slice 49: storefront published-post empty-state mapping switched to `core::published_posts_or_empty_message(...)`.
- [x] Slice 50: storefront published-post view-state mapping switched to `core::published_posts_view_state(...)`.
- [x] Slice 51: storefront published-post items normalization switched to `core::published_posts_items_or_default(...)`.
- [x] Slice 52: storefront published-post ready-state mapping switched to `core::published_posts_ready_items(...)`.
- [x] Slice 53: storefront published-post empty-state message handoff switched to `core::published_posts_empty_state_message(...)`.
- [x] Slice 54: storefront published-post empty-state payload switched to `core::published_posts_empty_state_view(...)`.
- [x] Slice 55: storefront published-post total counter label switched from direct `core::count_label(...)` usage in UI to dedicated `core::published_posts_total_label(...)` helper.
- [x] Slice 56: storefront published-post list header view (`title + total`) switched from inline UI composition to `core::published_posts_header_view(...)`.
- [x] Slice 57: storefront published-post header payload now precomputed before `view!`, removing inline `move` closure and keeping render tree strictly presentational.
- [x] Slice 58: storefront selected-post empty-state payload (`title + body`) switched from inline i18n calls in UI to `core::selected_post_empty_state_view(...)`.
- [x] Slice 59: storefront selected-post meta row now consumes typed payload `SelectedPostMetaView` from `core::selected_post_meta_view(...)` instead of local tuple destructuring.
- [x] Slice 60: storefront selected-post tags branch now consumes typed payload `SelectedPostTagsView` from `core::selected_post_tags_view(...)` instead of raw optional vector mapping.
- [x] Slice 61: storefront selected-post content fields now consume typed payload `SelectedPostContentView` from `core::selected_post_content_view(...)` instead of direct local excerpt/body rendering values.
- [x] Slice 62: storefront selected-post status fields now consume typed payload `SelectedPostStatusView` from `core::selected_post_status_view(...)` instead of direct local status/unknown label values.
- [x] Slice 63: storefront selected-post header fragment now consumes grouped typed payload `SelectedPostHeaderView` from `core::selected_post_header_view(...)` (title + meta + status).
- [x] Slice 64: storefront published-post card fragment now consumes typed payload `PublishedPostCardView` from `core::published_post_card_view(...)` instead of tuple destructuring from `list_post_card_view(...)`.
- [x] Slice 65: storefront empty-state/header fragments now consume typed payloads `SelectedPostEmptyStateView` and `PublishedPostsHeaderView` via `core::selected_post_empty_state_typed_view(...)` / `core::published_posts_header_typed_view(...)`.
- [x] Slice 66: storefront published-post empty-state fragment now consumes typed payload `PublishedPostsEmptyStateView` via `core::published_posts_empty_state_typed_view(...)` instead of tuple destructuring from `published_posts_empty_state_view(...)`.
- [x] Slice 67: storefront status-badge fragment now consumes typed payload `StatusBadgeView` via `core::status_badge_typed_view(...)` instead of tuple destructuring from `status_badge_view(...)`.
- [x] Slice 68: storefront post-link fragment now consumes typed payload `PostLinkView` via `core::post_link_typed_view(...)`; `list_post_summary(...)` switched from tuple link destructuring to typed link payload consumption.
- [x] Slice 69: storefront published-posts readiness branch now consumes typed payload enum `PublishedPostsReadyView<T>` via `core::published_posts_ready_typed_view(...)` instead of matching `Result<Vec<_>, String>` in UI.
- [x] Slice 70: storefront core helper dedup completed — removed duplicate definitions of `post_link_typed_view(...)` and `published_post_card_view(...)`, preserving single canonical typed helper path for published-post card/link mapping without transport contract changes.
- [x] Slice 71: admin post form normalization moved to core (`BlogPostFormInput`, `build_blog_post_draft`) and now reuses shared `rustok-api` UI input helpers (`normalize_ui_text`, `parse_ui_csv`) without changing native/GraphQL transport.
- [x] Slice 72: storefront shell copy and selected-post route/query state moved to core (`BlogStorefrontShellViewModel`, `BlogStorefrontRouteState`, `SELECTED_POST_QUERY_KEY`); Leptos now only reads host context/query and renders the core payload, while `slug` normalization reuses shared `rustok-api::normalize_ui_text` without changing native/GraphQL transport. Evidence: `cargo test -p rustok-blog-storefront --lib`.
- [x] Slice 73: storefront transport facade split into explicit `transport/native_server_adapter.rs` and `transport/graphql_adapter.rs`; the facade remains native-first and falls back to GraphQL, and the obsolete combined `api::fetch_storefront_blog(...)` wrapper was removed to keep ownership clear. Evidence: `cargo test -p rustok-blog-storefront --lib`.
- [x] Slice 74: storefront fetch input moved to core-owned `BlogStorefrontFetchRequest`; Leptos builds the request from `BlogStorefrontRouteState` + host locale, and native/GraphQL transport adapters now consume that typed request instead of raw `(post_slug, locale)` tuples. Evidence: `cargo test -p rustok-blog-storefront --lib`.
- [x] Slice 76: admin save-command preparation moved to `admin/src/core.rs` (`prepare_blog_post_save_command`, `BlogPostSaveOperation`, `BlogPostSaveCommand`) with unit-test evidence for required-field rejection, create operation selection and update operation selection; Leptos UI now resolves localized copy/form state, delegates validation/outcome/busy-key policy to core and only dispatches the resulting command through the module-owned transport facade.
- [x] Slice 77: fast source-level guardrail `scripts/verify/verify-blog-admin-boundary.mjs` added for the blog admin FFA boundary; it checks private crate-root wiring, Leptos-free core markers, core-owned save command helpers, UI calls through `transport`, GraphQL adapter preservation, and local/central docs sync without long Cargo compilation.
- [x] Slice 78: admin editor form-state mapping/reset defaults moved to `core::BlogPostEditorFormState`; Leptos now only applies prepared signal values for loaded-post and empty-form states, keeping post-to-form/default policy out of the render adapter. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; long `cargo test -p rustok-blog-admin --lib` was intentionally stopped after dependency compilation started to avoid the requested long compile.
- [x] Slice 79: admin table-row display/action state moved to `core::BlogPostAdminTableRowViewModel`; Leptos table rendering now consumes prepared slug/excerpt fallbacks, row busy/editing/status flags and action labels while keeping callbacks/markup local. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; targeted `timeout 20s cargo test -p rustok-blog-admin --lib table_row_view_model_composes_row_policy_without_ui_runtime` reached the timeout during dependency compilation, so no long compile was allowed.
- [x] Slice 80: admin table archive/delete action labels and archive visibility moved into `core::BlogPostAdminTableRowViewModel`, completing the row action presentation slice without changing callbacks, transport, or GraphQL/native contracts. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`.
- [x] Slice 81: admin write-path issue banner presentation moved to `core::BlogPostAdminIssueBannerViewModel`; Leptos now asks core for visible/class/label/message and renders only markup, keeping validation/runtime issue display policy portable. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; targeted `timeout 20s cargo test -p rustok-blog-admin --lib blog_post_admin_issue_banner --config profile.dev.debug=0` reached the timeout during dependency compilation, so no long compile was allowed.
- [x] Slice 82: admin publish/unpublish, archive and delete action command preparation moved to core (`BlogPostStatusCommand`, `BlogPostArchiveCommand`, `BlogPostDeleteCommand`); Leptos now sets busy state and dispatches transport from prepared command envelopes instead of composing operation/locale/busy-key policy inline. Evidence: `node scripts/verify/verify-blog-admin-boundary.mjs`; no Cargo compilation was run by request.
- [x] Sync admin surface for the same helper family where applicable and attach parity evidence.
- [ ] `cargo xtask module validate blog` / `cargo xtask module test blog` rerun after next slice touching runtime contract.

## Double documentation verification (current slice)

- [x] Pass #1 (code/docs consistency): storefront helper extraction, `slug` route/query key contract, explicit native/GraphQL adapter split, core-owned fetch request, admin save command, admin editor form-state mapping, admin table-row view-model/action presentation mapping and admin action command preparation reflected in tracker and local docs.
- [x] Pass #2 (cleanup stale wording): stale bootstrap-only wording remains absent; execution checkpoint synchronized with current phase B FFA context.
