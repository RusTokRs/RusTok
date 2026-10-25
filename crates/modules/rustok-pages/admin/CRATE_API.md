# `rustok-pages-admin` Public API & Architecture Contract

## Minimum Contract Set

- **Module Authority**: `rustok-pages-admin` is the authoritative SSR-first Leptos admin UI package for CMS page authoring, static page management, template layout editing, and site symbol management.
- **Symmetric FFA Boundary**: Framework-agnostic logic lives strictly in `src/core.rs` with zero UI framework imports, zero template bindings, and full pure Rust unit test coverage. Leptos UI components (`PagesAdmin`, `TemplatesAdmin`) act strictly as presentation and event-dispatch adapters.
- **Visual Builder Integration**: Hosts the Fly browser intent bridge and `PageBuilderAdmin` canvas for pages formatted with `PAGE_BUILDER_DOCUMENT_FORMAT`.
- **SSR Draft Session Store**: Manages server-side draft sessions (`SsrDraftSessionStore`, `DurableSsrDraftSessionStore`) with generation conflict tracking and revision-level invalidation.
- **Deterministic Access Policy**: Role-based capability policy (`pages_editor_capability_policy`) enforcing strict fail-closed access (only `super_admin` and `admin` receive publish privileges).

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
- `PagesBrowserIntentResponse`: DTO encapsulating the dispatch result, persistence snapshot, reload flag, draft token, and runtime context.
- `PagesBrowserIntentError`: Typed domain error for builder intent failures.
- `PagesBrowserIntentProblem`: RFC-7807 problem detail projection with machine-readable codes.

### Access & Policy APIs
- `pages_editor_capability_policy`: Builds an `EditorCapabilityPolicy` from an authenticated user role and contribution assembly.
- `pages_editor_capability_policy_for_role`: Builds the default editor capability policy for a specific role string.
- `pages_editor_permissions_for_role`: Maps a role to `CapabilityState` (`super_admin`/`admin` -> full, `manager` -> author without publish, others -> read-only).
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
- **Draft Session Scoping**: SSR draft sessions are bound to a specific `page_id` and verified against the page's persisted revision. Revision mismatches automatically prune the stale session and reseed from persisted state.
- **Symbol Referential Integrity**: Reusable site symbols cannot be deleted without warning if referenced by any template or page.
- **Unique Component Identity**: Every component instantiated from a starter preset generates fresh, non-colliding unique IDs.

### Events / Outbox Side Effects

- **UI Event Propagation**: `PagesAdmin` and `TemplatesAdmin` signal state mutations to parent contexts via `refresh_generation` signal updates.
- **Visual Builder Bridge**: Intent execution propagates change effects (`save`, `reload`, `client_actions`) through `BrowserIntentDispatchResult` and `PagesBrowserIntentResponse`.
- **Outbox Boundary**: Page mutations (create, update, publish, rollback) trigger backend domain events and outbox messages exclusively on the server through the `PageService` boundary; this admin package does not construct raw domain outbox records.

### Errors / Failure Codes

- `PagesBrowserIntentError::PageNotFound`: HTTP 404 — The targeted page does not exist.
- `PagesBrowserIntentError::PageMismatch`: HTTP 400 — Intent page ID does not match active session snapshot.
- `PagesBrowserIntentError::Draft(SsrDraftSessionError::GenerationConflict)`: HTTP 409 (`DRAFT_GENERATION_CONFLICT`) — Concurrent edit collision.
- `PagesBrowserIntentError::Draft(SsrDraftSessionError::RevisionConflict)`: HTTP 409 (`REVISION_CONFLICT`) — Page persisted revision changed mid-flight.
- `PagesBrowserIntentError::Capability(BrowserCapabilityAccessError::Denied)`: HTTP 403 (`CAPABILITY_DENIAL`) — Role or provider state forbids the requested action.
- `PagesBrowserIntentError::Dispatch(BrowserIntentDispatchError)`: HTTP 422 — Unprocessable visual builder action payload.
- `TemplateValidationError`: Typed validation errors for template keys (`EmptyKey`, `InvalidCharacters`, `KeyTooLong`).
- `SymbolValidationError`: Typed validation errors for symbol IDs and symbol JSON structures (`EmptyId`, `IdTooLong`, `InvalidCharacters`, `InvalidJson`, `RootNotObject`).
