# Crate API: rustok-page-builder-admin

## Public surface
### Presentation Shell & UI
- `PageBuilderAdmin`: Primary Leptos visual page builder presentation shell.
- `PageBuilderAdminHostContext`: Context container for canvas controller and admin facade injection.
- `PageBuilderAdminWithController`: Pre-wired component binding a specific `AdminCanvasController`.
- `ConsumerPropertiesPanel`: Property inspector component for editing consumer properties.

### Canvas & Controller
- `AdminCanvasController`: Manages iframe canvas lifecycle, selection, hover, and overlay geometry.
- `AdminCanvasEffect`: Side-effects produced by canvas interactions (geometry updates, postMessage framing).
- `AdminCanvasError`: Typed error representing canvas interaction failures.

### Intent & Capability Access
- `dispatch_browser_intent`: Validates and routes browser editing intents to controller actions.
- `dispatch_browser_intent_with_palette_access`: Verifies block permission before intent execution.
- `validate_browser_capability_access`: Validates operator capabilities against page builder policies.
- `validate_browser_palette_access`: Filters available palette items based on active permissions.

### Draft Sessions & Persistence
- `SsrDraftSessionStore`: Abstraction for persisting work-in-progress authoring drafts.
- `InMemorySsrDraftSessionStore`: Ephemeral in-memory store for unit tests and stateless runs.
- `FileSsrDraftSessionStore`: Filesystem-backed draft store with optimistic generation locking.
- `DurableSsrDraftSessionStore`: Distributed KV backend driver adapter.

### Asset & Consumer Properties
- `AssetProviderPort`: Extension port for asset library search and upload.
- `ConsumerPropertyEditorPort`: Extension port for host document property customization.
- `PageBuilderContributionHostContext`: Host context providing contribution registry extensions.

## Platform Capability Bindings
- **`fly`**: Core visual rendering and layout data structures.
- **`fly-ui`**: Presentation primitives and action state machine.
- **`fly-leptos`**: Leptos-specific canvas adapters and reactive signals.
- **`rustok-ui-core`**: Design token utilities and text normalization.
- **`rustok-ui-i18n`**: Localized Fluent catalog lookups for authoring tools.
- **`rustok-grid` / `rustok-grid-leptos`**: Paginated data grids for asset libraries and property lists.

## Domain Collaborations
- **`rustok-page-builder`**: Consumes core document format, validation rules, and landing materializers.
- **`rustok-pages`**: Authoritative persistence target for published CMS pages.
- **`rustok-media`**: Asset provider integration for media library browsing and uploads.

## Minimum Contract Set
### Input DTOs/Commands
- `EditorSessionAction`: Actions dispatched to the authoring state machine (selection, drop, move, resize).
- `SaveConsumerPropertiesInput`: Request payload for updating host-specific custom property values.
- `PageBuilderContributionPreviewRequest`: Envelope for requesting block contribution previews.

### Domain Invariants
- **Fluid Frontend Architecture (FFA)**: Strict separation between pure Rust canvas controllers, property view-models, and Leptos UI rendering components.
- **Sandboxed Execution**: Authoring canvas executes in an isolated iframe with CSP restrictions and strict postMessage protocol sequence verification.
- **Deterministic Draft Generation**: Optimistic concurrency tokens prevent silent parallel overwrites across authoring sessions.
- **Non-Replay Transport**: Commands and intent dispatches require valid instance sequence identifiers; replay attempts are rejected.

### Events / Outbox Side Effects
- Draft session modifications are scoped to local/distributed KV storage and do not emit public domain outbox events until published by host services.

### Errors / Failure Codes
- `AdminCanvasError`: Frame communication or iframe lifecycle failure.
- `BrowserCapabilityAccessError`: Operator lacks required permissions for the requested action.
- `BrowserIntentDispatchError`: Malformed or invalid editing intent dispatched to canvas.
- `ConsumerPropertyEditorError`: Validation failure in consumer properties payload.
- `SsrDraftSessionError`: Optimistic concurrency conflict or draft storage failure.
