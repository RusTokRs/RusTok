# Product write boundary

Status: `decided` for the owner split; receipt and Media-retention status is recorded explicitly
below.

## Decision

Product is the single writer of its own aggregate. Every write that another module, deployment, or
admin surface performs must use the relevant Product-owned transport or command boundary. Calling
a plain `CatalogService`, `ProductCatalogSchemaService`, `ProductRelationService`, or
`BundleService` mutation directly from a foreign crate is not an allowed integration path: that
bypasses the policy-carrying command context and can bypass durable receipt behavior.

The currently mounted Commerce and FFA adapters invoke the explicit receipt-bound Relation and
Bundle commands while a dedicated cross-module write-port replacement is not yet defined. Those
adapters must not call the corresponding plain `ProductRelationsPort`/`BundlePort` mutators.

## Ports and command surfaces

| Write family | Owner boundary | Operations | Current call/receipt policy |
|---|---|---|---|
| Product lifecycle, variants, variant axes, images | `ProductCatalogCommandPort` | create/update/delete product; publish/unpublish/archive; create/update/delete variant; set axes; add/update/delete/reorder image | `PortCallPolicy::write()` requires deadline and caller key. Durable owner receipts for all fourteen commands: tenant-scoped admission/replay, actual public-result encoding before commit, same-transaction completion, awaited rollback before deterministic failure finalization. Source implementation; Rust/PostgreSQL execution remains unverified. |
| Attribute definitions, options, schemas, category schemas/bindings, and attribute values | `ProductCatalogSchemaWritePort` | mounted schema/EAV operations | Durable receipts: `rustok_outbox::idempotency::admit` → Product write transaction → `complete`/terminal `fail`; receipt-result encoding, a missing result, or completion failure awaits transaction rollback before the error leaves the owner, as does any early owner error; only deterministic failures become terminal, while retryable failures retain a reclaimable lease; replay decodes the stored response. |
| Product relations | `ProductRelationService::create_relation_idempotent`, `update_relation_idempotent`, `delete_relation_idempotent`, and `reorder_relations_idempotent` with `ProductRelationCommandContext` | create, update, delete, reorder | Tenant-scoped durable receipt and same-transaction response completion for every command. Plain relation-port mutations are not external ingress. |
| Product bundles | `BundleService::create_bundle_idempotent`, `update_bundle_idempotent`, `delete_bundle_idempotent`, `add_bundle_item_idempotent`, and `remove_bundle_item_idempotent` with `BundleCommandContext` | create, update, delete, add item, remove item | Tenant-scoped durable receipt and same-transaction response completion for every command. Plain Bundle-port mutations are not external ingress. |

## Lifecycle receipts and pre-commit results

`ProductCatalogCommandPort` now admits all fourteen catalog operations through the shared
`product` owner receipt ledger. Request identity includes the actor, locale, aggregate/child IDs and
full command input; deadline/correlation changes do not alter the logical request. Reusing a key
with a different request or operation is rejected, including a changed predecessor revision.
Successful retries decode the stored response rather than rereading the current Product. Caller
keys retain their deployed transport namespaces and digest formulas.

Concrete commands build their actual Product/Variant/image/axes response before commit, including
same-status no-ops; deletes/reorders record unit. Product tags and metadata, Taxonomy names,
base prices, inventory, translations and axis values are projected on the supplied transaction.
A public-projection error therefore refuses the owner commit rather than reporting a failure after
persisting the mutation. The shared write transaction atomically persists receipt success with rows
and outbox events. It awaits rollback before returning an early owner failure to receipt
finalization. Deterministic failures become durable errors; unavailable/unknown commit outcomes
retain a reclaimable processing lease. Create Product/Variant root IDs derive from the admitted
operation UUID; internal calls without a receipt retain generated IDs.

