# rustok-forum docs

В этой папке хранится документация модуля `crates/rustok-forum`.

## Documents

- [Implementation plan](./implementation-plan.md)
- [Admin UI package](../admin/README.md)
- [Storefront UI package](../storefront/README.md)

## Status

- ✅ GraphQL и REST адаптеры модуля перенесены в `crates/rustok-forum`; `apps/server` держит только тонкий shim/composition-root слой.
- ✅ Общие transport-контракты (`AuthContext`, `TenantContext`, `RequestContext`, GraphQL helpers) модуль получает из `rustok-api`.
- ✅ Author presentation для topics/replies теперь идёт через `rustok-profiles::ProfilesReader`: GraphQL payloads `GqlForumTopic` и `GqlForumReply` отдают `author_profile` рядом с `author_id`, а в shared server runtime используют `ProfileSummaryLoader` как request-scoped cache там, где он зарегистрирован.
- ✅ Publishable Leptos admin UI для forum теперь живёт в `crates/rustok-forum/admin/`; host admin подключает пакет через manifest-driven `build.rs`.
- ✅ Publishable Leptos storefront UI для forum теперь живёт в `crates/rustok-forum/storefront/`; host storefront подключает пакет через manifest-driven `build.rs`, а public GraphQL read-path отдаёт categories, topic feed, thread detail и replies для `/modules/forum`.

## Event contracts

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)

## Rich-text contract

- Blog/Forum backend принимает `markdown` и `rt_json_v1` payload; для `rt_json_v1` выполняется обязательные `schema validation + sanitize` на сервере.
- Клиентские валидаторы считаются advisory-only и не являются источником истины.
- Детали спецификации: [docs/standards/rt-json-v1.md](../../../docs/standards/rt-json-v1.md).
- **Response contract (read path):** backend всегда возвращает `*_format` (`body_format`/`content_format`) и нормализованное поле `content_json` для `rt_json_v1`; при `markdown` `content_json = null`, а текст остаётся в `body/content` для обратной совместимости.
- Для миграции legacy markdown-записей используйте tenant-scoped job `cargo run -p rustok-server --bin migrate_legacy_richtext -- --tenant-id=<uuid> [--dry-run]`; job идемпотентный (checkpoint + retry) и безопасен для поэтапного rollout tenant-by-tenant.
