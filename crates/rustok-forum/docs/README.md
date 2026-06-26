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
- forum widget contract freeze surfaces: `ForumWidgetContractService`, REST endpoints `/api/forum/widgets/catalog` + `/api/forum/widgets/validate`, GraphQL query `forumWidgetCatalog`;
- forum page-builder consumer evidence: FW-2 static fallback matrix plus live Wave 1 rollout packet with control-plane audit trail, fallback/no-5xx guarantees, complete smoke outcomes, numeric SLO checks, forum-owned observability traces, keep decision, owner approvals, a monthly refresh policy, non-empty required refresh sections, and machine-readable latest-refresh provenance;
- tag attachments через `forum_topic_tags` при shared vocabulary в `rustok-taxonomy`;
- visibility, moderation и user-facing derived fields в forum read/write contracts.

## Интеграция

- использует `rustok-content` только как shared helper/orchestration dependency;
- использует `rustok-taxonomy` как shared dictionary для tag identity;
- использует `rustok-profiles` для author presentation contract;
- использует `rustok-channel` для visibility/pilot gating на public read-path: channel-restricted topics хранятся в `forum_topic_channel_access`, public GraphQL проверяет `channel_module_bindings`, а SEO/read-path фильтры потребляют host-provided request channel slug.
- `rustok-forum/admin` уже встраивает owner-side SEO panels через `rustok-seo-admin-support`,
  а `rustok-seo` теперь держит target kinds `forum_category` и `forum_topic` для shared runtime/resolver contract.

## Проверка

- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- `npm run verify:page-builder:consumer:forum` для fast FBA consumer guardrail без компиляции, включая Wave 1 smoke/SLO/trace anti-drift checks;
- targeted tests для topic/reply lifecycle, moderation, votes, subscriptions и visibility contracts;
- `npm run verify:channel:proof-points` для no-compile фиксации forum channel-aware read-path/SEO markers

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Admin UI package](../admin/README.md)
- [Storefront UI package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
