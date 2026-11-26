# rustok-blog-ports public API

- `BlogPublishedPostReadPort` defines the owner read boundary.
- `ReadBlogPublishedPost` carries the slug and optional fallback locale.
- `BlogPublishedPost` carries the owner-admitted public detail and required
  publication timestamp.
- `published_post_read_scope` verifies the service claim, tenant UUID, slug
  bound and deadline. The adapter must authenticate the service and bind tenant
  and channel authority before supplying the `PortContext`.

The embedded Blog owner enforces visibility and returns typed `PortError`
failures. An absent result covers unknown, hidden and unpublished posts.
The [owner plan](../docs/implementation-plan.md#first-standalone-blog-capability)
tracks transport evidence and service deployment.
