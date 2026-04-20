# Документация `rustok-blog`

`rustok-blog` — доменный модуль публикаций и комментарных сценариев для blog
surface. Модуль уже работает на blog-owned persistence и использует shared
платформенные контракты только там, где это оправдано по границе ответственности.

## Назначение

- публиковать канонический blog runtime contract для posts, categories и tag relations;
- держать blog-owned transport surfaces, domain services и UI packages внутри модуля;
- развивать blog как channel-aware и taxonomy-aware домен без возврата к shared storage.

## Зона ответственности

- `PostService`, `CommentService`, `CategoryService`, `TagService` и blog state machine;
- blog-owned storage для posts, translations, categories и typed relations;
- transport surfaces: GraphQL, REST, Leptos admin/storefront packages;
- channel visibility для публикаций и интеграция с `rustok-channel`;
- reuse shared taxonomy dictionary через `blog_post_tags`, не отдавая attachment ownership наружу.

## Интеграция

- использует `rustok-taxonomy` как shared vocabulary для tag identity;
- использует `rustok-comments` как comment runtime contract;
- использует `rustok-profiles` для author presentation contract;
- использует `rustok-channel` для module-level и publication-level visibility на public read-path.
- `rustok-blog/admin` уже встраивает owner-side post SEO panel через `rustok-seo-admin-support`
  и shared capability contract модуля `rustok-seo`.

## Проверка

- `cargo xtask module validate blog`
- `cargo xtask module test blog`
- targeted tests для post lifecycle, tag/category sync, channel visibility и public/admin read-path contracts

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Admin package](../admin/README.md)
- [Storefront package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
