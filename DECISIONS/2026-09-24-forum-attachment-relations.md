# Forum attachment relations use a Forum-owned CAS set and Media durable reference retention

- Date: 2026-09-24
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-forum and rustok-media maintainers
- Extends: Direct object-store runtime and owner-local lifecycle
- Supersedes: None
- Superseded by: None

## Context

Forum attachments are domain relations, not Media-owned content. Forum needs ordered, bounded attachment sets for topics and replies, with optimistic concurrency independent from Forum content revisions and the immutable mention/quote relation projection.

A read-only Media lifecycle admission is insufficient for persistence because an asset can be deleted after admission but before Forum commits its relation. The Media owner therefore needs a durable consumer reference that fences deletion while Forum owns the relation itself.

The two owners cannot share one database transaction in every deployment because Media may be embedded or remote behind its write port. The design must therefore prefer conservative retention over unsafe compensation.

## Decision

rustok-forum owns two attachment persistence structures: a relation head keyed by tenant, target kind/id and locale containing the current attachment relation revision and Forum content source revision provenance; and bounded relation rows containing stable relation identity, Media asset identity, usage, position and caption.

The attachment relation revision is a dedicated CAS token. 0 means no head exists; the first committed set becomes revision 1; every later replacement requires the exact current token and advances it monotonically. Clearing all attachments still commits a new revision.

Each persisted relation receives a stable consumer-owned Media reference ID derived from the tenant, target kind/id, normalized locale, position and Media asset ID. Usage and caption are presentation attributes and do not change the Media reference identity.

A mutation validates the requested content source revision before Media retention, acquires Media durable references for every desired relation, then the Forum transaction locks the target and revalidates that source revision and the attachment CAS token before it replaces the relation rows atomically and advances the head. Removed Media references are released only after Forum commit. An ambiguous commit result or failed post-commit release leaves the Media hold intact for later reconciliation.

## Sources of truth and ownership

rustok-forum is authoritative for relation existence, order, usage, caption, relation revision and source-revision provenance.

rustok-media is authoritative for Media lifecycle and the existence of durable owner-reference holds. Forum never reads or writes Media tables directly.

The Media reference ID is consumer-owned identity, while the Media hold row is owner-owned retention state.

## Invariants

### Allowed states

- Missing head: canonical empty attachment state at revision 0.
- Head with revision n >= 1: current committed set, including a valid empty set.
- Relation rows contain contiguous positions 0..n-1, with at most 32 rows.
- Every committed relation has a matching Media durable reference hold.

### Forbidden states

- Cross-tenant target or relation identity.
- Invalid target kind or target that does not exist at head creation/update.
- Negative or exhausted relation revisions.
- Duplicate/non-contiguous positions.
- Forum relation persistence without Media reference retention.
- Deletion of a Media asset while a Forum-owned durable reference is held.

## Non-goals

This decision does not make Forum own Media uploads, storage paths, delivery URLs, blobs, quarantine or Media lifecycle. It does not create a distributed transaction across Forum and Media. It does not add attachment-specific editor, GraphQL or REST surfaces in this persistence slice.

## Data, transaction, and concurrency boundary

Forum relation changes are atomic inside the Forum database transaction. All attachment writers lock the target content row before locking/creating the relation head, so concurrent writers for the same target serialize consistently.

Media reference retention is outside the Forum transaction and therefore intentionally forms a safety-first prepare/commit protocol rather than a two-phase distributed transaction.

Retries use stable reference IDs and Media idempotency semantics. An exact already-committed requested state re-establishes its Media holds before returning. No release is attempted for a desired reference before Forum commit.

## Context dimensions

Tenant and normalized locale are part of relation identity. Target kind/id are part of relation identity. Actor, authorization and command policy remain the caller's responsibility at the Forum command boundary. Media receives the trusted tenant, actor, deadline and idempotency context through the owner port. Channel is not part of attachment identity.

## Events and projections

This persistence slice publishes no new cross-module domain event. Attachment relation changes must not manufacture Forum content revisions. Future events/projections may consume attachment state only through Forum-owned reads.

## Failure semantics

Validation and CAS conflicts fail closed. Media provider failures preserve the owner error code and retryability through ForumError::CapabilityFailure.

If Forum fails before commit, newly retained Media holds remain conservatively retained. If the Forum commit outcome is ambiguous, holds are never released automatically. If an old hold cannot be released after a successful Forum commit, the committed Forum state remains authoritative and the hold is treated as conservative orphan state for reconciliation. FORUM-33 audits that state through a public Media owner-reference listing contract and reports it without automatic release.

## Migration and cutover

The additive migration creates the Forum-owned attachment head and relation tables without any cross-module foreign key to media_assets. Existing Forum data requires no backfill because attachment persistence did not previously exist. The source contract is enabled through the Forum module manifest's Media dependency.

## Alternatives considered

A single mutable row in the existing forum_relation_revisions stream was rejected because that stream is immutable mention/quote history and its revision identity is not an attachment-set CAS.

A read-only MediaAssetReferenceAdmission check was rejected because it has a time-of-check to time-of-use deletion race.

A cross-database distributed transaction was rejected because Forum and Media may be separated by gRPC. Conservative Media holds are safer than rollback that cannot prove the remote owner's state.

## Verification

Required evidence includes SQLite and PostgreSQL migration execution, CAS create/replace/clear semantics, stale-writer rejection, exact-state retry, tenant isolation, target validation, Media retention blocking deletion, release after relation removal, ambiguous-commit safety and bounded ordering invariants. Maintainer runtime execution remains outstanding after this source-ready slice.

## Consequences

The relation model is explicit and owner-correct, at the cost of an asynchronous orphan-hold reconciliation problem. That trade-off is intentional: a conservative Media hold can delay physical cleanup, while a premature release could permit deletion of a still-referenced asset.


## Reconciliation diagnostic

After relation persistence, Forum audits the conservative-hold failure mode through the public
owner boundary rather than querying Media persistence. Media exposes a bounded
`MediaAssetReferenceListRequest/Page` for one normalized owner module with a strict
`reference_id` keyset and a bounded exact `MediaAssetReferenceLookupRequest/Result` for Forum
relation IDs. Forum scans only `owner_module = "forum"` and checks each returned hold against the
Forum-owned `forum_attachment_relations.reference_id` identity in the same tenant, while the
reverse relation page verifies that every committed relation still has the expected durable hold.

The diagnostic distinguishes an `orphan_media_hold` when no Forum relation exists, a
`missing_media_hold` when a committed Forum relation has no Media hold, and a
`media_reference_mismatch` when the same reference identity is bound to a different Media asset.
The two owners cannot provide one distributed snapshot in remote deployments, so each page is
explicitly page-local and diagnostic. No reconciliation path releases or retains a Media hold;
automatic repair remains subject to the existing FORUM-33 operator RBAC, dry-run, audit,
idempotent job-state and bounded-recovery requirements.
