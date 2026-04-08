# План реализации `rustok-comments`

Этот документ фиксирует локальный roadmap модуля `rustok-comments`.

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
