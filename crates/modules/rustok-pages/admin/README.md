# rustok-pages-admin

Create-page reads the Pages GraphQL starter catalog and instantiates the selected
built-in or tenant starter at its observed revision. Catalog, clone and revision
errors stop creation; no blank fallback is substituted. Page titles are metadata:
example text in a preset is editable in the builder and is not rewritten. The
starter's suggested layout is shown with exact-locale availability but is not applied
automatically. Layout catalog errors block creation; the selected key is rechecked
before create; the create mutation also requests server-side checked layout selection
(PostgreSQL/SQLite; non-default selection fails closed on MySQL). It validates
existence, not a layout revision. The page workspace can
create a new starter from a saved exact-locale working body and displays its revision;
unsaved editor changes are not included. Managers can edit tenant starter metadata
and confirm deletion at the observed revision; built-ins remain read-only, and the
starter document and thumbnail are not changed by metadata edits. Content editing
is not provided by this catalog UI. The integration has not yet been compiled or
verified against a running server.

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
- **Stale-draft recovery is manual and unverified.** A mismatched saved revision no longer
  deletes or rebases the existing session: the route restores it, marks a conflict and exposes
  the entire project as copyable JSON. Copy it before the session expires or is evicted, then open
  the saved page in a private window at an address without `fly_draft` and merge manually. There
  is no automatic merge/import or permanent backup. The new paths and store concurrency have not
  been compiled or integration-tested here; verify all supported stores before release.
- **The shared active-page preview projection is not yet verified.** Both preview entrypoints now
  propagate a missing active page or encoding failure instead of substituting an empty project.
  Compilation and behavioral verification remain pending.
- **A draft that cannot be loaded is reported, not recovered.** A missing, expired or refused
  route draft stops the editor instead of opening a different document under that token. The intent
  endpoint returns `DRAFT_UNAVAILABLE` for a missing token, and preview fails closed too. The error is not logged (no `tracing`
  dependency); reopening the saved page requires an address without `fly_draft` in a separate
  private window to avoid the existing tab's stored session token. When the saved-page render
  finds a cached draft, the browser now stops editing and offers a route to restore it; first
  requests use the generation of the SSR-rendered snapshot. Page changes clear the previous
  route token in the same query update without deleting its session. Store outage/eviction
  recovery and browser/server integration still need verification. A request aborted after the
  server committed can lose its response token; simultaneous adapters for one page can also
  replace the single browser storage pointer. This is not durable draft discovery.
- **Server document CAS is not a complete multi-writer guarantee.** Timestamp revision reuse and
  the post-commit response read can associate an editor's project with another writer's revision;
  see `rustok-pages/README.md`. Resolve and test this before relying on stale-draft handling to
  prevent lost writes.
- **The save-failure message ages out.** It stays on screen, and across reloads, until a save
  succeeds, but the browser keeps it for at most 10 minutes; after that only the toolbar's unsaved
  state says that the draft is not stored.
- **The facade save path carries no draft owner, by design.** `PagesBuilderFacade` is built with
  `PagesBuilderSaveSnapshot { owner: None, .. }` (`src/composition.rs`). Attribution belongs to the
  intent endpoint: `dispatch_pages_browser_intent` takes the owner from the verified caller, and
  that is the only path that creates or commits a draft session. The facade's snapshot
  serves the capability and preview calls, which read no session. The editor's route draft is
  loaded separately with an owner built from the current user and the tenant
  (`load_route_draft(..., draft_owner.as_ref())`), so a session that belongs to another caller is
  not restored. Do not thread the facade's snapshot owner into the store: it would add a second,
  unverified attribution without changing which sessions can be read or committed.
