# Crate API: rustok-forum-storefront

## Public surface

### UI Components
- `ForumView`: Root storefront component for forum discussion feeds, topic lists, and thread views.

### Models
- `StorefrontForumCategoryRouteDescriptor`, `StorefrontForumCategoryRouteDisposition`, `StorefrontForumCategoryRouteResolution`: Category route resolution models.
- `StorefrontForumTopicRouteDescriptor`, `StorefrontForumTopicRouteDisposition`, `StorefrontForumTopicRouteResolution`: Topic route resolution models.

### Transport & Resolvers
- `StorefrontForumBulkReadResult`, `TransportError`: Result and error representations for forum transport.
- `fetch_storefront_forum`, `fetch_storefront_reply_current_revision`, `fetch_storefront_topic_current_revision`: Forum data query functions.
- `mark_all_storefront_topics_read`, `mark_storefront_category_read`, `mark_storefront_topic_read`: Read state tracking functions.
- `resolve_storefront_category_route`, `resolve_storefront_topic_route`: Canonical route resolution functions.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) storefront package for `rustok-forum`.
- Dual-transport architecture: native Leptos server functions in SSR/hydrate, GraphQL queries in CSR/headless.
- Internationalization via `rustok-ui-i18n` supporting English, Russian, and Arabic catalogs.

## Domain Collaborations
- Collaborates with `rustok-forum` domain services for topic feed queries and reply authoring.
- Coordinates with `rustok-channel` and `rustok-outbox` for visibility checking and events.

## Minimum Contract Set

### Input DTOs/Commands
- `ForumView`: Entrypoint storefront view mounting forum discussions.
- Topic and reply creation commands with inline quote support.
- Mark-read commands for topics and categories.

### Domain Invariants
- Only channel-visible categories and topics are accessible to users.
- Anonymous users do not mutate read-state tracking rows.
- Zero business logic inside UI view templates; route resolution and mapping live in `core`.

### Events / Outbox Side Effects
- Emits topic and reply creation events, plus user read-state progression markers.

### Errors / Failure Codes
- Missing or deleted routes resolve cleanly to canonical 404/not-found dispositions.
- Transport failures display accessible error alert feedback.

