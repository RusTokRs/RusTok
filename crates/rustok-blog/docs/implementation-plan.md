# План реализации `rustok-blog`

Статус: contract stability полностью достигнут; модуль перешёл в режим product
hardening и operability rollout. Channel-aware semantics и taxonomy sync подтверждены
интеграционными и unit тестами. GraphQL/REST adapters, Leptos admin/storefront
packages и module metadata синхронизированы.

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
  `#[instrument]` на всех сервисных методах, span-трекинг для post lifecycle.

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
