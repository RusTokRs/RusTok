# Документация модуля `rustok-comments`

`rustok-comments` — доменный модуль для классических комментариев вне форума.

## Назначение

- дать отдельную storage-boundary для комментариев к blog post и другим opt-in non-forum сущностям;
- убрать комментарии из shared `content`-storage модели;
- зафиксировать, что `comments` и `forum replies` — разные доменные сущности;
- подготовить модульную основу для будущих conversion flow между `blog` и `forum` через orchestration.

## Зона ответственности

- `rustok-comments` владеет только generic comments domain, его schema и service-level контрактами;
- `rustok-forum` продолжает владеть `forum_topics` и `forum_replies`;
- `rustok-content` остаётся shared library + orchestration слоем и не должен снова стать storage owner для комментариев;
- conversion flow `post + comments -> topic + replies` и обратно должен жить в orchestration, а не через общую таблицу или live sync.

## Текущий статус

- модуль зарегистрирован в workspace, `modules.toml` и optional server wiring;
- module-owned schema `comment_threads`, `comments`, `comment_bodies` реализована;
- `rustok-blog` уже переведён на `rustok-comments` для comment read/write path;
- shared rich-text/body-format и locale fallback contract выровнены с `rustok-content`;
- thread status contract больше не декоративный: `closed` реально блокирует новый
  create-path, а terminal comment statuses (`spam`, `trash`) требуют moderation scope;
- модуль теперь публикует `rustok-comments-admin` как module-owned Leptos moderation UI;
- для operator-facing read/write path добавлены service-level методы `list_threads`,
  `get_thread_detail`, `set_thread_status` и `set_comment_status`;
- product decision по `pages <-> comments` зафиксирован: у `rustok-pages` нет default
  integration с `rustok-comments`, а будущие page-like discussion surfaces возможны только
  как explicit opt-in.

## Интеграция

- `rustok-blog` уже переведён на `rustok-comments` для live comment read/write path;
- moderation UI публикуется как module-owned Leptos surface `rustok-comments-admin`;
- transport adapters и host wiring остаются в `apps/server`, но доменная логика и moderation contract принадлежат модулю;
- future integrations для page-like surfaces должны оформляться как явный opt-in contract.

## Module-owned admin UI и transport rule

- `rustok-comments-admin` монтируется в Leptos Admin как module-owned UI на `/modules/comments`.
- Внутренний data-layer для moderation surface строится через native `#[server]` calls поверх `CommentsService`.
- Отдельный GraphQL/REST fallback для этого UI не добавляется: у `rustok-comments` не было собственного legacy transport surface, и это зафиксированное исключение из общего dual-path правила.
- Существующая интеграция `rustok-blog -> rustok-comments` при этом не меняется.

## Status contract

- `comment_threads.status = open|closed` управляет только приёмом новых
  комментариев; закрытый thread остаётся читаемым, но не принимает новые записи;
- обычный create-path допускает только `pending|approved`;
- `spam|trash` считаются moderation statuses и требуют `comments:moderate`
  или `comments:manage`;
- смена статуса thread делается через service-level
  `set_thread_status_for_target`, а не прямой записью в БД из transport слоя.

## Наблюдаемость

- service entry-points `create_comment`, `get_comment`, `update_comment`,
  `delete_comment`, `list_comments_for_target` пишут
  `rustok_module_entrypoint_calls_total{module="comments",path="library"}`;
- ошибки сервиса классифицируются в низкокардинальные `database`,
  `not_found`, `forbidden`, `validation` и пишутся в
  `rustok_module_errors_total`;
- latency/error по операциям пишутся через
  `rustok_span_duration_seconds{operation="comments.*"}` и
  `rustok_spans_with_errors_total`;
- bounded read-path `list_comments_for_target` пишет
  `read_path_requested_limit/effective_limit/returned_items/query_duration/query_rows`
  с `surface="library"` и `path="comments.list_comments_for_target"`.

## Дальнейшие шаги

- если позже появятся commentable page-like surfaces, описать их отдельным spec/ADR, а не
  расширять текущий pages contract по умолчанию.


## Операционные алерты и operator playbook

- `rustok_module_errors_total{module="comments",kind="database"}` — page-now alert: это runtime/storage incident, а не нормальный moderation rejection.
- `rustok_module_errors_total{module="comments",kind="conflict"}` на `comments.create_comment` в норме должен объясняться только `CommentThreadClosed`; если всплеск идёт без осознанного close-thread действия, сначала проверяйте target binding и transport/client drift.
- `rustok_module_errors_total{module="comments",kind="forbidden"}` на create/update/delete и `set_thread_status_for_target` — warning-level сигнал на RBAC/moderation drift; сначала сверяйте effective permissions caller-а.
- `rustok_module_errors_total{module="comments",kind="validation"}` для обычных bad payload допустим, но повторяющиеся попытки писать `spam|trash` без moderation scope надо трактовать как client/moderation UX regression.
- Для `comments.list_comments_for_target` смотрите вместе stage-level `query_duration/query_rows` (`comment_threads.lookup`, `comments.page`, `comment_bodies.batch`) и budget-метрики `requested_limit/effective_limit/returned_items`, чтобы отделять DB latency от over-requesting caller-ов.

Порядок действий оператора:

1. Сначала классифицируйте всплеск по `kind`: `database`, `conflict`, `forbidden` или `validation`.
2. Для `conflict` сверяйте состояние target thread в `comment_threads` и последние вызовы `set_thread_status_for_target`; закрытый thread должен полностью объяснять reject pattern.
3. Для `forbidden` проверяйте недавние RBAC-изменения и caller scopes: `spam|trash` и смена thread status должны идти только от moderation-capable caller-ов.
4. Для latency без error spike сначала разбирайте read-path stages, а не эскалируйте сразу общий DB incident.
5. Для sustained `database` errors переключайтесь на общий DB/runtime incident flow: connections, recent deploy, migration drift, query pressure.

## Проверка

- `cargo xtask module validate comments`
- `cargo xtask module test comments`
- targeted tests для moderation/status contract, module-owned admin UI и blog integration path

## Связанные документы

- [План реализации](./implementation-plan.md)
- [README crate](../README.md)
- [ADR: `rustok-pages` не получает default-интеграцию с `rustok-comments`](../../../DECISIONS/2026-03-29-pages-comments-no-default-integration.md)
- [Карта документации](../../../docs/index.md)
