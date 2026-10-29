# Crate API: rustok-blog-admin

## Public surface

### UI Components
- `BlogAdmin`: Composed root admin page containing post authoring workspace and comment moderation panel.

### Page Builder Contributions
- `BlogContributionAdapter`, `BlogWidgetOwnerSchemaRef`, `BlogWidgetPropertyEditorModel`, `BlogWidgetRenderModel`: Widget integration types for the Fly page builder.
- `blog_contribution_manifest`, `blog_fly_registry_set`, `blog_full_admin_contribution_policy`, `blog_widget_contribution`, `blog_widget_preview_contribution`, `build_blog_admin_contribution_registry`, `register_blog_fly_widgets`: Contribution registry functions.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-blog`.
- Integrates with the Fly page builder framework (`fly`, `fly-ui`).
- Dual-transport support: native Leptos server functions in SSR/hydrate, GraphQL in CSR/headless.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).
- Post SEO authoring via `rustok-seo-panel` and `rustok-seo-targets`.

## Domain Collaborations
- Collaborates with `rustok-blog` backend services for post CRUD and category assignments.
- Coordinates with `rustok-comments` for thread comment moderation within blog posts.
- Coordinates with `rustok-outbox` for post publication and status update events.

## Minimum Contract Set

### Input DTOs/Commands
- `BlogAdmin`: Entrypoint workspace component mounting blog administration.
- Post draft creation, updating, scheduled publishing, and comment moderation commands.

### Domain Invariants
- Posts maintain slug uniqueness within channel and tenant boundaries.
- Zero business logic inside UI view templates; validation and view mapping reside in `core`.
- Comment moderation changes preserve audit trails and permissions.

### Events / Outbox Side Effects
- Emits post lifecycle state transitions and moderation actions.

### Errors / Failure Codes
- Validation failures reported cleanly in UI without crashing views.
- Transport failures mapped to accessible user feedback alerts.

