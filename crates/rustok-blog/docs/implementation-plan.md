# План реализации `rustok-blog`

Статус: contract stability полностью достигнут; модуль перешёл в режим product
hardening и operability rollout. Channel-aware semantics и taxonomy sync подтверждены
интеграционными и unit тестами. GraphQL/REST adapters, Leptos admin/storefront
packages и module metadata синхронизированы.

## Execution checkpoint

- Current phase: phase_b_in_progress
- Last checkpoint: FFA slice #53 completed (storefront published-post empty-state message handoff switched to `core::published_posts_empty_state_message(...)`, preserving dual-path transport contract and thin UI rendering branch).
- Next step: Зафиксировать evidence по parity checklist и выбрать следующий один use-case для admin/storefront core extraction без изменения transport-контракта.
- Open blockers: None.
- Hand-off notes for next agent:
  1. Продолжать one-task-per-iteration: один helper/use-case -> storefront/admin -> docs double-check.
  2. Не менять dual-path контракт (`native #[server]` + GraphQL fallback) при FFA-декомпозиции.
  3. После каждого slice обновлять parity evidence (`docs/verification/ffa-ui-parity-checklist.md`).
- Last updated at (UTC): 2026-05-25T03:14:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Evidence:
  - storefront/admin helper slices продолжают вынос UI decision logic в `core` без изменения dual-path transport contract;
  - native `#[server]` + GraphQL fallback остаются параллельными путями, removal/replace GraphQL не выполнялся;
  - backend boundary пока работает в in-process модели; remote extraction readiness ведётся как эволюционный трек без смены ownership/contract.
- Last verified at (UTC): 2026-05-24T18:00:00Z
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
- для storefront UI уже выделен первый FFA core slice: formatting/fallback helper-логика вынесена в `storefront/src/core.rs`, Leptos UI слой использует `core::*` и не меняет transport wiring.

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
- [x] Sync admin surface for the same helper family where applicable and attach parity evidence.
- [ ] `cargo xtask module validate blog` / `cargo xtask module test blog` rerun after next slice touching runtime contract.

## Double documentation verification (current slice)

- [x] Pass #1 (code/docs consistency): storefront helper extraction отражён в трекере и локальных docs.
- [x] Pass #2 (cleanup stale wording): удалены формулировки bootstrap-only статуса; execution checkpoint синхронизирован с текущим phase B контекстом.
