# rustok-pages-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md) |
> [Public API Contract](CRATE_API.md)

## Purpose

`rustok-pages-admin` publishes the Leptos admin root page for the `rustok-pages` module.

## Responsibilities

- Export the module-owned `PagesAdmin` root component for `apps/admin`.
- Keep pages-specific admin UI inside the module boundary instead of `apps/admin`.
- Act as the canonical working admin vertical slice for module-owned page CRUD.
- Expose contract-safe page-builder capability surfaces (`preview/tree/properties/publish`) on top of the vendor-neutral `grapesjs` backend payload.
- Keep write-path error handling consistent (`validation/sanitize/runtime`) for page-builder flows.
- Host the owner-side page SEO panel through `rustok-seo-panel` instead of delegating page metadata editing to `rustok-seo-admin`.

## Interactions

- Used by `apps/admin` through manifest-driven generated wiring.
- Uses the pages module GraphQL contract for list/create/edit/update/publish/delete flows.
- Writes the visual builder payload into the sole `body.document` field; Pages
  selects the Page Builder format server-side.
- Opens the editor and the revision history only for a page whose document the
  principal may read (`documentReadable`). Otherwise the workspace shows
  "Document not available", and builder requests for that page answer
  `403 DOCUMENT_UNAVAILABLE`; the starter document is never shown in its place.
  The notice is localized (`pages-document-*` in `locales/`); the text of the
  `403` problem is the English `DOCUMENT_UNAVAILABLE_MESSAGE`, like other
  problem texts of the endpoint.
- Offers "Restore" in the revision history only to a principal whose permission
  snapshot grants `pages:update` (`access::pages_can_restore_body_revision`).
  The history itself is readable with `pages:publish` as well. The server
  remains the authority and also applies the ownership scope of the grant.
- Uses the shared `rustok-seo` GraphQL contract through `rustok-seo-panel` for explicit page SEO authoring.
- Follows the generic host route contract `/modules/:module_slug`.

## Entry points

- `PagesAdmin`

## Known Limitations / Pending Implementation

- **A failed draft save is not retried on its own.** When the save that follows an applied edit is
  rejected, the browser intent still succeeds: the response carries `save_failure` and asks for a
  reload, and the draft stays marked as unsaved. The builder saves again on an explicit save or on
  the next edit that changes the document. An intent that does not change the document (selecting,
  switching panels) does not retry, so after a transient outage the draft stays unsaved until one
  of those happens.
- **The save-failure message ages out.** It stays on screen, and across reloads, until a save
  succeeds, but the browser keeps it for at most 10 minutes; after that only the toolbar's unsaved
  state says that the draft is not stored.
