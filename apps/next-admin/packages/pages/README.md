# @rustok/pages-admin

Module-owned admin UI of the Pages module for the classic Next admin. Following
the platform model (one GraphQL consumed by both admins; each module keeps its
own UI), this package registers the Pages navigation and provides the pages
management surface: list, create draft, unpublish, delete — all through the
`rustok-pages` GraphQL API (`pages`, `pageStarters`, `instantiatePageStarter`,
`savePageStarterFromPage`, `createPage`, `unpublishPage`, `deletePage`). Create-page
uses the selected starter's observed revision and fails visibly when the catalog or clone fails.
Managers can create a starter from the exact-locale saved working copy; unsaved
builder changes are not included. They can edit tenant starter metadata and confirm
deletion at the observed revision. Built-ins remain read-only; metadata updates
preserve document and thumbnail. Content editing is not provided here. The create
form offers exact-locale layout keys and shows whether the starter's suggested key
is available, without applying it automatically. Catalog errors stop creation; a
preflight rechecks the choice; the create mutation also requests server-side checked
layout selection (PostgreSQL/SQLite; non-default selection fails closed on MySQL).
It validates existence, not a layout revision. This integration has not yet
been verified against a running server or in a browser.

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
- Provides deep links to visual editing (`builderUrlForPage`) and, for published pages, to the live storefront page (`storefrontUrlForPage`). A page that is not published has no storefront link; its working copy is previewed in the builder.
