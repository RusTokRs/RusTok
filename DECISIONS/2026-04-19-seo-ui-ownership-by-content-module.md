# SEO UI ownership by content modules

- Date: 2026-04-19
- Status: Accepted

## Context

`rustok-seo` has already been assembled as a single tenant-aware runtime for metadata precedence, canonical routing,
redirects, sitemap/robots, and storefront `SeoPageContext`. At the same time, in the current state,
`rustok-seo-admin` contains a central metadata editor for `page`, `product` and `blog_post`.

Such a central editor conflicts with the basic module-owned UI contract of the platform:

- the entity screen must be owned by its owner module;
- the host only mounts the surface;
- a cross-cutting capability must not become the owner of another domain's UI;
- new modules must be able to integrate SEO into their own UI, rather than going
  through a separate universal hub.

This is especially important for `pages`, `product`, `blog`, `forum`, and any future content-domain modules,
where SEO controls, diagnostics, and completion scoring should live alongside the main entity editor UI.

## Decision

The following ownership contract is fixed:

1. `rustok-seo` remains the sole backend/runtime owner for SEO capability:
   metadata storage, precedence, canonical routing, redirects, sitemap/robots, diagnostics,
   storefront/headless read contracts.
2. Entity-specific SEO authoring UI belongs to the owner modules:
   `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`
   and any future content modules.
3. `rustok-seo-admin` retains only SEO-owned infrastructure UI:
   redirects, robots policy, sitemap generation/status, global defaults, shared diagnostics overview,
   and similar cross-cutting controls.
4. `rustok-seo` or a support layer next to it may ship shared SEO widgets/helpers for
   owner modules, but these widgets do not make `rustok-seo-admin` the owner of a domain screen.
5. The current central metadata editor in `rustok-seo-admin` is considered a transitional surface and must
   be removed after cutover of entity SEO authoring to owner modules.

## Consequences

- The module-owned UI contract becomes consistent with the SEO capability model:
  where the domain entity is, there its SEO editor is.
- `rustok-seo-admin` narrows down to a real cross-cutting control plane instead of a universal editor.
- For `pages`, `product`, `blog`, `forum`, owner-side integration is required on top of shared SEO
  contracts and reusable widgets.
- The `rustok-seo` plan must record the migration path: shared widgets/support layer, cutover
  of metadata editor to owner modules, then cleanup of the transitional central editor.
