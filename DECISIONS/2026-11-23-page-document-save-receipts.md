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

A Fly editor may commit a page document or restore a journaled revision and
lose the HTTP response. Retrying the same `expected_revision` then conflicts
even when the original operation committed successfully. UI-local attempt IDs
cannot correlate another session with that completed write. Transactionally
constructing the response prevents post-commit result drift but does not give
cross-session replay.

## Decision

1. Pages owns document-write admission, journal and result. An optional
   1..128 ASCII `idempotencyKey` is exposed on `SaveGqlPageDocumentInput`.
   When the caller omits it, PageService derives a deterministic actor-scoped
   key from the normalized request hash. New distinct requests still require
   optimistic CAS.
2. Keys are unique per `(tenant_id, page_id)`. A save receipt binds the
   authenticated actor to a SHA-256 digest of the canonical layout-stripped
   body, format, locale, expected revision, tenant and page. A restore receipt
   uses a separate domain-separated digest of the expected working-copy
   revision and immutable body-revision ID. Both store the exact authorized
   `PageResponse` snapshot; content is never embedded in the key.
3. Admission obtains the page's write-transaction lock and rechecks current
   `pages:update` permission. It checks an existing key/hash/actor receipt
   **before** optimistic revision CAS and before new-write builder policy:
   matching save or restore retries replay the original result; another
   payload or actor fails with `PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT`.
   New operations still require CAS.
4. For a document save, the body/draft write, revision journal, shared symbol
   catalog update, any applicable outbox event, and receipt commit in one
   transaction. For a restore, the working-copy write, revision journal, any
   applicable page event, and receipt commit together; restore intentionally
   does not roll back site-wide symbol definitions. On rollback, no receipt is
   acknowledged. A replay never appends a second journal row or event.
5. An authenticated `pageDocumentSaveReceipt` GraphQL query and the Pages
   service read method look up an explicit save key without resubmitting the
   document. They recheck current write authorization. Restore retries use
   their deterministic server-derived key by resubmitting the exact original
   restore command. A missing save receipt means **unknown**, not proof of
   failure: the original request may still be in flight.
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
   CAS and conflict detection.
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
   while retaining the dirty editor draft. This is not an automatic resend
   and does not imply that an in-flight request has stopped.
10. Source-only test scenarios cover duplicate-key replay and competing CAS
    writers. The opt-in PostgreSQL suite includes duplicate save and restore
    replay across independent pools. The SQLite suite includes duplicate
    restore replay across independent file-backed pools; SQLite document
    writes use `BEGIN IMMEDIATE` before the first revision read. No database
    test has been executed.

## Sources of truth and ownership

Pages owns document save/restore receipts and the committed document revision.
The adjacent create/lifecycle decision owns page identity and lifecycle
operation receipts. The owner-bound Fly draft session records browser intent
acknowledgements.

The accepted [shared Outbox owner receipt](./2026-08-03-owner-operation-receipts.md)
is canonical for leased, potentially independent owner-port commands. The
Pages document save or restore is a short, synchronous, aggregate-locked transaction with
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
    session token. Attributed operations require an exact tenant/user owner
    match. An unattributed legacy record can be accessed only by an
    unattributed internal caller; an authenticated caller cannot claim it or
    become its owner because its original provenance is unknowable. Custom
    draft stores must explicitly advertise atomic receipt support; otherwise
    the editable UI fails closed.
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
15. The owner-bound SSR intent receipt still covers only strict Pages SSR
    browser intents. Separately, PageService protects direct document saves and
    revision restores with durable document receipts. Next `createPage` and
    unpublish/archive/delete now use the separate create/lifecycle operation
    receipts documented in
    [the adjacent decision](./2026-11-24-page-create-and-lifecycle-operation-receipts.md).
    Publish/rollback retain their own operation journals. The Dioxus host
    inherits PageService document receipts when wired through
    `PagesBuilderFacade`; custom host facades and mounted ambiguous
    completion/recovery remain unverified.

## Invariants

A tenant, page, and key identify at most one document save or restore. The
authenticated actor and domain-separated request digest must match on replay.
An expired snapshot leaves a tombstone that prevents reexecution.

## Non-goals

Provider-host edge cases, custom Dioxus facade recovery and mounted Dioxus
gesture parity remain separate work. Next page-create and lifecycle operation
receipts are tracked in the adjacent decision.

## Data, transaction, and concurrency boundary

Pages resolves save and restore receipt replay under the page write lock
before checking the expected revision. SQLite uses `BEGIN IMMEDIATE` before
the revision read; PostgreSQL uses row-level locks. The document or
working-copy write, revision journal, receipt and operation-specific side
effects commit together. The Fly draft store separately commits the browser
intent receipt with its snapshot and generation.

## Context dimensions

The write and lookup use the authorized tenant, page, and actor. The
document request digest also binds the locale and observed revision.

## Events and projections

The committed receipt stores the authorized document-operation response. A
replay returns that snapshot without appending a revision or publishing
another event.

## Failure semantics

A missing save receipt leaves the save outcome unknown; a restore with an
ambiguous response must retry the exact original command or refresh the
document before creating another operation. A different actor or request
conflicts, and a retained tombstone reports expiry. The browser preserves
unsaved work until the operator resolves uncertainty.

A replay is not treated as the current page if a later session has already
advanced the document: the SSR store compares the observed page revision
with the receipt revision, and the browser host compares the refreshed page
revision before updating its surrounding Pages state. A superseded save
receipt is surfaced as `REVISION_CONFLICT` so the stale editor is fenced
rather than being presented as current. Restore callers likewise must refresh
their surrounding page state after an acknowledgement; a replay returns the
original committed snapshot, not a newer page snapshot.

## Migration and cutover

Apply the additive `page_document_save_receipts`,
`page_create_operation_receipts` and
`page_lifecycle_operation_receipts` migrations to PostgreSQL in production.
SQLite migrations are retained for isolated integration tests. MySQL is not
a target database for this deployment. Explicit document-save keys are
optional: when absent, PageService derives a stable key for save and restore
requests; CAS remains mandatory for each new operation.

## Alternatives considered

An editor-local attempt ID cannot recover a save after a session change.
Reexecuting on a stale revision cannot distinguish a lost response from a
competing write. The shared leased receipt service has different retention
and does not provide perpetual tombstones for this aggregate.

## Verification

The opt-in PostgreSQL save/restore race tests and file-backed SQLite
independent-pool restore test are authored but remain unexecuted. The adjacent
create/lifecycle decision adds independent-pool PostgreSQL create/unpublish/
delete scenarios and SQLite create/lifecycle replay scenarios; those also
remain unexecuted. They still need to establish row-lock ordering and SQLite
busy handling in the runtime. All implementation evidence recorded in this
decision is source-only.

## Consequences

Clients can recover acknowledged save and restore revisions across sessions
by receipt lookup or exact-command replay. Pages keeps the newest 64 document
operation response snapshots per page and retains older key tombstones until
page deletion.
