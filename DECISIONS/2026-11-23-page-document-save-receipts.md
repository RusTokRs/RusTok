# Pages document save receipts and cross-session replay

- Date: 2026-11-23
- Decision status: Accepted
- Implementation status: In progress (source changes only; not verified)
- Owner: `rustok-pages`
- Extends: [Page body working copies](./2026-10-09-page-body-draft-and-revision-history.md)
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
10. Source-only test scenarios cover two independent SQLite connections,
    duplicate-key replay, competing distinct CAS writers, and an opt-in
    PostgreSQL two-session test. No database test has been executed.

## Relation to the shared owner receipt primitive

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

## Migration and verification

The additive table migration currently supports PostgreSQL and SQLite;
MySQL fails closed until its schema and concurrent behavior are implemented.
Deploy the server and schema before clients send `idempotencyKey`. Older
clients remain supported but have no durable replay guarantee.

PostgreSQL/MySQL page locking and SQLite busy/locked behavior still require
multi-connection integration evidence. The current migration does not
implement MySQL despite the existing Pages service having MySQL branches.
Restore, publish/lifecycle save correlation, Next direct-write clients,
provider-host edge cases, and Dioxus gesture parity are separate follow-ups.
No Rust compilation, tests, formatting, linting, Playwright or runtime
verification were executed for this change; PR must remain Draft.
