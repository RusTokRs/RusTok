# Crate API: rustok-forum-admin

## Public surface

### UI Components
- `ForumAdmin`: Root route dispatcher and admin workspace component for forum management.

### Page Builder Contributions
- `ForumContributionAdapter`, `ForumWidgetOwnerSchemaRef`, `ForumWidgetPropertyEditorModel`, `ForumWidgetRenderModel`: Widget integration types for the Fly page builder.
- `build_forum_admin_contribution_registry`, `forum_contribution_manifest`, `forum_fly_registry_set`, `forum_full_admin_contribution_policy`, `forum_widget_contribution`, `forum_widget_preview_contribution`, `register_forum_fly_widgets`: Contribution registry functions.

### Transport & Adapters
- `ForumPageBuilderTransportAttestationResponse`, `ForumWidgetPreviewTransportRequest`, `attest_forum_page_builder_transport`, `preview_forum_page_builder_widget`: Page-builder widget preview transports.
- `ForumWidgetPropertySchemaTransportRequest`, `ForumWidgetPropertySchemaTransportResponse`, `ForumWidgetPropertyValidationIssueTransport`, `ForumWidgetPropertyValidationTransportRequest`, `ForumWidgetPropertyValidationTransportResponse`, `load_forum_page_builder_widget_property_schema`, `validate_forum_page_builder_widget_properties`: Property schema and validation transports.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-forum`.
- Integrates with the Fly page builder widget framework (`fly`, `fly-ui`).
- Dual-transport support: native Leptos server functions in SSR/hydrate, GraphQL queries in CSR/headless.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).
- SEO management via `rustok-seo-panel` and `rustok-seo-targets`.

## Domain Collaborations
- Collaborates with `rustok-forum` backend services (`TopicService`, `ReplyService`, `ForumTopicMergeService`).
- Interacts with `rustok-outbox` for event propagation.

## Minimum Contract Set

### Input DTOs/Commands
- `ForumAdmin`: Entrypoint workspace component mounting forum administration.
- Topic merge, reply range move, topic split, and slug rename commands.

### Domain Invariants
- Zero framework types inside model definitions (`topic_merge_model`, `topic_split_model`, etc.).
- Topic and reply operations enforce tenant boundary constraints.
- Merging and splitting preserve reply chronology and accepted solution markers.

### Events / Outbox Side Effects
- Emits topic lifecycle mutations, reply moderation updates, and forum taxonomy changes.

### Errors / Failure Codes
- Command validation failures surfaced with descriptive error messages.
- Transport and server function failures reported without causing UI panic.