This is source hardening, **not runtime certification**. The registered catalog-receipt source guard
and mutation tests are not substitutes for the added Rust service tests, actual database failure
injection, PostgreSQL concurrency/lease-reclaim tests, or mounted client retry evidence. Next
save/status/delete callers now retain browser-journal attempts across reload/navigation; child
mutations still need stable logical-attempt identity. Catalog create/update now compose typed
value batches in one transaction; database and mounted-client execution remain unverified.
## Product image Media retention

The [accepted Media ADR](../../../../DECISIONS/2026-10-17-product-image-media-reference-retention.md)
now has a source implementation: Product-owned stable image IDs, retain/current-hold confirmation
inside admitted execution before the local transaction, same-transaction coverage markers,
committed-removal release work, bounded background delivery, and diagnostic-only reconciliation.
The old read-only decorator and opaque image-write fallback are removed. Completed receipts do
not depend on live Media. Missing provider or incomplete legacy backfill is retryable/unavailable.

Product uses its own consumer port; only the host adapts Media's public atomic read/write pair.
Privileged bounded backfill preserves existing image IDs. New image admission is blocked until
legacy provisioning is complete; safe rollout also freezes destructive Media operations while
legacy rows are still unprotected. [Workflow/runbook](./media-reference-retention.md) records
operator routes, rollback constraints, and the still-unexecuted Rust/database/remote gates.
Architecture status is Accepted; implementation status remains In progress, not runtime certified.

## Standalone values versus editorial composition

`ProductCatalogCommandPort` remains the lifecycle/variant/image boundary. Independent EAV authoring
and value writes use the receipt-bound `ProductCatalogSchemaWritePort`. Editorial create/update
compose `ProductAttributeValueBatch` through the shared transaction writer, not through the schema
port, and use only the enclosing catalog receipt. Standalone Product value save/clear and exact-locale
value Translation apply advance the Product business revision so a stale aggregate draft cannot
overwrite those edits unnoticed. The embedded writer does not advance it again.

## Value and Translation lock order

Typed Product value patch/clear resolves category-dependent state only after locking Product.
Variant value patch/clear uses Product → Variant, and exact-locale EAV Translation applies use
Product → value or Product → Variant → value. Identity discovery reads are not validation
snapshots: owner identity is rechecked under the subsequent locks before snapshot/CAS admission.
These source changes require live PostgreSQL concurrency/Translation-target execution before
claiming deadlock/concurrency certification; independent schema administration is not serialized
by the Product row lock alone.

## Receipt identities

- Lifecycle: caller keys are scoped by `commerce-admin-product:{operation}:{digest}` (REST) or
  `commerce-graphql-product:{operation}:{digest}` (GraphQL), and are now persisted as Product
  lifecycle receipts.
- Schema/EAV: Product owner receipt scope and operation are bound by
  `ProductCatalogSchemaWritePort`.
- Relations: `product_relations` owner; `create_relation`, `update_relation`, `delete_relation`,
  and `reorder_relations`; tenant-scoped receipts. Native FFA and Commerce GraphQL derive the
  same owner key with `rustok_product_relations::scoped_caller_idempotency_key`.
- Bundles: `product_bundles` owner; `create_bundle`, `update_bundle`, `delete_bundle`,
  `add_bundle_item`, and `remove_bundle_item`; tenant-scoped receipts. Native FFA and Commerce
  GraphQL derive the same owner key with `rustok_product_bundles::scoped_caller_idempotency_key`.
- Reusing a receipt key with a different payload is rejected as
  `outbox.operation_receipt_conflict` instead of aliasing two logical writes.

## Receipt failure finalization

Every Product receipt ingress, including the Product, Variant, Image, and schema/EAV Translation
adapters, waits for a receipt-bound owner transaction that returned early to roll back before it
attempts an independent failure finalization. It records a terminal receipt only for a deterministic
`PortError`: non-retryable Validation, NotFound or Conflict. Non-retryability alone is insufficient.
Forbidden, Unavailable, Timeout and InvariantViolation retain the processing lease regardless of
retryability, as does every retryable error, so admission can later reclaim an unresolved
infrastructure/configuration outcome instead of replaying a terminal technical failure. If persisting a
deterministic failure itself fails or discovers a reclaimed lease, the ingress returns that retryable
receipt error—not the original terminal domain error—so the caller retries with the same key and
observes the durable replay or reclaim outcome.

