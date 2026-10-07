# rustok-pages-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

## Purpose

`rustok-pages-admin` publishes the Leptos admin root page for the `rustok-pages` module.

## Responsibilities

- Export the module-owned `PagesAdmin` root component for `apps/admin`.
- Keep pages-specific admin UI inside the module boundary instead of `apps/admin`.
- Act as the canonical working admin vertical slice for module-owned page CRUD.
- Expose contract-safe page-builder capability surfaces (`preview/tree/properties/publish`) on top of the vendor-neutral `grapesjs` backend payload.
- Keep write-path error handling consistent (`validation/sanitize/runtime`) for page-builder flows.
- Host the owner-side page SEO panel through `rustok-seo-panel` instead of delegating page metadata editing to `rustok-seo-admin`.

## Authoring behaviour

- **Page list** is server-paginated (25 per page) with a title/slug search and a status filter
  (`ListGqlPagesFilter.search/status/sort/page/perPage`). Search is case-insensitive on
  PostgreSQL for every script; SQLite `lower()` folds ASCII only.
- **Slugs** come from `rustok_page_builder::normalize_page_slug`, the same Unicode rules the server
  enforces (`О компании` → `о-компании`).
- **New pages** start from `rustok_page_builder::starter_page_document`, which passes the static
  publish policy, so a fresh page can be published without manual repairs.
- **Roles** (`pages_lifecycle_permissions_for_role`, mirroring `rustok-core` RBAC): managers create,
  save and delete drafts; publishing and unpublishing require `admin`/`super_admin`. The builder
  `Publish` capability persists the draft and is authorized with `pages:update`
  (`PageBuilderCapabilityPermissions::draft_persistence()`).
- **Persistence**: the editor autosaves 2.5 s after the last edit (never retrying a failed save),
  warns before leaving with unsaved changes, and mirrors its status into the header, where Publish
  stays disabled until the canvas is saved. Saving no longer remounts the editor.
- **Destructive actions** (Unpublish, Delete) require an explicit second confirmation.
- **Static pages** publish without a promoted runtime scenario baseline (static default runtime);
  pages that read runtime context still require a promoted baseline, enforced on the server too.

## Interactions

- Used by `apps/admin` through manifest-driven generated wiring.
- Uses the pages module GraphQL contract for list/create/edit/update/publish/delete flows.
- Writes the visual builder payload into the sole `body.document` field; Pages
  selects the Page Builder format server-side.
- Uses the shared `rustok-seo` GraphQL contract through `rustok-seo-panel` for explicit page SEO authoring.
- Follows the generic host route contract `/modules/:module_slug`.

## Entry points

- `PagesAdmin`
