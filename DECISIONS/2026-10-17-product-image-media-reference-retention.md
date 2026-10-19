# Product image relations use Product-owned IDs and Media durable reference retention

- Date: 2026-10-17
- Decision status: Accepted
- Accepted/amended: 2026-10-18 (explicit implementation request)
- Implementation status: In progress (source implementation; Rust/database/transport gates open)
- Owners: rustok-product and rustok-media maintainers
- Extends: [Forum attachment relations use a Forum-owned CAS set and Media durable reference retention](./2026-09-24-forum-attachment-relations.md)
- Supersedes: None
- Superseded by: None

## Context

A Product image is a Product-owned relation row in `product_images`; `media_id` identifies a
Media-owned asset. The pre-cutover Product command decorator asks Media only for a read-side admission
before `add_product_image` writes the relation. It does not retain a durable Media reference, and
neither image deletion nor Product deletion releases one. An admitted asset can therefore be
deleted after the check while its Product image relation still exists.

Media already owns durable consumer reference holds through
`MediaAssetWritePort::{retain_asset_reference, release_asset_reference}` and exposes bounded
owner-reference reads for reconciliation. Product cannot safely bolt those calls onto the current
read-only decorator: `CatalogService::add_product_image` generates `product_images.id` only inside
its local transaction, while a durable Media hold needs a stable consumer reference ID before that
transaction commits. Media may also be remote, so the two owners cannot share a transaction.

The accepted Forum attachment decision establishes the platform safety principle: retain first,
commit the consumer-owned relation locally, release removed holds only after commit, and preserve
ambiguous holds for reconciliation. Product images differ materially because their identity is
currently allocated in the catalog service and Product has no owner-reference reconciliation
surface or Media write-port runtime composition.

## Decision

This proposal adopts the same safety-first protocol for Product images, subject to acceptance:

1. Product allocates the canonical `product_images.id` before requesting Media retention. The ID is
   Product-owned and must be carried only through an internal Product image command/coordinator;
   it is not a browser-supplied image identifier.
2. The stable Media consumer reference is exactly that Product image ID. Product calls Media with
   `owner_module = "product"` and `reference_id = product_images.id`.
3. Product retains the Media reference before its local Product write transaction. The Product
   transaction persists the image row, Product event/outbox facts, and any Product command receipt
   atomically. The successful response is returned only after the local commit.
4. Product releases the reference only after a committed image deletion or Product deletion. A
   failed or ambiguous local commit never triggers automatic release. A failed post-commit release
   also leaves the hold retained.
5. Product owns reconciliation of its consumer references. It compares its current image rows with
   Media's bounded `owner_module = "product"` reference list and exact lookup APIs, reports
   missing, mismatched, and orphan holds, and does not silently release or recreate holds without
   an accepted repair command and operator policy.

No Product-to-Media distributed transaction, direct Media table access, or best-effort
pre-validation-only replacement is allowed.

## Sources of truth and ownership

`rustok-product` is authoritative for Product image relation existence, order, translation, and
Product image ID. `rustok-media` is authoritative for asset lifecycle and durable hold rows.

A Media hold is retention state, not a second Product image source of truth. Product reads Media
only through `MediaAssetReadPort` and `MediaAssetWritePort`; it never accesses Media persistence
directly. Media uses the Product-owned image UUID only as an opaque, consumer-controlled
`reference_id` within the `owner_module = "product"` namespace.

## Invariants

### Allowed states

- A committed Product image has one matching Media durable hold for the same tenant, asset,
  `owner_module`, and image ID.
- A hold may exist without a Product image after a failed/ambiguous Product operation or failed
  post-commit release. It is a conservative orphan pending reconciliation.
- An image deletion commits locally before its prior hold is released.

### Forbidden states

- A committed Product image with no durable Media hold.
- A Product image using a hold owned by another module or another image ID.
- Cross-tenant asset retention or release.
- Release before the local Product removal commits.
- Browser-controlled or caller-selected Product image IDs used as durable hold identities.
- Direct Product access to Media tables or a fabricated cross-service transaction.

## Non-goals

This decision does not make Product own uploads, blobs, delivery URLs, quarantine, or asset
lifecycle. It does not introduce a cross-module foreign key, add image presentation to storefront
projections, or define automatic orphan-hold deletion. It also does not make Media references a
replacement for Product command idempotency receipts.

## Data, transaction, and concurrency boundary

Product image rows, their order, Product events, and a Product command receipt must commit in one
Product transaction. Media retain/release is an intentionally separate safety-first operation.

