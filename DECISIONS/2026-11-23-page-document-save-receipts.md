# Pages document save receipts and cross-session replay

- Date: 2026-11-23
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-pages
- Extends: [Page body working copies](./2026-10-09-page-body-draft-and-revision-history.md)
- Supersedes: None
- Superseded by: None
- Related: [Shared owner-operation receipts](./2026-08-03-owner-operation-receipts.md)

## Context

A Fly editor may commit a page document and lose the HTTP response. Retrying
the same `expected_revision` then conflicts even when the original operation
committed successfully. UI-local attempt IDs cannot correlate another session
with that completed save. Transactionally constructing the response prevents
post-commit result drift but does not give cross-session replay.

## Decision

1. Pages owns save admission, journal and result. An optional 1..128 ASCII
   `idempotencyKey` is exposed on `SaveGqlPageDocumentInput`; legacy requests
   without the key preserve their optimistic CAS behavior.
2. Keys are unique per `(tenant_id, page_id)`. A row binds the authenticated
   actor to a SHA-256 digest of the canonical layout-stripped body, format,
   locale, expected revision, tenant and page. It stores the exact authorized
   `PageResponse` snapshot of the accepted mutation. Content is never in the
   key itself.
3. Admission obtains the page's write-transaction lock and rechecks current
   `pages:update` permission and builder policy. It checks an existing
   key/hash/actor receipt **before** optimistic revision CAS: matching retries
   replay the original result; another payload or actor fails with
   `PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT`. New requests still require CAS.
4. The body write, revision journal, shared symbol catalog update, outbox
   event, and receipt are committed in one transaction. On rollback, none of
   them are acknowledged. A replay never appends a second journal row or event.
5. An authenticated `pageDocumentSaveReceipt` GraphQL query and the Pages
   service read method lookup a previously completed operation without
   resubmitting the document. They recheck the current write authorization.
   Missing means **unknown**, not proof of failure: the original request may
   still be in flight.
6. Full snapshots are retained for the newest 64 operations per page. Older
   rows keep their key/hash/revision tombstone indefinitely while the page
   exists. Replaying a tombstone yields
   `PAGE_DOCUMENT_SAVE_RECEIPT_EXPIRED` and **never executes again**. Page
   deletion cascades to these records. Operators must account for the growing
   compact tombstone count and retained snapshot storage.
7. Fly's Leptos SSR store derives a stable attempt key from tenant, actor,
   page, observed revision, locale and canonical project. A new edit or
   revision generates another key. The former preflight stale-revision check
   cannot precede the server receipt lookup; Pages performs the authoritative
   CAS and conflict detection. After a new save or receipt replay, the SSR
   store re-reads the current page before acknowledging or updating surrounding
   Pages state. A revision advanced beyond both the request's expected revision
   and the receipt revision is surfaced as `REVISION_CONFLICT`, never as a
   successful rebase onto an older receipt.
8. The browser must not auto-merge unsaved work, equate a missing receipt
   with failure, or automatically retry an ambiguous mutation. A caller with
   its original payload may retry with the same key; alternatively it may
   query the receipt and then show an explicit reload/recovery path.
9. The Leptos SSR store performs one read-only receipt lookup after an
   ambiguous network/HTTP 408/429/5xx response; a known completed receipt
   is the only evidence that can turn it into a success acknowledgement.
   GraphQL business rejections, authorization errors and ordinary HTTP 4xx
   remain definite errors. An absent or unreadable receipt is reported as
   `PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN` (an application-level 503)
   while retaining the dirty editor draft. A failed post-receipt page re-read
   is also an unknown reconciliation outcome; the caller must not advance its
   surrounding Pages state from the older receipt. This is not an automatic
   resend and does not imply that an in-flight request has stopped.
10. Source-only test scenarios cover two independent SQLite connections,
    duplicate-key replay, competing distinct CAS writers, and an opt-in
    PostgreSQL two-session test. No database test has been executed.

## Sources of truth and ownership

Pages owns document save receipts and the committed document revision.
The owner-bound Fly draft session records browser intent acknowledgements.

