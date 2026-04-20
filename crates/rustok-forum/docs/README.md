# Документация `rustok-forum`

`rustok-forum` — доменный модуль forum/Q&A сценариев. Модуль уже работает на
forum-owned persistence и должен оставаться самостоятельной bounded context
границей, не откатываясь обратно в shared storage модель.

## Назначение

- публиковать канонический forum runtime contract для categories, topics, replies и moderation;
- держать forum-owned transport surfaces, Q&A capabilities и UI packages внутри модуля;
- развивать forum как taxonomy-aware и channel-aware домен с явной observability surface.

## Зона ответственности

- `CategoryService`, `TopicService`, `ReplyService`, `ModerationService`;
- forum-owned storage для categories, topics, replies, votes, solutions, subscriptions и user stats;
- transport surfaces: GraphQL, REST, Leptos admin/storefront packages;
- tag attachments через `forum_topic_tags` при shared vocabulary в `rustok-taxonomy`;
- visibility, moderation и user-facing derived fields в forum read/write contracts.

## Интеграция

- использует `rustok-content` только как shared helper/orchestration dependency;
- использует `rustok-taxonomy` как shared dictionary для tag identity;
- использует `rustok-profiles` для author presentation contract;
- использует `rustok-channel` для visibility/pilot gating на public read-path.
- `rustok-forum/admin` уже встраивает owner-side SEO panels через `rustok-seo-admin-support`,
  а `rustok-seo` теперь держит target kinds `forum_category` и `forum_topic` для shared runtime/resolver contract.

## Проверка

- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- targeted tests для topic/reply lifecycle, moderation, votes, subscriptions и visibility contracts

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Admin UI package](../admin/README.md)
- [Storefront UI package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
