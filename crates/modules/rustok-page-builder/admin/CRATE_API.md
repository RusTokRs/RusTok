# Crate API: rustok-page-builder-admin

## Public surface
### Presentation Shell & UI
- `PageBuilderAdmin`: Primary Leptos visual page builder presentation shell.
- `PageBuilderAdminHostContext`: Context container for canvas controller and admin facade injection.
- `PageBuilderAdminWithController`: Pre-wired component binding a specific `AdminCanvasController`.
- `ConsumerPropertiesPanel`: Property inspector component for editing consumer properties.

### Canvas & Controller
- `AdminCanvasController`: Manages iframe canvas lifecycle, selection, hover, and overlay geometry. `active_page_project` produces the one-page project used by both server-preview entrypoints without changing editor state; a missing active page or encoding failure is returned as a `FlyError`, not an empty document.
- `AdminCanvasEffect`: Side-effects produced by canvas interactions (geometry updates, postMessage framing).
- `AdminCanvasError`: Typed error representing canvas interaction failures.

### Intent & Capability Access
- `dispatch_browser_intent`: Validates and routes browser editing intents to controller actions.
- `dispatch_browser_intent_with_palette_access`: Verifies block permission before intent execution.
- `validate_browser_capability_access`: Validates operator capabilities against page builder policies.
- `validate_browser_palette_access`: Filters available palette items based on active permissions.

### Draft Sessions & Persistence
- `SsrDraftSessionStore`: Abstraction for persisting work-in-progress authoring drafts. Every `load`/`commit`/`remove` takes an optional `DraftSessionOwner` binding; an attributed record rejects every other caller with `OwnerMismatch`, while unattributed (legacy) records stay loadable and are adopted by the first attributed commit. On every backend `commit` keeps the stored runtime context (a new session starts with an empty one) and only `commit_with_context` replaces it; `prune` returns the number of entries it deleted.
- `DraftSessionOwner`: `{ tenant_id, user_id }` binding stamped into every draft session record (audit §2.5).
- `InMemorySsrDraftSessionStore`: Ephemeral in-memory store for unit tests and stateless runs. Enforces `MAX_DRAFT_SESSIONS_PER_OWNER` per-owner session cap. `load` runs under the shared read lock (the sliding TTL is an atomic per-entry deadline) and returns `None` for an expired entry.
- `FileSsrDraftSessionStore`: Filesystem-backed draft store with optimistic generation locking. Updates run under a per-token lock file (`O_EXCL` create, stale-break after 30s, 2s wait deadline) so read-check-write sequences are atomic across processes; TTL refreshes go through compare-and-set so a load can never regress a concurrent commit, are skipped while the stored deadline is still fresh, and report a failed refresh from `load` instead of swallowing it. Layout: `<dir>/t-<tenant>/<token>.json` per tenant, `<dir>/_unattributed/` for sessions without an owner, `<dir>/_locks/`; a token presented by another tenant is `OwnerMismatch`, and a tenant id that cannot name a partition is `InvalidOwner`. On Unix directories are `0700` and files `0600`; a storage directory that is a symbolic link, cannot be restricted to the process user, or is owned by another user (checked explicitly, because mode bits prove nothing to a privileged process) makes the store fail closed (every operation returns `Io`). Records are written through a synced temporary file and an atomic rename. `prune` also collects expired files of the earlier flat layout. A session is over when its record is past its deadline, or when the record cannot be parsed and the file has not been written for a whole TTL; it is then reported absent, and its file is deleted under the token lock by the read that found it or by `prune`, after the file was read again under that lock. A read that finds, under the lock, that a commit replaced the record returns the record now in the file. A record that cannot be parsed and is younger than one TTL returns `Serialization`; a record of a newer format (`PersistentDraftSessionRecord::format` above `DRAFT_SESSION_RECORD_FORMAT`) returns `UnsupportedFormat` instead and is neither aged out nor deleted, because it belongs to a build that is still writing the store. Records without the field are format `0` and are upgraded on the next write. Failure handling: a stale lock that cannot be removed fails the operation with `Io` instead of being retried; a session that cannot be evicted to honour `MAX_DRAFT_SESSIONS_PER_OWNER` fails the create with `Io`; `prune` counts only the entries it removed. A failed removal of a leftover (expired record, temporary file, released lock) or a failed directory flush does not fail the operation and is logged through `tracing` without the file path, because record and lock file names are session tokens.
- `DRAFT_SESSION_DIR_ENV` (`RUSTOK_DRAFT_SESSION_DIR`): storage directory of the default file store. Unset, it falls back to `<OS temp dir>/rustok_draft_sessions`; production hosts should point it at a private, non-shared path.
- `DurableSsrDraftSessionStore`: Distributed KV backend driver adapter. Commits persist through `PersistentDraftSessionBackend::compare_and_save_record` (compare-and-set on the record generation); a lost race returns `GenerationConflict` and reseeds the memory cache from the winner, and a commit whose backend write fails drops its memory entry, so the memory tier never runs ahead of the stored record. `prune` reports the sum of both tiers. The key-value backend deletes a record it found expired, on a read or in `prune`, through `DraftSessionKvDriver::compare_and_delete` against the generation it read, so a commit that landed in between keeps its record: the read then returns that record and `prune` skips it. `prune` counts only the keys its driver deleted and logs the deletions that failed. A record the key-value backend cannot parse returns `Serialization` and is left to the TTL the driver stored it with; a record of a newer format returns `UnsupportedFormat` and is left alone for the same reason as in the file store.
- `PersistentDraftSessionBackend`: `load_record(token, owner)` / `delete_record(token, owner)` take the caller's owner so partitioned backends can locate a record; key-value backends ignore it.
- `DraftSessionKvDriver`: KV driver contract with `compare_and_set(key, expected_cas, value, cas, ttl)` (SETNX-style create when `expected_cas` is `None`) and `compare_and_delete(key, expected_cas)` (deletes only while the key holds that token; `false` when it is absent or holds another); the CAS token is the record generation, not the payload bytes. Both are required: a driver written against the earlier trait has to add `compare_and_delete`.
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

### Rendered SSR draft binding

`SsrDraftRenderBinding` is an optional consumer-provided context containing the token and
store generation of the exact snapshot used for the canvas, or `None` for the saved-page render.
When provided, the host bootstrap prevents edits against a cached or route session inconsistent
with that render, offers a same-origin recovery link, and uses the rendered generation as the
first request's optimistic-concurrency guard. Older responses cannot update the URL; failed
history synchronization stops editing and keeps a recovery link. Pages supplies the binding;
other hosts must opt in only when they can provide the actual rendered snapshot. Integration and
all-store concurrency verification remain outstanding.
