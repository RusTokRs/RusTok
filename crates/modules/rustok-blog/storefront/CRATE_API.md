# Crate API: rustok-blog-storefront

## Public surface

### UI Components
- `BlogView`: Root storefront component rendering published blog posts and post details.
- `BlogCommentComposer`: Storefront comment composer island component.

### Feed Transports
- `render_rss_feed`: Renders native RSS 2.0 feed xml for published posts.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) storefront package for `rustok-blog`.
- Dual-path read access: native server functions in SSR/hydrate, GraphQL queries in CSR/headless.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).
- Comments integration via `rustok-comments-storefront-support`.
- Search integration via `rustok-search-storefront`.

## Domain Collaborations
- Collaborates with `rustok-blog` domain services for post catalog queries.
- Coordinates with `rustok-comments` for loading approved post comments.
- Integrates with `rustok-channel` and `rustok-tenant` for visibility and tenancy context.

## Minimum Contract Set

### Input DTOs/Commands
- `BlogView`: Entrypoint storefront view mounting blog listings or post detail view.
- Comment submission command dispatched to Comments capability.

### Domain Invariants
- Only published posts visible to the current channel are rendered.
- Comment moderation status strictly respected: only approved comments are displayed.
- Zero business logic inside UI view templates; pagination, routing, and view mapping live in `core`.

### Events / Outbox Side Effects
- Emits comment creation requests bound to specific blog post identities.

### Errors / Failure Codes
- Missing or unpublished posts surface safe 404 or empty states without throwing panics.
- Transport failures display accessible error alerts.

