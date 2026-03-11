# API Architecture

Политика использования API-стилей описана в [`docs/architecture/routing.md`](./routing.md).

## Краткое резюме

RusToK использует гибридный подход: GraphQL для UI-клиентов, REST для интеграций и служебных сценариев.

| API | Endpoint | Назначение |
|-----|----------|-----------|
| GraphQL | `/api/graphql` | Единый endpoint для admin и storefront UI |
| REST | `/api/v1/…` | Внешние интеграции, webhooks, batch jobs |
| OpenAPI/Swagger | `/swagger` | Документация REST API (генерируется через `utoipa`) |
| Health | `/api/health` | Статус сервиса и модулей |
| Metrics | `/metrics` | Prometheus метрики |

## Auth transport consistency

Для auth/user сценариев (`register/sign_in`, `login/sign_in`, `refresh`, `change_password`, `reset_password`, `update_profile`) REST и GraphQL работают как thin adapters и используют общий application service `AuthLifecycleService` (`apps/server/src/services/auth_lifecycle.rs`).

Это снижает дублирование бизнес-логики между transport-слоями и фиксирует единые policy для session invalidation.


## Auth lifecycle consistency и release-gate

### Единый application service

Auth/user сценарии (`register/sign_in`, `login/sign_in`, `refresh`, `change_password`, `reset_password`, `update_profile`, `create_user`) реализованы через общий `AuthLifecycleService` (`apps/server/src/services/auth_lifecycle.rs`), а transport-слои REST/GraphQL выступают thin adapters.

### Единая policy сессий

- `reset_password` / `confirm_reset` отзывают все активные сессии пользователя.
- `change_password` отзывает все сессии, кроме текущей (через `except_session_id`).
- `sign_out` использует soft-revoke (`sessions.revoked_at`) вместо hard delete.

### Transport-контракты ошибок

Для ключевых auth-ошибок используется типизированный контракт `AuthLifecycleError` с единообразным mapping в REST/GraphQL (в т.ч. `InvalidResetToken`, `UserInactive`, `UserNotFound`, `InvalidCredentials`).

### Observability

`/metrics` публикует auth lifecycle counters:

- `auth_password_reset_sessions_revoked_total`
- `auth_change_password_sessions_revoked_total`
- `auth_flow_inconsistency_total`
- `auth_login_inactive_user_attempt_total`

### Pre-release gate (операционный)

Перед выкладкой обязателен запуск:

```bash
scripts/auth_release_gate.sh --require-all-gates \
  --parity-report <staging-parity-report> \
  --security-signoff <security-signoff>
```

Скрипт:

- запускает локальные integration auth-срезы (`cargo test -p rustok-server auth_lifecycle` + `cargo test -p rustok-server auth`),
- формирует markdown gate-report и логи,
- завершает прогон с non-zero exit code при падении любого локального auth-среза или при незакрытых обязательных gate.

## Rich-text input contract (blog/forum/pages)

Для create/update операций в blog/forum/pages transport-слои (GraphQL/REST) поддерживают:

- legacy режим: `body_format`/`content_format = "markdown"` + текстовое `body`/`content`;
- rich режим: `body_format`/`content_format = "rt_json_v1"` + обязательное `content_json`.

Для `rt_json_v1` backend выполняет обязательную server-side валидацию и sanitize через RT JSON pipeline перед записью.

Для поэтапного перевода legacy-контента (markdown) в `rt_json_v1` используется server migration job `cargo run -p rustok-server --bin migrate_legacy_richtext -- --tenant-id=<uuid> [--dry-run]`:

- выбирает только tenant-scoped записи `post/comment/forum_topic/forum_reply` с `format=markdown`;
- конвертирует markdown → `rt_json_v1`, затем прогоняет через тот же server-side sanitize/validation gate (`validate_and_sanitize_rt_json`);
- выполняет safe update с retry (optimistic guard по `id + updated_at + format`), чтобы не перетирать конкурентные изменения;
- публикует счётчики прогона: `processed/succeeded/failed/skipped`;
- поддерживает idempotent restart через checkpoint-файл (`--checkpoint-file`, по умолчанию `scripts/checkpoints/legacy_richtext.json`).

### Rollout/rollback для migration job (tenant-by-tenant)

1. `dry-run` для одного tenant, проверить `failed=0` и выборку только ожидаемых kind/locale.
2. apply для этого же tenant с выделенным checkpoint-файлом.
3. smoke-read (GraphQL/API) по post/comment/topic/reply и проверка формата `rt_json_v1`.
4. повторить цикл для следующего tenant; не запускать multi-tenant bulk без checkpoint isolation.

Rollback стратегия:

- кодовый rollback: остановить job и вернуть read/write трафик на legacy markdown (обратная совместимость сохраняется контрактом API);
- data rollback: восстановить записи конкретного tenant из DB backup/snapshot, снятого перед apply;
- при частичном падении возобновлять с того же checkpoint (идемпотентно) вместо ручного массового rewrite.

## GraphQL схема

GraphQL схема формируется из per-domain объектов через `MergedObject`:

- `CommerceQuery` / `CommerceMutation` — `rustok-commerce`
- `ContentQuery` / `ContentMutation` — `rustok-content`
- `BlogQuery` / `BlogMutation` — `rustok-blog`
- `ForumQuery` / `ForumMutation` — `rustok-forum`
- `AlloyQuery` / `AlloyMutation` — `alloy-scripting`

Точка сборки схемы: `apps/server/src/graphql/schema.rs`

## Связанные документы

- [Routing policy](./routing.md) — детальная policy GraphQL vs REST
- [Architecture overview](./overview.md)
- [UI GraphQL architecture](../UI/graphql-architecture.md)
