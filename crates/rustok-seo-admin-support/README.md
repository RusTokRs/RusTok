# rustok-seo-admin-support

## Purpose

`rustok-seo-admin-support` provides reusable admin-side SEO widgets and transport helpers for module-owned entity editors.

## Responsibilities

- Expose reusable Leptos panels for embedding SEO authoring into owner-module admin routes.
- Keep shared SEO GraphQL transport helpers out of domain-specific admin packages.
- Provide lightweight scoring and form helpers for explicit SEO metadata editing.
- Prepare reusable observability/remediation widgets for the SEO Phase D control-plane expansion.
- Provide a shared control-plane widget state contract (`loading/ready/empty/permission_denied/error`) for owner-module SEO surfaces.
- Consume the host-provided effective locale and canonicalize it before read/write flows instead of inventing a package-local locale override.
- Preserve the ownership rule: content modules own their screens, while `rustok-seo` owns the shared capability contract.

## Interactions

- Consumed by `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, and `rustok-forum/admin`.
- Uses the shared `rustok-seo` GraphQL contract for explicit metadata reads/writes and revision publishing.
- Localizes panel chrome from the host locale and does not expose its own locale field in the SEO editor.
- Does not own tenant-toggled runtime behavior and is not itself a platform module.

## Entry points

- `SeoEntityPanel`
- `SeoCapabilityNotice`
- `SeoControlPlaneWidgets`
- `SeoControlPlaneWidgetStateCard`
- `SeoSnippetPreviewCard`
- `SeoRecommendationsCard`
- `SeoSummaryTile`
- `SeoEntityForm`
- `SeoMetaView`
- internal `transport` helpers for SEO GraphQL operations

## Current execution wave (Phase D)

The crate is aligned with SEO Phase D for:

- reusable event/delivery-status and remediation UI cards
- shared control-plane widget state contract with semantic permission/error handling
- host-locale-only owner-panel wiring for `pages/product/blog/forum`
- GraphQL/REST transport helper parity for Next and Leptos admin hosts
- verification and UX consistency checks across owner-module panels

See `docs/implementation-plan.md` for the active backlog.
