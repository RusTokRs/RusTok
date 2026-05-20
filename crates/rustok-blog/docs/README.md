# Документация `rustok-blog`

`rustok-blog` — доменный модуль публикаций и комментарных сценариев для blog
surface. Модуль уже работает на blog-owned persistence и использует shared
платформенные контракты только там, где это оправдано по границе ответственности.

**Статус contract stability:** полностью достигнут. Channel-aware semantics и
taxonomy sync подтверждены интеграционными и unit тестами.

## Назначение

- публиковать канонический blog runtime contract для posts, categories и tag relations;
- держать blog-owned transport surfaces, domain services и UI packages внутри модуля;
- развивать blog как channel-aware и taxonomy-aware домен без возврата к shared storage.

## Зона ответственности

- `PostService`, `CommentService`, `CategoryService`, `TagService` и blog state machine;
- blog-owned storage для posts, translations, categories и typed relations;
- transport surfaces: GraphQL, REST, Leptos admin/storefront packages;
- moderation REST surface: `POST /api/blog/comments/{id}/moderate` для approve/spam/trash transitions c RBAC `blog_posts:manage`;
- channel visibility для публикаций и интеграция с `rustok-channel`;
- reuse shared taxonomy dictionary через `blog_post_tags`, не отдавая attachment ownership наружу;
- observability через `rustok-telemetry`: `metrics::record_read_path_*` на GraphQL/REST read paths,
  `#[instrument]` на сервисных методах, span-трекинг для post lifecycle и visibility filtering.

## Интеграция

- использует `rustok-taxonomy` как shared vocabulary для tag identity;
- использует `rustok-comments` как comment runtime contract;
- использует `rustok-profiles` для author presentation contract;
- использует `rustok-channel` для module-level и publication-level visibility на public read-path;
- использует `rustok-telemetry` для observability на read/write paths;
- `rustok-blog/admin` уже встраивает owner-side post SEO panel через `rustok-seo-admin-support`
  и shared capability contract модуля `rustok-seo`.

## Контрактные тесты

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

## Проверка

- `cargo xtask module validate blog`
- `cargo xtask module test blog`
- targeted tests для post lifecycle, tag/category sync, channel visibility и public/admin read-path contracts

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [CRATE_API](../CRATE_API.md)
- [Admin package](../admin/README.md)
- [Storefront package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
