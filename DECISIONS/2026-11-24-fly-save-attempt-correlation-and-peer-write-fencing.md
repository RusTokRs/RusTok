---
id: adr://2026-11-24-fly-save-attempt-correlation-and-peer-write-fencing
title: Fly save-attempt correlation and peer-write fencing
status: Accepted
implementation_status: In progress
date: 2026-11-24
owners:
  - fly
  - rustok-page-builder-admin
relations:
  extends:
    - ./2026-11-23-page-document-save-receipts.md
    - ./2026-11-11-fly-ssr-control-contract-and-route-view-state.md
---

# Fly save-attempt correlation and peer-write fencing

## Context

Fly's controller already tracks a local save sequence and the snapshot hash submitted to the
host. That sequence is scoped to one controller instance and is intentionally not restored from
serialized editor state. A delayed response could therefore collide with a later attempt after
rehydration if the sequence and project hash repeated. The UI state machine also accepted
uncorrelated save success/failure transitions, allowing a stale response to clear or fail a newer
save.

The Leptos host has both a typed Rust workspace and the classic `fly-browser.js` adapter. They
can each own a `BroadcastChannel` listener for the same editor root. Without an editor-scoped
sender identity, one adapter would interpret the other's local confirmed write as a sibling-tab
write. The Dioxus host also needs the peer-write protocol while it owns the iframe/canvas lifecycle
directly.

Finally, the Page Builder Save action persists a working document. It is not the release/publish
operation and must not be blocked by a release-readiness policy when the actor has edit permission.

## Decision

1. **Every save attempt has two identities.** `SaveAttempt.sequence` identifies the submitted
   in-memory revision snapshot. `SaveAttempt.request_id` is a fresh UUID that identifies that
   particular asynchronous request, including across controller replacement and sequence reuse.
2. **All save completions are attempt-bound.** `SaveSucceeded`, `SaveFailed` and
   `SaveConflict` must carry the active request ID. A stale or uncorrelated completion cannot
   acknowledge, fail or conflict a newer attempt. Host callbacks receive the request, optional
   `SaveAttempt` and a completion callback; they must complete the exact request through
   `PageBuilderCapabilityExecution::complete`, which shares the facade validation path.
   Known `REVISION_CONFLICT` and `PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN` codes survive the Leptos
   server-function and GraphQL boundaries; a stale-revision response fences only the matching
   attempt and retains the actual server revision when it is present.
3. **An in-flight attempt is not serialized.** The active request ID, save-start sequence and
   snapshot are transient. A restored controller starts without an active request and cannot
   accept a response from the previous controller. The host must use the separate server receipt
   contract to resolve an unknown network outcome; a local UUID is not a durable idempotency key.
4. **Draft-save permission is edit permission.** Page Builder working-draft persistence is gated
   by `edit`. The host's runtime publish/release-readiness policy remains separate and must not
   disable saving a working draft.
5. **Peer-write notifications are advisory and editor-scoped.** The JS adapter and Rust host use
   the same `fly:page-authoring-writes:v1` channel. A sender ID combines a per-document nonce
   with an editor-root scope. Channel instances for the same editor advance their baseline on
   local confirmed writes; another tab or another editor root is treated as a peer and fences the
   stale editor into read-only mode. Dioxus uses the document-scoped identity when it owns the
   only channel in that host. The JS adapter also fences an explicit server revision-conflict
   response if a BroadcastChannel notification was missed, and clears that parked fence only
   after a reload observes a different server revision.
6. **No automatic merge or replay is introduced.** A peer notification asks the user to preserve
   local work and reload. The server's revision compare-and-swap remains authoritative if
   BroadcastChannel is unavailable or a notification is missed.

## Invariants

- A response may mutate save state only when its complete `SaveAttempt` matches the active
  controller attempt; hash and local sequence alone are insufficient.
- A stale failure is ignored just like a stale success.
- An uncorrelated failure cannot retire an active request.
- A deserialized controller has no active request identity.
- The local `request_id` is correlation metadata only. It must not replace the stable server
  idempotency key bound to tenant, actor, page, expected revision and canonical project digest.
- Peer-write messages contain page/revision/hash metadata, not document content or credentials.
- Peer-write fencing does not provide presence, locking, OT/CRDT, operation merging or durable
  cross-session reconciliation.

## Consequences

- `UiIntent` save transitions and the public host callback context carry request identity.
- Callback-backed hosts must call `PageBuilderCapabilityExecution::complete` once with the
  result of the exact asynchronous request; the runtime validates and applies the captured
  `SaveAttempt` before acknowledging or failing it.
- Leptos Rust and JS adapters can share one logical editor identity without self-conflicting;
  independent tabs and editor roots remain isolated.
- Dioxus and Leptos hosts now use the same local correlation and advisory peer-write semantics.
- Server receipt persistence, unknown-outcome recovery across remounts/independent sessions,
  durable restore/lifecycle operation IDs and mounted browser evidence remain separate work.

## Verification status

The source changes are authored on a dedicated branch. No Cargo build, Rust test, lint, Playwright
suite or mounted-host verification was run for this change. The implementation status therefore
remains **In progress** until the maintainer's verification pass succeeds.

## References

- [Pages document save receipts](./2026-11-23-page-document-save-receipts.md)
- [Fly SSR control contract and route view state](./2026-11-11-fly-ssr-control-contract-and-route-view-state.md)
- [Fly / Page Builder backlog](../docs/modules/page-builder-backlog.md)
- [Fly Web](../crates/ui/fly/web/README.md)
- [Fly Dioxus](../crates/ui/fly/dioxus/README.md)
