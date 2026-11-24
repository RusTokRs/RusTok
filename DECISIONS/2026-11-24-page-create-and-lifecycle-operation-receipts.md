# Pages create and lifecycle operation receipts

- Date: 2026-11-24
- Decision status: Accepted
- Implementation status: Source-ready; runtime verification pending
- Owners: rustok-pages
- Related: [Pages document save receipts](./2026-11-23-page-document-save-receipts.md)

## Context

A page can be created together with an initial document through one `createPage`
mutation. If the transaction commits but the response is lost, retrying the
mutation can hit the unique slug constraint after creating the page once, or
create a duplicate when the request has no unique slug. The document-save
receipt does not cover page identity creation.

The Next admin also exposes unpublish and delete operations (and the Pages
GraphQL API supports archive). A lost unpublish response can be mistaken for a
stale-version conflict. A lost delete response is harder: the page row no longer
exists, so the retry cannot lock the page or discover the original result
unless the receipt outlives the deleted row.

## Decision

1. Page creation accepts an optional caller-generated `idempotencyKey`.
   The key is validated as 1..128 safe ASCII characters and is unique per
   tenant in `page_create_operation_receipts`. The request hash binds the
   tenant, actor, checked-layout mode and complete create input, including the
   optional initial document.
2. A create reserves its unique key inside the same write transaction **before**
   slug checks or page writes. On PostgreSQL, a competing insert waits on the
   unique key; after the winner commits, the loser rolls back and replays the
   receipt. SQLite serializes the same path with `BEGIN IMMEDIATE`. If the
   transaction fails, the reservation rolls back with the page and no receipt
   remains.
3. The create receipt stores the exact authorized `PageResponse` produced from
   the transaction snapshot. The page, translations, initial document,
   revision journal, translation evidence, outbox event and completed receipt
   commit together. A matching retry returns that original acknowledgement
   before repeating uniqueness checks. A different actor or request hash
   conflicts with `PAGE_CREATE_IDEMPOTENCY_CONFLICT`.
4. Create receipts deliberately have no foreign key to `pages`. A later page
   deletion must not erase the evidence that creation committed. The receipt
   stores the original response, so a replay after later edits returns the
   original creation acknowledgement; callers should query the current page
   separately when they need its latest state.
5. Unpublish, archive and delete accept optional lifecycle idempotency keys.
   `page_lifecycle_operation_receipts` scopes a key to tenant, page and
   operation; its request hash binds the actor and expected page version.
   Receipt lookup occurs under the page write lock before CAS. The page
   transition, promoted working copy, route snapshots, translation evidence,
   outbox events and response receipt commit in one transaction.
6. Delete receipts also have no foreign key to `pages`. Delete checks for a
   receipt before looking up the page; when a concurrent delete removes the row
   while another request waits, the waiter rolls back and re-reads the receipt
   in a fresh transaction. This makes a retry succeed even after the page row
   is gone. Current delete permission and the original owner scope are checked
   before replay, and the receipt actor/request hash must match.
7. The Next admin persists a pending key in tab-scoped `sessionStorage`
   when available (with an in-memory fallback) before sending
   create/unpublish/delete. The storage key is derived from the tenant and
   request fingerprint; no bearer token is stored. A key is removed only
   after the mutation returns successfully. The page list now includes
   `version`; unpublish and delete send that version as an expected-version
   fence, so a stale list cannot silently unpublish or delete a newer edit.
8. GraphQL key arguments remain optional for compatibility. New Next admin
   flows always send keys. Legacy callers that omit them retain the previous
   non-replayable behavior and should be migrated.
9. PostgreSQL is the production database target. SQLite migrations are kept
   for isolated integration tests; MySQL is not part of this deployment's
   database support matrix. Receipt snapshots/tombstones currently have no
   compaction policy; create/lifecycle volume is expected to be low, but
   retention monitoring and a future tombstone-preserving compaction design
   remain operational follow-up.

## Invariants

- One tenant/create key identifies at most one create request.
- One tenant/page/operation/key identifies at most one lifecycle request.
- The same actor and canonical request hash are required for replay.
- Receipt reservation, page mutation, revision evidence and outbox event commit
  or roll back together.
- A replay never repeats page creation, page transition, deletion, route
  tombstone writes or domain events.
- A stale expected version is checked only after receipt lookup, so a lost
  response cannot be converted into a false CAS conflict.
- Delete receipt identity survives deletion of the page row.

## Verification

Source-only regression scenarios cover create replay after a later document
write, concurrent create across independent SQLite pools, concurrent create
across independent PostgreSQL pools, unpublish replay after a later publish,
archive replay after a later publish, delete replay after page removal, and
concurrent PostgreSQL unpublish/delete across independent connections.

No build, Cargo check/test, lint, migration run, GraphQL integration test or
browser/Playwright test has been executed. The tests are authored for the
maintainer's verification pass.