The accepted [shared Outbox owner receipt](./2026-08-03-owner-operation-receipts.md)
is canonical for leased, potentially independent owner-port commands. The
Pages document save is a short, synchronous, aggregate-locked transaction with
no separately committed external effect. It follows Pages' existing domain
publish/rollback operation journals. Its permanent key tombstones prevent a
late retry from becoming a new save after full snapshots expire, which the
shared primitive's terminal-row pruning would not guarantee. This is an
explicitly Pages-owned record rather than a general-purpose second receipt
service. Any future consolidation must preserve the lock order, original
acknowledgement identity and no-reexecution tombstones.

### Pages SSR browser intents

11. The strict Pages SSR browser-intent path now gives each mutating envelope a
    random, validated `operation_id`. The server fingerprints the normalized
    envelope without that ID, then atomically commits the controller snapshot,
    generation and compact acknowledgement in the owner-bound draft session.
    A duplicate ID with the same fingerprint returns the prior receipt before
    generation validation; reusing the ID with another fingerprint is a
    conflict. The receipt stores only the compact save-failure projection;
    current revision/UI state is rebuilt from the current session.
12. Editable SSR Pages renders create the owner-bound draft session before
    exposing the editor and bind its token into the route before the first
    mutation. This closes the first-write window in which a successful draft
    commit could be followed by a lost response before the browser learned its
    session token. Custom draft stores must explicitly advertise atomic receipt
    support; otherwise the editable UI fails closed.
13. The browser persists the exact original mutating envelope in
    `sessionStorage` before sending it. After reload or an ambiguous network/
    HTTP result it performs only a read-only `resolve_operation` lookup. A
    missing receipt remains unknown: the editor blocks new mutations and
    offers an explicit status check or an explicit retry of the same original
    request/ID. There is no automatic mutation retry. The original draft route
    is restored from the pending request if its query token was removed.
14. Browser operation receipts are bounded to 2,048 per session and 1 KiB per
    compact response; capacity/size exhaustion fails closed without evicting
    earlier acknowledgements. Session TTL still bounds retention. The browser
    pending request is tab-scoped; cross-tab notifications remain advisory and
    do not transfer or merge operation identities.
15. This receipt layer covers strict Pages SSR browser intents. It does not yet
    provide durable identities for every direct writer, Next editor path,
    restore/lifecycle workflow or Dioxus host. PageService's content/revision
    save receipt remains a separate, lower-level safeguard for the actual
    document write.

## Invariants

A tenant, page, and key identify at most one save. The authenticated actor
and request digest must match on replay. An expired snapshot leaves a
tombstone that prevents reexecution.

## Non-goals

Restore, publish and lifecycle save correlation, Next direct-write clients,
provider-host edge cases, and Dioxus gesture parity remain separate work.

## Data, transaction, and concurrency boundary

Pages resolves receipt replay under the page write lock before checking the
expected revision. The document, revision journal, catalog updates, outbox
event and save receipt commit together. The Fly draft store separately commits
the browser intent receipt with its snapshot and generation.

## Context dimensions

The write and lookup use the authorized tenant, page, and actor. The
document request digest also binds the locale and observed revision.

## Events and projections

The committed receipt stores the authorized save response. A replay returns
the snapshot without appending a revision or publishing another event.

## Failure semantics

A missing receipt leaves the save outcome unknown. A different actor or
payload conflicts, and a retained tombstone reports expiry. The browser
preserves its pending request until the operator resolves uncertainty.

## Migration and cutover

Apply the additive `page_document_save_receipts` migration to PostgreSQL or
SQLite before the client sends `idempotencyKey`. MySQL fails closed pending a
supported migration. Requests without a key retain the existing revision CAS.

## Alternatives considered

An editor-local attempt ID cannot recover a save after a session change.
Reexecuting on a stale revision cannot distinguish a lost response from a
competing write. The shared leased receipt service has different retention
and does not provide perpetual tombstones for this aggregate.

## Verification

Multi-connection evidence is still required for PostgreSQL locking and SQLite
busy handling. MySQL is outside the implemented migration scope. The
implementation evidence recorded in this decision is source-only.

## Consequences

Clients can recover the acknowledged revision across sessions. Pages keeps
the newest 64 response snapshots per page and retains older key tombstones
until page deletion.