Retain and release must use a stable command idempotency identity derived from the Product caller
context and image relation identity. Retrying a retained image relation must re-establish the same
Media hold rather than create a new identity. Product deletion must enumerate image IDs and Media
IDs before deleting local rows so post-commit release has the exact committed-removal set.

The Product image ID allocation boundary must be explicit before Media retain. Merely deriving an
ID from media ID or position is rejected because duplicate assets and reordering are valid Product
states.

## Context dimensions

Tenant is mandatory in both Product and Media calls. Product command actor, deadline,
correlation/idempotency identity, and authorization propagate through typed port context. Locale
and channel affect Product presentation but not Media hold identity. Product image position and
alt text do not affect Media hold identity.

## Events and projections

This proposal does not add a new shared domain event. Existing Product image/Product update facts
remain Product-owned. Reconciliation is an owner diagnostic/repair workflow, not an event
projection; its results must be bounded and observable without turning Media's hold list into a
Product read model.

## Failure semantics

Media admission or retain failure rejects the requested image mutation before Product local commit.
If Product fails before commit after retain, the hold remains retained. If local commit is ambiguous,
no compensating release is attempted. If release fails after a committed deletion, the local Product
state remains authoritative and the hold remains a conservative orphan for reconciliation.

A committed image missing its expected hold is an invariant breach and must fail closed for any
repair path. Reconciliation diagnostics may report it but must not silently fabricate a replacement
relation or delete an asset.

## Migration and cutover

Acceptance requires an atomic zero-legacy cutover of Product's image command contract and runtime
composition: internal preallocated image identity, Media read/write port composition for embedded
and supported remote profiles, add/delete/Product-delete hold behavior, reconciliation diagnostics,
all transports, fixtures, and current documentation. Existing Product images need a bounded,
operator-observable backfill that retains a hold using their existing image IDs before enforcement
is enabled.

The current read-only `ProductMediaValidatedCommandPort` must not be described as durable retention
and must not remain as a parallel image-write path after the accepted cutover.

## Alternatives considered

A read-only Media existence/admission check is rejected because it has a deletion time-of-check to
time-of-use race.

Retaining after Product commit is rejected because it leaves a committed relation unprotected if
Media fails or the asset is deleted between calls.

Releasing immediately when a pre-commit Product write fails is rejected because a remote commit
outcome cannot always be proven; conservative retention is safer.

A distributed transaction is rejected because Product and Media can be deployed separately behind
owner ports.

A deterministic reference derived from `(tenant, product, media, position)` is rejected because it
is unstable under reordering and cannot represent repeated use of the same asset.

## Accepted implementation refinements (2026-10-18)

- Retain executes inside the admitted owner future, never in front of receipt replay.
  Image IDs derive from the stable operation UUID; caller DTOs are unchanged.
- Product owns a consumer projection/port and imports no Media crate or persistence.
  Host composition uses atomic `MediaAssetPorts` from one owner instance, including gRPC.
- Committed deletions persist a Product-owned durable release queue in the same local
  transaction as rows/events/receipt. A bounded role-gated worker retries after commit;
  release failure cannot turn committed deletion into a failure response.
- Historical retain/release receipt acknowledgements require current owner lookup
  confirmation before claiming a live hold or acknowledging release work.
- Absent provider and incomplete legacy backfill fail closed as retryable capability
  failures. Existing rows use their original IDs in privileged bounded backfill; owner-local
  provisioning markers gate new image admission and are not a mirror of Media state.
- Diagnostic orphan scans never release holds. Operator routes expose bounded audit,
  backfill and release status; rollout requires a maintenance window for legacy protection.
- Architecture acceptance is not execution certification. Implementation remains In progress
  until the verification gates execute. See [workflow, cutover and evidence gates](../crates/modules/rustok-product/docs/media-reference-retention.md).

## Verification

Acceptance requires SQLite and PostgreSQL coverage for retain-before-commit, commit failure,
receipt replay, image deletion, Product deletion, cross-tenant rejection, duplicate asset use,
post-commit release failure, ambiguous commit safety, and bounded reconciliation diagnostics.
Embedded and remote Media composition must both prove that browser-facing callers never address a
local Media service directly.

## Consequences

The decision adds an explicit Product image allocation/coordinator boundary and a conservative
orphan-hold operational workflow. That cost is intentional: a leaked hold delays cleanup, while a
premature release can delete an asset still referenced by a committed Product image.
