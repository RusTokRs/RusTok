# rustok-seo-admin-support implementation plan

## Current state

`rustok-seo-admin-support` provides reusable owner-side Leptos SEO panels,
form/view-model helpers, remediation widgets, and entity metadata GraphQL
operations for pages, products, blog, and forum. It consumes the host effective
locale and does not own a central SEO screen, tenant runtime, storage, or an
independent locale fallback chain.

## Boundary

- Owner entity screens remain in `pages`, `product`, `blog`, and `forum`.
- Shared entity metadata uses `seoMeta`, `upsertSeoMeta`, and
  `publishSeoRevision` GraphQL helpers in this crate.
- SEO control-plane REST parity for diagnostics, sitemap, bulk, and replay is
  owned by `rustok-seo` admin/Next paths; duplicating that heavy client in this
  support crate requires an explicit new consumer case.

## Next results

1. **Lock the support-versus-control-plane transport decision.** Document and
   test which calls stay entity-panel GraphQL helpers and which remain owner
   control-plane REST/GraphQL operations. Done when a new widget cannot add a
   duplicate REST client or change selected transport without a named owner.
2. **Execute reusable owner-layout coverage.** Add integration or browser
   smoke coverage for loading, empty, permission-denied, semantic-error,
   remediation, and host-locale behavior in pages, product, blog, and forum
   panels. Done when each owner package proves the shared widget contract
   without moving its screen into an SEO hub.
3. **Publish reusable-widget acceptance rules.** Define the Definition of Done
   for panel/widget additions: owner applicability, host locale, permission and
   error state, transport ownership, accessibility, and docs/examples. Done
   when a support change has a repeatable review and test checklist.

## Verification

- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- Targeted checks for `rustok-pages-admin`, `rustok-product-admin`,
  `rustok-blog-admin`, and `rustok-forum-admin`.
- Next admin lint/typecheck when a shared contract affects that host.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [SEO module plan](../../rustok-seo/docs/implementation-plan.md)