## Consequences for new integrations

1. Importers, assistants, and marketplace bridges write through the owner transport or an explicit
   receipt-bound owner command. A new lifecycle operation requires extending
   `ProductCatalogCommandPort` and this document.
2. Every externally reachable Relation or Bundle write carries a durable caller-owned idempotency
   key. A mutation without the key is rejected by the owner transport contract.
3. A new Product lifecycle write must carry a caller key, use the existing catalog receipt path,
   and record its actual response before committing the owner transaction.
4. A write family that is not in the table above is not externally writable; shipping one requires
   extending this document together with the owner contract.

## Evidence

- `crates/modules/rustok-product/src/catalog_command_port.rs` and
  `src/services/catalog/commands.rs` (all catalog commands receipt-bound; runtime evidence open).
- `crates/modules/rustok-product/src/catalog_schema_write_port.rs` and
  `src/services/write_transaction.rs` (receipt-bound schema/EAV writes).
- `crates/modules/rustok-product-relations/src/services/receipts.rs`.
- `crates/modules/rustok-product-bundles/src/services/receipts.rs`.
- `scripts/verify/verify-product-relations-bundles-idempotency.mjs`.

## Catalog base-price replacement

Amounts in the Next Product/Pricing transports are decimal **strings in major units**, including
zero and compare-at amounts. Editors must not convert to cents, use binary floating-point money,
or impose a currency exponent table. The Rust money owner retains currency/rounding admission.

Catalog `update_variant(prices: Some(...))` replaces only unscoped base rows for the locked Variant:
`price_list_id`, `channel_id`, `channel_slug`, `region_id`, `min_quantity` and `max_quantity` must all
be null. `Some([])` clears just those base rows; `None` leaves prices untouched. Contextual input
is refused with a validation error and must use Pricing commands. Contextual stored rows survive
base edits and clears unchanged. Product and Variant deletion still delete all owned prices.
Initial creation retains the existing bootstrap contract, including explicitly supplied channels;
subsequent contextual edits are Pricing-owned.

Catalog Product/Variant responses expose only these base rows because `PriceResponse` has no
context identity and therefore cannot round-trip scoped rows. Pricing's full/contextual reads and
resolver remain authoritative. Next removes the previously ineffective price-list selector; it
must not imply that a catalog base-price payload assigns a price list. The Next Product detail
view accepts a pricing-only overlay keyed strictly by Variant ID, preserves Product inventory,
and does not fall back to catalog prices when Pricing is missing or explicitly returns no price.
This does not certify every Pricing context/default-currency workflow or the multi-command save.

## Atomic editorial values

Catalog create/update accept `attribute_values` batches (locale plus typed patches). They are part
of the same owner command, predecessor check, transaction, events and receipt as the document,
primary category and publication. Batches are applied against the persisted candidate category
before final Active completeness validation; they must not invoke the separately receipted public
schema port or open another transaction. The standalone schema/value API remains for independent
value edits. Editors must not follow a document save with value requests or compensating deletion.

## Next logical editorial attempts

The Next editor owns the immutable request and caller key for save/status/delete through the
browser recovery projection described below.
Re-rendering or retrying an uncertain response must not mint another key, change the predecessor,
or submit current dirty fields. Server Actions return serializable confirmed/rejected/unknown
outcomes; exceptions are never definitive refusal. Only bounded, nonretryable Product domain refusals carrying GraphQL `outcome: rejected`
finalize a refused attempt. The mapper emits that marker only for domain error codes returned after awaited
receipt failure finalization/replay, not for context/admission failures that can share the same public
`PRODUCT_VALIDATION` code. `retryable: false` alone is insufficient. Auth, preflight, protocol,
receipt conflicts and transport failures remain unknown. This does not claim that
Variant/Image/Relation/Bundle callbacks already retain attempts.

