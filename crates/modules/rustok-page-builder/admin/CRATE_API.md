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
- `SsrDraftSessionStore`: Abstraction for persisting work-in-progress authoring drafts. Every `load`/`commit`/`remove` takes an optional `DraftSessionOwner` binding; an attributed record rejects every other caller with `OwnerMismatch`, while unattributed (legacy) records stay loadable and are adopted by the first attributed commit.
- `DraftSessionOwner`: `{ tenant_id, user_id }` binding stamped into every draft session record (audit §2.5).
- `InMemorySsrDraftSessionStore`: Ephemeral in-memory store for unit tests and stateless runs. Enforces `MAX_DRAFT_SESSIONS_PER_OWNER` per-owner session cap.
- `FileSsrDraftSessionStore`: Filesystem-backed draft store with optimistic generation locking. Updates run under a per-token lock file (`O_EXCL` create, stale-break after 30s, 2s wait deadline) so read-check-write sequences are atomic across processes; TTL refreshes go through compare-and-set so a load can never regress a concurrent commit.
- `DurableSsrDraftSessionStore`: Distributed KV backend driver adapter. Commits persist through `PersistentDraftSessionBackend::compare_and_save_record` (compare-and-set on the record generation); a lost race returns `GenerationConflict` and reseeds the memory cache from the winner.
- `DraftSessionKvDriver`: KV driver contract with `compare_and_set(key, expected_cas, value, cas, ttl)` (SETNX-style create when `expected_cas` is `None`); the CAS token is the record generation, not the payload bytes.
- `MAX_DRAFT_SESSION_RECORD_BYTES` (20 MiB): serialized record size cap enforced before any write.
- `MAX_DRAFT_SESSIONS_PER_OWNER` (8): per-owner live session cap enforced on create and prune.

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
- `SsrDraftSessionError`: Optimistic concurrency conflict or draft storage failure. Includes `OwnerMismatch` (opaque — never echoes user ids), `RecordTooLarge { bytes, maximum }`, and `LockTimeout` (token lock contention).
