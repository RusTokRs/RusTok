# @rustok/pages-admin

Module-owned admin UI of the Pages module for the classic Next admin. Following
the platform model (one GraphQL consumed by both admins; each module keeps its
own UI), this package registers the Pages navigation and provides the pages
management surface: list, create draft, unpublish, delete — all through the
`rustok-pages` GraphQL API (`pages`, `createPage`, `unpublishPage`,
`deletePage`).

The visual page builder itself is **not** reimplemented here. The builder UI is
module-owned UI of the Rust FFA admin (`rustok-pages-admin`, Leptos connector);
this package links to it per page ("Open in builder", configured with
`NEXT_PUBLIC_RUST_ADMIN_URL` on the host). Publishing is also deliberately absent
from this surface: publish uses the builder's reviewed runtime flow, and the
GraphQL `createPage` mutation rejects `publish: true` by design.

Host wiring (already done in `apps/next-admin`):

- `package.json`: `"@rustok/pages-admin": "file:./packages/pages"`
- `src/modules/index.ts`: `import '@rustok/pages-admin';` (registers nav)
- `src/app/dashboard/pages/page.tsx`: renders `PagesPage` with the session
  token/tenant, Rust admin origin (`builderBaseUrl`), and Storefront origin (`storefrontBaseUrl`).
- Provides deep links to visual editing (`builderUrlForPage`) and live storefront viewing / draft mode preview (`storefrontUrlForPage`).