The snapshot also retains tenant/actor scope. Server Actions compare it against one authenticated
session and reject missing/non-UUID attempt keys before owner IO. This is a context fence, not
client authority. Cache invalidation after an acknowledged commit cannot change that result to
unknown. A server prop refresh cannot silently replace the predecessor of a local dirty draft.

### Browser recovery journal

Editorial attempts use an origin-local recovery projection keyed by authenticated tenant/actor,
Product resource and an opaque `editor_intent` URL key. The browser persists the canonical request
before owner IO and serializes writers for that intent with Web Locks. Unsupported/blocked storage,
invalid records and unavailable locks fail closed, never falling back to a newly minted caller key.
Terminal acknowledgements replace request bodies with small tombstones before the editor can
advance. An old URL therefore recovers its original decision rather than silently becoming a fresh
create. A new intent is explicit; different intent URLs represent different operator edits.

The journal is not owner state and grants no authority. It contains Product draft data, IDs and
caller keys, never auth tokens. Pending records have no automatic expiry; browser data clearing,
private-mode eviction, cross-device recovery and receipt retention remain separate constraints.

The editor initializes recovery before enabling writes. Explicit retry dispatches the saved
operation without revalidating current form fields; even a missing Product route can mount a
recovery-only view for an existing intent (not an editable replacement Product). Server route
mount identity includes tenant, actor, resource and URL intent. Native URL changes cannot submit
an old mounted intent. Duplicated intent URLs serialize admission/IO and reread the shared record
under an exclusive lock; a settled URL never issues another command.

Confirmed and rejected attempts both settle their URL intent. Existing-editor continuation loads
a fresh owner document under an explicit new intent; it does not silently rebase a dirty draft.
A refused draft can instead explicitly rotate intent while preserving its current fields and
predecessor, without sending automatically. Terminal records retain only operation/key/outcome,
not the draft body. A failed terminal write keeps the original pending key recoverable; it does
not authorize a replacement command. Corrupt/foreign records, changed/lost live-editor identity,
unsupported secure-context/Web Locks access and storage quota/read/write failures block new IO.

Scope: source implementation plus Node tests with injected storage/locks/IO and scoped editor
typechecking; real multi-tab browser, owner/backend, eviction and commit-ambiguity execution are
not certified. Clearing browser data can erase both pending requests and tombstones, and a new
mount cannot distinguish that loss from an unused intent. Never clear storage as recovery from
an uncertain commit. Do not automatically expire unresolved records or tombstones; owner receipt
retention and any future safe journal cleanup require a separately proven retention contract.

## Leptos document and Variant actions

Existing Product Save/Publish sends only the document/category/Product EAV/publication command.
The pricing card names the exact hydrated Variant and offers a separate `Save only this variant`
action for its base-price/SKU/barcode/stock controls. Ordinary edit Save respects selected status;
only the explicit creation Publish action forces Active. This is an explicit separation of
transactions, not an extension of Product document atomicity to Variant writes.

Both actions snapshot their controls before network IO and consume their own results. Document
acknowledgement advances the confirmed Product snapshot; Variant acknowledgement updates only
that row. Neither action refetches the whole editor over the other action's unsaved draft. Local
Variant validation refuses invalid decimal/inventory scalars and currently unsupported nullable
SKU/barcode clears. Empty price input leaves base prices unchanged; it must not synthesize a free
USD row. Input freezing during IO does not provide durable pending-request recovery.

Source guards pin this separation; added Rust tests and mounted execution remain unverified.
Media/axes/SEO refreshes and Leptos retry-identity/draft retention still require their own review.

## Receipt-aware Shipping Profile admission

The former mutable Shipping Profile preflight in Commerce GraphQL Product/Variant
create/update and REST Product create/update is removed. Product's four owner service
methods now validate explicit assignments before opening the write transaction. The
catalog command port evaluates those methods only inside admitted receipt execution:
completed success and finalized rejection replay without calling Shipping Profiles.
Permission, module, tenant/actor and immutable input/identity admission remain before replay.

