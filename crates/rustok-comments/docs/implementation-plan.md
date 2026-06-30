# План реализации `rustok-comments`

Этот документ фиксирует локальный roadmap модуля `rustok-comments`.

## Execution checkpoint

- Current phase: FBA provider baseline for generic comment threads
- Last checkpoint: owner FBA batch закрепил canonical shared policy: comment create/update/delete вызывают `PortCallPolicy::write()` до tenant parsing/service invocation, а policy внутри `rustok-core` уже обеспечивает deadline + idempotency; отдельный дублирующий `require_write_semantics()` не нужен. Read paths остаются на `PortCallPolicy::read()`.
- Next step: Закрыть runtime contract execution/fallback smoke для `CommentsThreadPort` и подтвердить blog embedded/native compatibility snapshots; для FFA — не расширять native-only admin transport без нового legacy/headless contract, а поддерживать parity/evidence guardrails.
- Open blockers: отсутствуют; native-only comments admin exception зафиксирован, потому что у модуля не было legacy GraphQL/REST admin surface.
- Hand-off notes for next agent: После каждого FFA/FBA инкремента обновлять этот блок, локальный FFA/FBA status block и central readiness board в одном PR.
- Last updated at (UTC): 2026-06-19T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - пакетный no-compile gate `scripts/verify/verify-owner-fba-runtime-order.mjs` проверяет `crates/rustok-comments/contracts/evidence/comments-provider-runtime-order-smoke.json`: read/write policy order, idempotency через canonical write policy, owner `CommentsService` invocation, typed error mapping и fallback/degraded parity; статус остаётся `in_progress` до live provider/consumer execution;
  - `rustok-comments-admin` теперь имеет явные `admin/src/core.rs`, `admin/src/transport/mod.rs`, `admin/src/transport/native_server_adapter.rs` и `admin/src/ui/leptos.rs`; `admin/src/lib.rs` больше не содержит render/business logic, не wires pre-FFA `api.rs` и публикует только `CommentsAdmin`;
  - covered admin UI больше не вызывает raw `api::*` напрямую из Leptos render layer, а идёт через module-owned transport facade;
  - status filter parsing, thread list/detail target/status labels, comment row identity/locale/body mapping и transport request/command DTO construction вынесены в Leptos-free core и покрыты unit tests;
  - selected-thread и locale route/query key ownership, normalization и host write intent теперь живут в Leptos-free core на shared `UiRouteQueryUpdate`, а Leptos adapter только применяет готовый `CommentsAdminRouteQueryWrite` через host writer;
  - fast boundary guardrail `scripts/verify/verify-comments-admin-boundary.mjs` включён в aggregate `verify:ffa:ui:migration` и закрепляет native-only comments admin exception без package-local GraphQL fallback; fixture suite `scripts/verify/verify-comments-admin-boundary.test.mjs` включён в aggregate `test:verify:ffa:ui:migration` и проверяет canonical split, legacy `api.rs`, Leptos-free core, route/query ownership, transport facade isolation, GraphQL-fallback prohibition и server-function adapter placement;
  - текущий admin transport остаётся native-only single-adapter server-function path, path зафиксирован typed `CommentsAdminTransportPath`/`ACTIVE_TRANSPORT_PATH`, а отдельный GraphQL/REST fallback не добавляется как module-documented exception без legacy admin transport surface;
  - FBA provider registry `crates/rustok-comments/contracts/comments-fba-registry.json`, neutral `CommentsThreadPort`/`comments.thread.v1` and static contract evidence `crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json` are locked for blog and future commentable-surface consumers; write operations require shared `PortCallPolicy::write()`, read operations require shared `PortCallPolicy::read()`, and typed `PortError` mapping remains owned by `rustok-comments`;
  - fast FBA guardrail `scripts/verify/verify-comments-fba.mjs` / `npm run verify:comments:fba` checks manifest metadata, port source markers, static evidence drift and central readiness-board sync; status remains below `boundary_ready` until runtime contract execution and fallback smoke evidence land;
