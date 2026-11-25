# Dioxus draft recovery and conflict-safe multi-tab reconciliation

- Date: 2026-11-25
- Status: Accepted
- Implementation status: In progress
- Owners: Fly / Page Builder

## Context

The Page Builder already uses server revision checks and durable save receipts, and browser peer-write notifications are advisory. Those protections prevent a stale tab from silently overwriting a newer server revision, but they do not by themselves preserve an unsaved browser draft across a reload, lost connection, or full Dioxus remount. A single shared browser-storage key would also allow two tabs to overwrite each other's unsaved draft before either tab reaches the server.

The Dioxus host needs recovery without moving document ownership or authoritative persistence out of the consumer module. The Leptos and classic browser adapters must continue to use the same server revision and save-receipt contracts.

## Decision

1. Browser recovery is opt-in and scoped by a stable opaque host value derived from the authenticated tenant and actor. The value is not an email address or other direct identifier. Recovery keys also include a hash of the page identity.
2. Each browser tab writes under a separate writer identity. Every save uses a fresh immutable local-storage key containing that writer identity and a per-writer monotonic sequence. A record contains its base server revision and document, current draft document, format version, sequence, and update time. The reader selects only the latest record for each writer; if a tab is interrupted before older versions are pruned, those versions remain identifiable and are not merged as separate drafts. One tab cannot overwrite another tab's unsaved record or replace bytes another tab has already read.
3. On editor initialization, all records for the exact scope and page are read and reconciled against the latest server document:
   - JSON objects merge recursively by key.
   - Arrays of objects with unique stable `id` values merge by identity; independent changes can be combined without relying on array indexes.
   - Ambiguous arrays, simultaneous edits to the same scalar/value, deletion-vs-edit, and incompatible reorderings are conflicts.
   - A conflict never selects a winner. The editor becomes read-only and exposes the preserved local record JSON and conflicting paths.
4. A recovered merge is installed through a validated, undoable Fly snapshot command. It does not advance the server revision token. The next save therefore remains guarded by the normal server compare-and-set and idempotency receipt.
5. A successful save updates the recovery base from the exact submitted document snapshot, not from a later live editor state. Consumed records are removed only if their stored bytes still match the versions that were read. Because each write has an immutable key, a concurrent save creates a different key and cannot be deleted by a compare-before-delete cleanup of an older version. The current tab's records are removed only when the editor is clean.
6. The browser record limit is 2 MiB. Storage, quota, decode, and serialization errors are surfaced; the UI retains a manual JSON-copy path. The host must not claim that recovery is durable when storage did not confirm a write.
7. Cross-tab unsaved drafts are reconciled on a reload/recovery boundary, not live-merged. BroadcastChannel notifications remain advisory; the server revision and save receipt remain authoritative.
8. The default file-backed SSR editor draft store uses a persistent per-user state directory and 30-day sliding retention. Container deployments must mount the configured directory persistently; multi-replica deployments must use a shared KV backend.

## Consequences

- Hosts that do not provide a recovery scope keep the existing behavior and receive an explicit disabled status.
- Local storage is same-origin storage and is not encrypted. Hosts should enable it only where browser-local storage of the editor document is acceptable.
- The 2 MiB ceiling is a deliberate fail-visible bound for localStorage. Larger documents require an IndexedDB-backed implementation rather than silently splitting or truncating a record.
- This is recovery and safe reconciliation, not collaborative editing. Live presence, conflict-free concurrent editing, and remote draft streaming remain out of scope.
- Native Dioxus palette and layer drag sources use HTML5 dragstart/dragend; an unsuccessful drop emits CancelDrag. The iframe drop handler sets dropEffect explicitly. Parent pointerup/pointercancel must not cancel a drag that is crossing the iframe boundary.
- Source changes do not count as runtime evidence. Dioxus WASM compilation, mounted iframe/drag/resize tests, PostgreSQL integration execution, and the complete adapter audit remain required before release.