Product consumes `ProductShippingProfilePort`, not Commerce persistence or service types.
Commerce's `ShippingProfileService` implements that read-only boundary. The selected
embedded command runtime installs the capability inside CatalogService; it is not a
proxy decorator. Host composition preserves explicitly configured providers and external
Product owners. With Commerce absent, a host can explicitly publish a neutral provider;
there is no implicit remote or database fallback. Embedded Commerce HTTP/GraphQL fallback
composition also installs the owner provider. External Product owners must enforce the
same first-execution policy in their own receipt boundary.

Create validates both the Product profile and every initial Variant override. Distinct
explicit slugs are normalized/deduplicated, bounded to 100 entries/100 bytes per slug,
and read within a single two-second budget (or the caller's shorter deadline). Empty,
omitted, cleared and inherited selections require no provider lookup. Update Product
validates only Patch::Set; Variant patch semantics remain unchanged. No unrelated current
profile lookup is added to lifecycle-only updates or completed replay.

Missing capability, timeout, protocol/authorization/configuration failures and retryable
NotFound are retryable storage/capability outcomes; they must not finalize rejection.
Only nonretryable authoritative unknown/inactive profiles produce Product validation
and a finalized rejection receipt. Unknown Product commit outcomes retain existing
receipt recovery semantics. The check is admission-time evidence, not a distributed
transaction or a guarantee that a profile remains active through commit or forever.
Trusted direct CatalogService writes also validate; they do not acquire caller receipts
unless invoked through the command port. They reuse Product's tenant/actor-bound operation
context. Native/REST/GraphQL consumers must use the selected command runtime.

Source guards and two added owner receipt scenario sources cover the intended contract.
They are not Rust build, database, real provider, mounted transport or outage/restart
execution evidence. Maintainer execution remains required.


## Nullable Variant patches (2026-10-18)

The [nullable Variant update contract](./variant-nullable-patches.md) supersedes earlier notes
that Variant nullable updates still conflate missing and null. Product now owns Keep/Clear/Set
for all five nullable fields, with Set-only Shipping admission, GraphQL exact-decimal mapping,
Next command-shaped callback/action forwarding, separate Leptos create/update drafts and
complete-draft retry intent. The unused foundation update DTO is removed. Source verification
and WASM formatting are not compilation or runtime proof. Ten new Rust regression scenario
sources are unrun, existing editor validation sources are migrated, and no test suite was run.
Mounted transports, database/concurrency/rollback and actual UI gates remain open; this does not
close durable child-command retry or whole-screen Variant atomicity gaps or advance readiness.


## Flex and terminal-failure hardening (2026-10-18)

The [current owner diagnostic contract](./product-public-error-diagnostic-safety.md#flex-and-receipt-failure-continuation-2026-10-18)
records exhaustive Flex classification, static copy, bounded command/replay-decode diagnostics,
and the shared domain-only terminal receipt predicate. Existing stored receipts and lease duration
are unchanged. Six new Rust scenario sources remain unrun; no tests or runtime certification.


## Taxonomy and Core error continuation (2026-10-18)

The [current error boundary](./product-public-error-diagnostic-safety.md#taxonomy-and-nested-core-continuation-2026-10-18)
now distinguishes Taxonomy write refusals from read-projection corruption and centrally classifies
nested Core failures across Product read/write/Translation and shared public/HTTP adapters.
Generic conflicts have an owner Conflict variant rather than Validation text. GraphQL definitive
outcomes require a known domain code and actual domain kind; the shared receipt allowlist and
existing stored receipts remain unchanged. The three category source guards now track current
same-ID/all-backend owner composition rather than retired donor/binding/closure compatibility.
Seven new Rust scenarios and strengthened existing hierarchy scenarios are unrun. Sixteen standalone
source guards and WASM formatting/idempotence of 27 Rust files pass; these do not prove compilation,
mounted behavior, rollback/replay/reclaim or privacy at runtime. No test suites or CI were run.
