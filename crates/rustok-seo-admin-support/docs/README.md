# `rustok-seo-admin-support` Documentation

`rustok-seo-admin-support` — support crate for owner-module admin UI that provides reusable SEO widgets and transport helpers without moving screen ownership to `rustok-seo-admin`.

## Purpose

- provide content modules with a common SEO UI/tooling layer for entity-owned editor surfaces;
- maintain shared GraphQL helpers for `seoMeta`, `upsertSeoMeta`, `publishSeoRevision`;
- publish a unified `SeoEntityPanel` and lightweight capability notices for owner-side admin screens.

## Scope

- reusable Leptos panel for explicit SEO metadata authoring;
- simple completeness scoring and form/view-model helpers;
- canonical host-locale consumption without package-local locale input: the panel takes the effective locale from the owner-module context,
  canonicalizes BCP47-like tags and does not invent its own fallback chain;
- reusable diagnostics/widgets layer: snippet preview, recommendations card, delivery/remediation cards and state notice
  can be reused in owner-module layouts without reverting to a central SEO hub;
- shared control-plane widget state contract (`loading/ready/empty/permission_denied/error`) for unified
  loading/error/permission/empty states of SEO control-plane widgets;
- owner-module integration seam between `rustok-seo` runtime and `pages/product/blog/forum` admin packages.

## Integration

- used by `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`;
- owner-side panel chrome is localized from the host locale and no longer holds an editable locale field inside the SEO panel;
- does not own its own runtime, tenant settings, RBAC policy or central SEO route.

## Phase D alignment

The support crate is synchronized with SEO Phase D along the following directions:

- reusable observability/remediation widgets for owner-module SEO panels;
- transport fallback parity (GraphQL/REST) for Leptos and Next admin hosts;
- verification/UX consistency matrix for the shared panel surface.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [`rustok-seo` documentation](../../rustok-seo/docs/README.md)