- Owner: `rustok-comments` module team

## Область работ

- удерживать `rustok-comments` отдельной storage/domain границей для generic comments вне `rustok-forum`;
- развивать moderation/status contract, module-owned admin UI и opt-in integrations без возврата комментариев в shared `content`-модель;
- синхронизировать runtime contract, local docs и host wiring по мере появления новых commentable surfaces.

## Текущее состояние

- `rustok-comments` уже является live storage-owner для generic comments;
- `rustok-blog` использует модуль в production read/write path;
- `rustok-comments-admin` опубликован как module-owned moderation UI;
- observability baseline и thread status contract уже зафиксированы в runtime.

## Этапы

### Этап 1. Module foundation

- [x] добавить crate, `CommentsModule`, permissions и module manifest;
- [x] подключить модуль в workspace, `modules.toml`, server feature wiring и central docs;
- [x] зафиксировать локальную storage/API стратегию внутри module docs.

### Этап 2. Storage boundary

- [x] спроектировать таблицы `comment_threads`, `comments`, `comment_bodies`;
- [x] добавить module-owned migrations;
- [x] ввести entities/repositories и базовый `CommentService`.

### Target schema

- `comment_threads`
  - thread ownership per `(tenant_id, target_type, target_id)`
  - typed `status`, `comment_count`, `last_commented_at`
- `comments`
  - typed `thread_id`, `author_id`, `parent_comment_id`, `status`, `position`
  - no reuse of forum reply storage
- `comment_bodies`
  - locale-aware body storage with explicit `body_format`
  - canonical support for shared rich-text contracts from `rustok-content`

### Required indexes and constraints

- unique `(tenant_id, target_type, target_id)` on `comment_threads`
- unique `(comment_id, locale)` on `comment_bodies`
- ordered list indexes on `(thread_id, position)` and `(thread_id, created_at)`

### Этап 3. Domain contracts

- [x] определить target binding contract для blog и generic opt-in non-forum surfaces;
- [x] определить moderation/status contract для comment-domain;
- [x] свести comment body к shared rich-text contract.

### Этап 4. Integrations

- [x] перевести `rustok-blog` на `rustok-comments`;
- [x] определить интеграцию `rustok-pages` с `rustok-comments`: default integration не
  вводится, future page-like discussion surfaces возможны только как explicit opt-in;
- [x] добавить transport adapters в `apps/server`.

### Этап 5. Orchestration compatibility

- [x] реализовать mapping между `blog comments` и `forum replies` через `rustok-content`;
- [x] покрыть conversion flows end-to-end тестами после появления orchestration service.

### Этап 6. Observability baseline

- [x] добавить module-level entrypoint/error metrics для service entry-points;
- [x] добавить read-path budget/query metrics для `list_comments_for_target`;
- [x] определить moderation/status alerts и operator playbook после фиксации
  финального comment-moderation contract.

## Проверка

- `cargo xtask module validate comments`
- `cargo xtask module test comments`
- targeted tests для moderation/status contract, blog integration и admin UI runtime wiring

## Правила обновления

1. При изменении comment-domain contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata и UI wiring синхронизировать `rustok-module.toml`.

## Детализация текущего состояния

- `rustok-comments` — больше не scaffold, а live storage-owner для generic comments;
- `rustok-blog` уже использует модуль в production read/write path;
- `rustok-pages` не получает default comments surface; pages-level integration сознательно
  оставлена вне текущего product scope;
- observability baseline для service-layer уже поднят: module entrypoint/error
  counters, span duration/error и read-path budget/query metrics на list path;
- thread status contract уже enforced в runtime: `closed` блокирует новый
  create-path, а `spam|trash` требуют moderation scope;
- дальнейший scope модуля теперь связан не со split, а с расширением moderation и
  product-level integrations.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
