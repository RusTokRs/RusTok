# `rustok-pages-admin` Public API & Architecture Contract

## Minimum Contract Set

- **Module Authority**: `rustok-pages-admin` is the authoritative SSR-first Leptos admin UI package for CMS page authoring, static page management, template layout editing, and site symbol management.
- **Symmetric FFA Boundary**: Framework-agnostic logic lives strictly in `src/core.rs` with zero UI framework imports, zero template bindings, and full pure Rust unit test coverage. Leptos UI components (`PagesAdmin`, `TemplatesAdmin`) act strictly as presentation and event-dispatch adapters.
- **Visual Builder Integration**: Hosts the Fly browser intent bridge and `PageBuilderAdmin` canvas for pages formatted with `PAGE_BUILDER_DOCUMENT_FORMAT`.
- **SSR Draft Session Store**: Manages server-side draft sessions (`SsrDraftSessionStore`, `DurableSsrDraftSessionStore`) with generation conflict tracking and revision-level invalidation. Sessions are bound to a `DraftSessionOwner` (`tenant_id` + `user_id` from the verified caller); read-only callers (no edit capability) never create or mutate a session, and records above `MAX_DRAFT_SESSION_RECORD_BYTES` or owners above `MAX_DRAFT_SESSIONS_PER_OWNER` live sessions are rejected/evicted (audit §2.5). The default file tier stores drafts per tenant under `RUSTOK_DRAFT_SESSION_DIR` (see `rustok-page-builder-admin`).
- **Save Failure Contract**: An intent whose edit was applied but whose save was rejected (by the Pages service, or because the autosave was blocked by validation diagnostics) is answered with HTTP 200, not an error: the edit stays in the draft session marked as unsaved, `PagesBrowserIntentResponse::save_failure` carries the problem (`status`, `code` — the consumer's stable code such as `REVISION_CONFLICT`, otherwise `SAVE_FAILED` — `error`, `intent: "save"`), and `reload` is `true` so the client re-renders from the server. The failed save is retried by the next intent that changes the document or by an explicit `save`; intents that leave the document untouched (`select`, `hover`) do not repeat it, so a persistent failure is reported once per change rather than on every click; their response state still says `dirty: true`, which is what keeps the failure message on screen in the browser adapter until a save succeeds. A dispatch executes at most one consumer request; an intent that produces more fails with `PagesBrowserIntentError::UnsupportedRequestBatch` before anything is executed.
- **Deterministic Access Policy**: Effective-permission capability policy (`pages_editor_capability_policy`) enforcing strict fail-closed access (`pages:manage` grants everything, `pages:update` grants authoring, `pages:publish` grants publishing; snapshots without Pages grants stay read-only).

## Platform Capability Bindings

- **`fly` / `fly_ui` / `fly_browser`**: Browser intent protocol, canvas controller, CSRF token validation (`derive_csrf_token`, `verify_csrf_token`), contribution manifests, and capability state resolution.
- **`rustok-page-builder` & `rustok-page-builder-admin`**: `PageBuilderAdmin`, `ConsumerPropertyEditorRuntime`, `SsrDraftSessionStore`, `DurableSsrDraftSessionStore`, and document format constants.
- **`rustok-ui-core`**: `AdminQueryKey::PageId`, `UiRouteContext`, `status_badge_class`, `parse_ui_csv`, `normalize_ui_text`.
- **`leptos-auth`**: Session tokens (`use_token`), tenant context (`use_tenant`), and authenticated user claims (`use_current_user`).

## Domain Collaborations

- **`rustok-pages`**: Collaborates with the backend CMS service for page CRUD, page revisions, scheduled publication, page templates, and reusable site symbols.
- **`rustok-media-admin`**: Asset selector integration for page image/media attachments.
- **`rustok-seo`**: Dedicated page SEO authoring integration hosted through the module boundary.

## Public surface

### Components
- `PagesAdmin`: Primary Leptos admin page component for listing, creating, and editing CMS static pages.
- `TemplatesAdmin`: Leptos admin page component for managing layout templates and reusable site symbols.

### Intent Dispatch & Session APIs
- `dispatch_pages_browser_intent`: High-level entry point for processing visual builder actions via the default draft store.
- `dispatch_pages_browser_intent_with_store`: Processes visual builder actions against an explicit `SsrDraftSessionStore`.
- `dispatch_pages_browser_intent_with_capabilities`: Dispatches intents with active editor capability constraints.
- `dispatch_pages_browser_intent_with_store_and_capabilities`: Dispatches intents with explicit store and capabilities.
- `pages_browser_draft_store`: Resolves the active SSR draft session store (custom or default durable store).
- `set_pages_browser_draft_store`: Host boot hook for registering distributed or custom draft session stores.
- `pages_browser_default_durable_store`: Default in-memory durable draft session store.
- `PagesBrowserIntentResponse`: DTO encapsulating the dispatch result, persistence snapshot, optional `save_failure` problem, reload flag, draft token, and runtime context.
- `PagesBrowserIntentError`: Typed domain error for builder intent failures.
- `PagesBrowserIntentProblem`: RFC-7807 problem detail projection with machine-readable codes.

### Access & Policy APIs
- `pages_editor_capability_policy`: Builds an `EditorCapabilityPolicy` from the authenticated user's effective permission snapshot and contribution assembly.
- `pages_editor_capability_policy_from_effective`: Builds the default editor capability policy for an effective permission snapshot.
- `pages_editor_permissions_from_effective`: Maps an effective permission snapshot to `CapabilityState` (`pages:manage` -> full, `pages:update` -> author without publish, `pages:update` + `pages:publish` -> full, `pages:publish` alone -> read-only plus publish, anything else -> read-only).
- `effective_permissions_grant`: Checks whether an effective snapshot grants a required `"{resource}:{action}"` permission (exact match or the resource's `manage` grant), mirroring `rustok_api::has_effective_permission`.
- `pages_editor_provider_state`: Derives `EditorProviderState` (`Healthy`, `Degraded`, `Unavailable`) from contribution diagnostics.
- `pages_palette_block_access`: Verifies whether a given block contribution can be inserted under active capabilities.

### Contributions & Starters
- `build_pages_admin_contribution_registry`: Assembles module contribution descriptors into a verified registry.
- `pages_admin_contribution_policy`: Declares the contribution assembly policy for Pages admin.
- `pages_contribution_manifest`: Retrieves build-generated contribution metadata verified against `rustok-module.toml`.
- `pages_landing_blocks_contribution`: Contribution descriptor for landing blocks.
- `pages_metadata_contribution`: Contribution descriptor for page metadata properties.
- `pages_metadata_property_schema`: Validated JSON schema for page metadata properties.
- `page_starters`: Returns the catalog of predefined page starter presets (`blank`, `saas_landing`, `ecommerce_showcase`, `lead_capture`, `link_in_bio`).
- `PageStarterDescriptor`: Metadata describing a page starter template.
- `instantiate_starter_preset`: Instantiates a starter preset with fresh component UUIDs and title.
- `instantiate_starter_preset_pretty`: Formats the instantiated starter preset as pretty JSON.

### Input DTOs/Commands

- `PageDraftFormInput<'a>`: Form input for creating a page draft (`locale`, `title`, `slug`, `channel_slugs`, `starter_id`, `template`).
- `CreatePageDraft`: Normalized draft creation command with parsed channels, slug, and project document.
- `EditFormSeed`: Seed data for page edit forms with title and formatted project JSON.
- `PageMetadataPatch`: Command DTO for updating page metadata without document recompilation.
- `BrowserIntentEnvelope`: Standardized envelope for browser visual builder actions.

### Domain Invariants

- **Zero UI Framework Leakage in Core**: `src/core.rs` contains zero imports from `leptos`, `dioxus`, or any UI rendering framework.
- **Role Fail-Closed Security**: Unauthenticated, customer, or unrecognized roles receive strictly read-only capabilities (`CapabilityState::read_only()`). Only `super_admin` and `admin` receive publication capabilities.
- **Draft Session Scoping**: SSR draft sessions are bound to a specific `page_id` and verified against the page's persisted revision. Revision mismatches automatically prune the stale session and reseed from persisted state. Sessions are also owner-bound (`DraftSessionOwner`): attributed records reject loads, commits, and removals from any other user or tenant with `OwnerMismatch`, and callers without the edit capability never create or mutate a session at all.
- **No Stand-In Document**: The starter document is opened only for a page whose document the principal may read and which has none yet. When `PageDetail.document_readable` is `false`, the workspace shows "Document not available", the browser intent endpoint answers `DOCUMENT_UNAVAILABLE`, and `PagesPageBuilderProjectStore::load_project` and `save_project` fail with `Forbidden` before the document is loaded or a revision is compared (one gate, `ensure_document_readable`).
- **Restore Needs The Save Grant**: The revision history offers "Restore" only when the permission snapshot of the signed-in principal grants `pages:update` (`pages_can_restore_body_revision`); a principal that reads the history with `pages:publish` alone is not offered it. An empty snapshot hides the action.
- **Symbol Referential Integrity**: Reusable site symbols cannot be deleted without warning if referenced by any template or page.
- **Unique Component Identity**: Every component instantiated from a starter preset generates fresh, non-colliding unique IDs.

### Events / Outbox Side Effects

- **UI Event Propagation**: `PagesAdmin` and `TemplatesAdmin` signal state mutations to parent contexts via `refresh_generation` signal updates.
- **Visual Builder Bridge**: Intent execution propagates change effects (`save`, `reload`, `client_actions`) through `BrowserIntentDispatchResult` and `PagesBrowserIntentResponse`.
- **Outbox Boundary**: Page mutations (create, update, publish, rollback) trigger backend domain events and outbox messages exclusively on the server through the `PageService` boundary; this admin package does not construct raw domain outbox records.

### Errors / Failure Codes

- `PagesBrowserIntentError::PageNotFound`: HTTP 404 — The targeted page does not exist.
- `PagesBrowserIntentError::DocumentUnavailable`: HTTP 403 (`DOCUMENT_UNAVAILABLE`) — The page exists, but its authoring document is withheld from the principal (`documentReadable: false`); no controller is seeded.
- `PagesBrowserIntentError::PageMismatch`: HTTP 400 — Intent page ID does not match active session snapshot.
- `PagesBrowserIntentError::Draft(SsrDraftSessionError::GenerationConflict)`: HTTP 409 (`DRAFT_GENERATION_CONFLICT`) — Concurrent edit collision.
- `PagesBrowserIntentError::Draft(SsrDraftSessionError::RevisionConflict)`: HTTP 409 (`REVISION_CONFLICT`) — Page persisted revision changed mid-flight.
- `PagesBrowserIntentError::Capability(BrowserCapabilityAccessError::Denied)`: HTTP 403 (`CAPABILITY_DENIAL`) — Role or provider state forbids the requested action.
- `PagesBrowserIntentError::Dispatch(BrowserIntentDispatchError)`: HTTP 422 — Unprocessable visual builder action payload.
- `TemplateValidationError`: Typed validation errors for template keys (`EmptyKey`, `InvalidCharacters`, `KeyTooLong`).
- `SymbolValidationError`: Typed validation errors for symbol IDs and symbol JSON structures (`EmptyId`, `IdTooLong`, `InvalidCharacters`, `InvalidJson`, `RootNotObject`).
