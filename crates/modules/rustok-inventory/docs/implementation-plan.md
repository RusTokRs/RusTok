# Implementation plan for `rustok-inventory`

## Selected Index consumer continuation (2026-11-02; source only)

- [x] Author selected Distribution schema/source/real sealed listeners using only native Inventory
      history and freshly authenticated current service contexts. This is not compiler/runtime evidence.
- [x] Author exact committed Changed identity verification, authoritative minimum-version load,
      immutable owner mutation UUID and generic durable upsert/tombstone before handler success.
- [x] Author activation's original-baseline-UUID durable pending enqueue before success, including
      empty baselines; exact-job fencing, bounded generic ModuleWork replay and checkpoint resumption.
- [ ] Integrate bounded automatic page-failure backoff and authorized original failed/cancelled-job
      recovery. The existing runner terminalizes page errors; do not count normal yield leases as
      consecutive failures or invoke the retry store after lease terminalization.
- [ ] Verify compiler, real service credentials/current grants/module availability, secret rotation,
      PostgreSQL SQL/locking/concurrency, original-ID restart/yield/cancel/ACK and broker/digest admission.
- [ ] Complete native/HTTP/GraphQL/Next/Leptos operator recovery and deployment preflight for older
      active baselines/missing historical facts. No reset, forged history or replacement UUID.
- [ ] Admit Inventory/Index convergence and Product step-8 evidence. Registry/source presence or
      enqueue success is not a complete source, absence certificate or globally ordered watermark.

The targeted source review reuses the canonical authenticated principal mapper without changing
service identity, bypasses the shared secret cache through its canonical owner, refuses orphan replay checkpoints/unproven
completed status, fixes PostgreSQL inbox schema-version decoding and repairs regression SQL.
Seven additional Rust and five Node regression definitions are authored, not executed; no retry,
operator-recovery, deployment or readiness gate is advanced.

[Actual boundary/configuration](../../rustok-distribution/docs/inventory-index.md),
[generic refresh contract](../../rustok-index/docs/m5-source-refresh-event.md),
[source packet](../contracts/evidence/source-index-consumer-source.json).
The prior publication/history packets remain historical source-only evidence. No tests, build,
SQL, migrations, credentials or application runtime were executed for this continuation.

## Source canonical delivery continuation (2026-11-02; uncompiled)

This bounded slice adds sealed changed/activation facts through the existing canonical outbox,
retains original envelope identities and forwards current caller actors across every real
owner writer, including Product bootstrap and both guarded deletion callers. It uses the
[owning contract](../CRATE_API.md#inventory-source-canonical-delivery-2026-11-02-source-continuation).
No historical migration or delivery history is rewritten. Journal reads never republish events.

- Real tracked changes use their retained change UUID and exact committed source version.
- Staged baseline rows emit no per-SKU source facts. Final activation, page receipt, checkpoint
  and one activation fact share the same owner transaction, including the empty baseline.
- Memory and Iggy fan-out require actual acknowledged module handling for Inventory facts;
  missing handlers cannot turn into broker-only success. Remote cursor ACK stays separate.
- Add focused unrun regression definitions for schema/identity/actor/no-op/rollback/replay/
  activation/relay refusal. Tests remain maintainer-run.
- At this publication handoff the selected Distribution consumer was absent; the newer source
  continuation above authors it. Full native/HTTP/GraphQL/Next/Leptos recovery remains pending.
- Current-installation history is unknown; no source status observation re-creates omitted
  historical source facts or activation. Live cutover still requires explicit preflight/recovery.
- Cargo/rustc are unavailable; typing, schema digest admission, SQL/backend/concurrency/Iggy/
  restart/runtime and Index/step-8 readiness remain unverified.

Source-only verification is recorded in the [canonical-delivery packet](../contracts/evidence/source-delivery-source.json):
20/26 direct guards pass; six dirty-entry-equivalent refusals remain (five byte-identical,
one exception equal after absolute snapshot-root normalization only). Twenty Rust syntax/format,
four MJS syntax, three JSON, 26 Prettier, whitespace and new-link checks pass. All 669 entry
migration Rust files, manifests/lock and pinned historical wires/digest remain unchanged. This
is not Rust typing/backend/runtime evidence and none of the 26 Rust/10 Node definitions ran.

## Inventory source history continuation (2026-11-01; uncompiled)

The next bounded owner slice implements explicit paged current-state baseline, positive
Inventory-only source revisions, retained deletes, a same-transaction source-change journal
and bounded owner reads. It is not the selected generic Index integration or an audit.
See the [owner contract](../CRATE_API.md#inventory-source-history-2026-11-01-source-continuation).

- Migration I027 creates empty owner tables only; historic migration payloads/data are untouched.
- Explicit permission-checked start/page/status commands retain baseline and page identities.
- A building baseline fences actual domain writes; durable original-page replay is read-only, stale new page
  requests refuse. Page size 1–128 and total complete level count 4096 bound work atomically.
- Quantity, ordinary/identity/anonymous reservation, shipment/cancel/restore/reopen/return,
  fresh Product bootstrap and exact guarded deletion participate through existing owner plans.
- Replay, zero, stock no-op, physical preparation and reservation adoption do not advance.
- Stored snapshots/versions and immutable changes share domain rollback; retained tombstones
  outlive live Product/item identities. Missing load is unverified, never absence.
- At the owner-history handoff canonical publication/relay was still pending; the current
  continuation above adds that source slice; the latest selected-consumer section supersedes the
  missing Distribution source/bridge/worker code boundary. Actionable cross-client recovery remains pending. The journal is not a globally
  ordered outbox and the Index source must not be marked ready.
- 18 Rust/8 Node focused definitions are added, all uncompiled/unrun. Native baseline commands
  enforce the actual context budget through the core timeout helper, await rollback on work
  failure, and treat commit timeout as unknown. Creator-actor reassignment is not implemented.
- Apply I027 before matching binaries; uninitialized installations still need the empty tracking
  tables. No compiler/tests/SQL/migrations/runtime checks have been executed in this environment.

## Quantity writer prerequisite (2026-11-01; source only)

Source now consolidates Product's real bootstrap update and all native set/adjust facades into
one Inventory-owned quantity plan. The [owner API](../CRATE_API.md#canonical-quantity-writes-2026-11-01-source-continuation),
[decision](../../../../DECISIONS/2026-03-25-commerce-module-split-product-pricing-inventory.md#quantity-writer-consolidation-2026-11-01)
and [source packet](../contracts/evidence/quantity-writes-source.json) record the contract.

- Concrete caller transaction, current nonnil tenant/actor/Variant, positive saved item/levels;
  missing state refuses rather than recreating stock identity or succeeding silently.
- Complete 4096-level bound plus overflow sentinel, original tenant-location locks before the
  current binding reread, checked wide arithmetic, exact-one conditional target stocked update.
- Existing code=`default`/oldest-active selector with deterministic UUID ties; no warehouse,
  first-level, allocation, archived-location or missing-level fallback is invented.
- Global available target preserves reserved/incoming, all metadata and other locations. No-op
  stock plans are read-only and need no outbox table; signed no-op availability remains valid.
- Existing v1 stock-event compatibility is checked before effects; negative old/new global
  snapshots refuse explicitly. Canonical root publication and adjust-only low notification share
  native/Product rollback, without changing the quantity-free reservation fact.
- Product response and visible-channel projection sums are wide/checked, not order-dependent
  saturation. Visibility is not authoritative absence and these readers are not a new Index source.
- Typed owner/native/Product/HTTP/GraphQL policies preserve storage/unknown classes. Durable
  Product receipt replay does not execute a new stock plan; retry hints are not final outcomes.
- **33 Rust/10 Node new focused definitions, all unrun**; existing guard/API regressions were
  migrated without execution. Direct read-only guard: `node scripts/verify/verify-inventory-quantity-writes.mjs`.
  Cargo/rustc, real isolation/locking/phantom/rollback/concurrency and client/runtime proof remain gates.
- MySQL isolation admission uses active explicit-transaction Performance Schema metadata for the
  same connection, never its session default. Missing instrumentation/inactive metadata refuses;
  query/decode failure propagates without fallback. Actual metadata permissions and nondefault
  transaction/session mismatch scenarios remain real-MySQL maintainer gates.
- Traced Product `update_variant` → quantity hook → Product publication →
  `get_variant_in(&txn, ...)` → `BootstrapService::load_available_quantities(conn, ...)` →
  checked owner aggregation → result recording → commit. The response uses the same transaction,
  not a legacy Variant scalar, a separate connection or a quantity-result override.

### Executed source checks and blockers

23 direct read-only guards: 19 passed, four failures exactly match the immutable dirty-entry
source (same output bytes and exits). The four are `verify-inventory-availability-quantity-context.mjs`
(exit 2), `verify-inventory-port-diagnostic-safety.mjs` (exit 2),
`verify-ecommerce-public-port-error-safety-v2.mjs` (exit 9) and
`verify-product-catalog-receipts.mjs` (exit 1). No unrelated remediation or readiness promotion.
The baseline reconstructed every one of 13140 task-entry files; source-only packet records hashes.

32 scoped Rust files parsed with zero syntax errors and passed Scalar formatting; four MJS files
passed Node syntax without importing companions; four JSON files parsed; all eight MJS/JSON paths
passed Prettier. Eleven changed Markdown files have 19 new resolved links/five distinct new
per-document script references and no new trailing whitespace. All 668 task-entry migration-directory files and the root-event
digest are unchanged. Client source is unchanged relative to this task's dirty entry, not clean main.

The canonical remediation gate passed diff invariants, including a separate exact task-entry
comparison with six new files; architecture metadata failed because Cargo is absent, before clippy.
No compiler, tests, SQL, migration, locking, relay, browser or runtime proof is claimed.

Inventory admin's existing generic error redactor is unchanged; this is not full actionable
Next/Leptos recovery parity, an idempotent native stock command or an operational readiness claim.
No migration or event digest is rewritten. No baseline/backfill, Inventory revision/tombstone,
all-writer publication or generic Index registration is added; those remain the next owner slice.

## Catalog deletion safety (2026-11-01; source only)

The Inventory → generic Index investigation exposed a prerequisite: the native Product
lifecycle hook deleted all reservation history and stock levels before removing an item.
That behavior is not an admissible owner tombstone. This slice hardens the existing native
hook and both Product callers; it does **not** implement an Inventory Index source/version.

The contract is a concrete caller transaction, current nonnil tenant, at most 256 distinct
SKU identities and a complete 4096-level plan (4096+1 overflow probe). Tenant-proved Variants
and items lock in Variant UUID order shared by native stock writers. Missing Inventory
identity is not zero-stock or absence proof. Any reservation row, shipment proof or returned-
stock ledger reference preserves the SKU, including zero/released/deleted/terminal history
and contradictory saved labels. Existence probes project only keys. All stocked, reserved
and incoming counters must individually be zero; net availability is not deletion proof.

Original tenant-owned locations lock before levels; archived empty locations need no
reactivation. The complete current level key/binding set is revalidated before any delete.
PostgreSQL requires READ COMMITTED; other isolation refuses. MySQL uses current locking
reads, and SQLite relies on its transaction/snapshot writer exclusion. Exact-one conditional
level/item effects share Product's transaction and rollback. There is no reservation/history
purge, stock disposal, replacement SKU, new event family or observation-generated revision.
Additive reference indexes support the real bounded existence probes; applied migration
history and all receipt keys/columns remain unchanged. Product bounds the whole child list
before cleanup, preserves publication/default-SKU rules and exposes typed actionable refusals.

Implementation, both real callers, finite Product policies and focused regression sources are
synchronized. The [source packet](../contracts/evidence/catalog-deletion-source.json) and
read-only `scripts/verify/verify-inventory-catalog-deletion.mjs` retain the separate source checks.
There are 31 additional Rust definitions (9 Inventory pure, 15 Commerce persistence, 3 Product
policy, 2 HTTP and 2 GraphQL) and 10 Node companion definitions; **none were executed**.
Snapshots cover complete local owners/prices/events, retained nullable shipment proofs, ignored
conditional effects and immutable refusal-receipt replay. Pure source inspection is not SQL proof.

No compiler, SQL, backend locks, migration or runtime behavior is certified in this environment.
The maintainer runs the tests. I026 adds only the three real history-reference lookup indexes,
after I016/I024/I025; its down removes only indexes. Normal index creation may block writers:
use a controlled rollout window, preserve backups and inspect backend plans/locks before enabling
matching binaries. Existing historical migration payloads, receipt keys and rows are untouched.
Foreign-key cascades, unrestricted SQL and historical downgrades are not certified by this
multi-row service guard; it is not a database-wide prevention or reconciliation mechanism.

Product owns complete graph/publication/default-SKU policy and the enclosing rollback. Known
stock/history/changed conflicts follow its finalized nonretryable failure receipts; invariant and
storage uncertainty remain unknown, never inferred from retry advice. Native/HTTP/GraphQL copy
uses the same finite policy. Existing Next recovery still needs real execution. Leptos Product
CRUD still selects the generic GraphQL lifecycle gateway/redaction, with non-durable retained
CRUD keys and no general native CRUD cutover. Actionable full-client retention/recovery parity
and a general retirement/disposal workflow remain incomplete; no destructive SKU UI is enabled.

### Executed deletion source checks

- The dedicated deletion guard passes, including 31 Rust/10 Node definition checks and 11
  historical Inventory migration hashes. The 15 Commerce definitions include empty corrupt
  root-graph refusal as well as whole-batch stock refusal; no callbacks/owner code were run.
- Of 17 scoped read-only guards, 15 pass. `verify-ecommerce-public-port-error-safety-v2.mjs`
  retains nine findings and `verify-product-catalog-receipts.mjs` retains two `set_variant_axes`
  source-location findings. Their exact outputs equal the hash-proven task-entry dirty baseline,
  not a claim about clean main; neither gate is called green or weakened to hide the failures.
- Tree-sitter parses 35 task-changed Rust files without syntax errors; Scalar WASM rustfmt is
  idempotent over the same 35. Four Node syntax checks and six focused Prettier checks pass.
  These checks do not expand macros, establish types or compile default/SSR/hydrate features.
- All 667 baseline migration-directory Rust files were compared: 666 are unchanged; only the
  Inventory registration file adds I026 and its dependencies. All 607 non-`mod.rs` baseline
  payloads remain byte-identical. I026 exists in source only. The canonical event digest bytes
  remain unchanged and were not regenerated; prior pending digest admission stays pending.
- The 15 new/updated documentation sections have 31 valid local links/packet paths and 11
  existing source-script references. Scoped whitespace checks also pass, including new files.
  This is not a global documentation/ADR governance pass.
- The unsuppressed scoped remediation gate passes diff invariants but stops at architecture
  `cargo metadata`: Cargo is unavailable. Architecture/clippy/compilation are **not** certified.

All new regressions remain unrun, including SQLite trigger fixtures; no backend, migration,
receipt, relay, runtime, client recovery or Index readiness is promoted. Detailed commands,
preexisting failures and remaining gates are retained in the source packet.

Remaining Index work: decide and persist Inventory-owned positive revisions, baseline and
retained tombstones; cover every native set/adjust, reservation, shipment/restore/reconsume,
return and Product bootstrap writer; add canonical publication and selected source/bridge/
worker registration with real backend and convergence evidence. Bootstrap quantity updates
and native set/adjust arithmetic are identified writer gaps, not certified by this deletion
slice. Do not borrow Product revisions or broaden the UUID-only reservation fact silently.

## Reservation adoption into Order (2026-10-31; source work)

Ordinary bounded Product step-8 continuation, not an audit phase or readiness promotion.
[Owner API](../CRATE_API.md#reservation-adoption-into-order-2026-10-31-source-continuation),
[Commerce handoff](../../rustok-commerce/docs/implementation-plan.md#inventory-owned-reservation-adoption-2026-10-31)
and [ADR 10h](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10h-inventory-owned-reservation-adoption-into-order-2026-10-31)
are the contract; the implementation is uncompiled and regression sources are unrun.

- The existing identity port now has `adopt_inventory_by_identity`, with mandatory write
  admission and the same safe owner wrapper. The batch is nonempty and bounded at 1024;
  all UUIDs, positive quantities, unique reservation/external/Cart-line/Order-line identities
  and disjoint source/target line IDs are checked before storage.
- Tenant-proved Variant UUIDs precede deterministic item locks. Reservation reads/locks are
  constrained to those items; the owner validates external identity, original location,
  quantity, active state and operational source provenance. Original tenant-owned locations
  remain usable for adoption/replay after visibility retirement or legacy archival; no
  current availability, publication, Pricing, backorder-policy refresh or location reselection occurs.
- All reservation and reserved-level coverage checks precede binding writes. Counter sums use
  checked arithmetic; missing/ambiguous/insufficient reserved levels refuse. On-hand stock is
  deliberately not an eligibility check for an existing backorder allocation. Levels, quantity,
  external IDs and location never change during adoption.
- New bindings update only the owner line/Order provenance and description/timestamp. Exact
  already-adopted rows do not refresh timestamps or rewrite metadata. Released or partially
  consumed reservations refuse rather than being restored. Original exact release identity
  continues to work after adoption.
  The current release writer and historical disposal/supersession migrations were traced before
  treating metadata as passive payload: non-null terminal markers also refuse an apparently
  positive active row, without erasing markers or resurrecting a prior allocation.
- Full saved metadata plus both target fields is measured against the existing physical
  32 KiB limit in one backend-native batch projection before the first write: PostgreSQL
  JSONB text bytes and SQLite JSON object bytes, not character counts or a guessed compact
  JSON length. `inventory.adoption_metadata_too_large` is nonretryable; no metadata is
  truncated/dropped. Unsupported storage refuses explicitly. Exact replay does not resize
  the unchanged payload. The SQL projection itself remains unexecuted here.
- Crate-root and `ports` factory/concrete paths all use the same admitted wrapper; private
  persistence cannot be constructed through a public bypass. Missing/zero deadlines refuse
  through the canonical policy before storage. Commerce also checks positive inherited
  deadline before timeout/provider polling; a zero timeout is not admission.
- Current allocator `inventory_identity_port` source and known prior adopted
  `checkout_operation` source are distinguished. Known compaction-marked rows may lack the
  Cart/operation labels that the applied compaction migration intentionally omitted;
  contradictory retained labels and missing immutable identities refuse. Commerce verifies
  its own journal identity. No historical marker, label, schema or migration is rewritten.
- Commerce passes its already-selected identity capability, correlates the complete owner
  reply, then commits its own mapping and checkpoint separately. Partial handoff recovery
  reuses existing identities; there is no cross-owner transaction or new receipt. False owner
  retry advice parks the unverified handoff, not automatic compensation/release/reallocation.
  A local replay read/provenance error is not proof that a previous owner call had no effect;
  the same bounded recovery policy covers it. Recovery preserves the original typed cause
  before its compensation lookup, including when journal parking already succeeded.
- Existing Inventory and Commerce FBA registries declare the live identity port and three
  operations. Six Inventory matrix cases remain `static_locked_runtime_pending`; smoke stays
  `executable_no_compile`. Original diagnostic evidence has a separate source continuation,
  not fabricated runtime proof. Earlier Cart declaration/matrix assertion drift is synchronized
  without dropping locked-observation/context assertions or promoting their status.

### Source verification and maintainer gates

Read-only guard: `node scripts/verify/verify-commerce-checkout-inventory-adoption.mjs`.
The [Commerce source record](../../rustok-commerce/docs/implementation-plan.md#inventory-adoption-source-evidence)
records executed static checks separately from unrun regression sources. The guard's companion
MJS cases, Inventory library cases, Commerce library cases and registered
`inventory_service_test` / `checkout_service_test` targets remain maintainer-run.

No Rust types/lifetimes/features, SQL/rollback/contention, crash/lost-response recovery,
mounted/remote provider parity or complete shipment/cancellation/return/outbox/Index chain
is certified. Cargo/rustc are absent. No reservation outbox publisher or stock hold is added;
existing quantity-only reservation weaknesses and wider step-8 convergence remain open.

## Product downstream stock continuation (2026-10-30; source work)

Ordinary Product step-8 work, not an audit-ledger advancement. Source is written, not
compiled or database/runtime-certified; the maintainer runs tests.

- New identity allocation probes up to 4096+1 candidate SKU levels and refuses overflow,
  never truncates. It filters active tenant-owned stock locations with the same Inventory
  channel-visibility helper as public availability, including when backorders are allowed.
- Explicit availability request channel and trusted context must normalize to the same
  nonblank slug. No selectors means global; zero-quantity availability probes remain valid.
- Existing matching receipts precede current eligibility. Replay keeps the original location
  after archival/visibility changes; release uses that location and stays idempotent. Only
  new reservations receive owner-controlled `allocated_channel_slug` provenance. This is not
  an authorization/eligibility receipt and there is no historical metadata backfill.
- Public crate-root and `ports` paths expose the same admitted wrappers. `ports.rs` is now
  compiled privately as `ports_impl`, avoiding a public raw-implementation bypass while
  retaining DTO/trait and factory paths. Touched entry logs use only bounded request facts.
- Unexecuted regression sources cover channel contradiction/normalization, public entry wiring,
  visible versus larger hidden stock, global refusal, archived receipt replay/exact repeated
  release and hidden-stock backorder refusal. Persistence cases are included from Commerce's
  registered `inventory_service_test` target, not assumed auto-discovered.

Selected Scalar syntax/format checks and the owner/local/port diagnostic source guards are
not tests, compilation, SQL/concurrency or mounted evidence. The architecture dependency guard
is blocked by the missing Cargo toolchain. Historical migrations, receipts and stock links
are untouched. Availability numbers (`available_quantity`/`in_stock` on reservation outcomes)
remain global/descriptive, not proof of a location's channel eligibility. Candidate bounds
apply to initial allocation, not replay or every inventory read. Location eligibility races,
shipment/return/cancellation effects, event/outbox/projection convergence and selected remote
execution remain open in the owning plans; this slice adds no reservation outbox publisher.

## Current state

`rustok-inventory` owns stock, reservations, availability, public-channel
inventory projections, and the inventory admin read/write surface. Commerce
uses owner public-channel availability for cart and checkout; it must not
recreate backorder, tenant/channel/locale, or stock lookup policy.

Inventory admin stock operations are owned by native/transport mutations:
set quantity, adjust quantity, reserve, release reservation, and check
availability. The admin package uses `HostRuntimeContext` and a typed
transactional event bus, is host-neutral, and intentionally has no GraphQL
fallback for this current operator surface.

Mounted Inventory admin native endpoints use static public error envelopes for
framework extraction, runtime dependency, read, and write failures. Original
owner causes remain in structured diagnostics with per-call correlation,
tenant, actor, subject, and request channel/locale context when available. The
source contract is guarded by
`scripts/verify/verify-inventory-admin-native-error-safety.mjs`; runtime evidence
has not been executed or promoted.

The Inventory admin client transport facade independently fails closed after the
native call. All eight read/write operations create a per-call correlation
context, retain the original `ServerFnError` only in structured diagnostics, and
return one static `InventoryTransportError` message. Tenant, product, variant,
locale, filter, and numeric request values are represented only by safe
presence/length shape facts. This source contract is guarded by
`scripts/verify/verify-inventory-admin-client-transport-error-safety.mjs` and
remains unvalidated.

The canonical durable reservation wrapper now retains correlation, closed
operation labels, bounded context shape, bounded reservation/request shape,
stable code, retryability, closed error-kind labels, and error-message
presence/length. It no longer records complete `PortError` values, raw context,
raw UUIDs, exact quantities, raw external IDs, or tenant UUID parse causes.
Admission, tenant-validation, and exact stable local-outcome mappings return the
same errors unchanged. This source contract is guarded by
`scripts/verify/verify-inventory-reservation-owner-context.mjs` and
`scripts/verify/verify-inventory-reservation-local-context.mjs`; execution
evidence remains open.

`BootstrapService` owns default-location creation, initial item/level creation,
variant-record cleanup, and batched available-quantity reads when product
creates or deletes variants. This is a native transaction-sharing bootstrap
exception: no GraphQL/REST bootstrap contract exists, while public availability
and reservation contracts remain inventory-owned.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Admin native error safety: `source_ready_unvalidated`
- Admin client transport error safety: `source_ready_unvalidated`
- Durable reservation owner diagnostic safety: `source_ready_unvalidated`
- FBA provider contract: `InventoryReservationPort` /
  `inventory.reservation.v1` in
  `crates/modules/rustok-inventory/contracts/inventory-fba-registry.json`.
- Static and no-compile runtime evidence:
  `crates/modules/rustok-inventory/contracts/evidence/inventory-contract-test-static-matrix.json`
  and `crates/modules/rustok-inventory/contracts/evidence/inventory-runtime-contract-smoke.json`.
- Durable reservation diagnostic evidence:
  `crates/modules/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source.json`
  and
  `crates/modules/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source-review.json`.
- `scripts/verify/verify-inventory-admin-boundary.mjs` locks the native
  core/transport/UI split and absence of pre-FFA/GraphQL admin paths.
- `scripts/verify/verify-inventory-admin-native-error-safety.mjs` locks mounted
  static public envelopes and private owner-cause diagnostics without claiming a
  runtime pass.
- `scripts/verify/verify-inventory-admin-client-transport-error-safety.mjs` locks
  the final facade mapping, per-operation correlation context, safe request-shape
  diagnostics, and static `InventoryTransportError` text without claiming a
  runtime pass.
- `scripts/verify/verify-inventory-reservation-owner-context.mjs` and
  `scripts/verify/verify-inventory-reservation-local-context.mjs` lock bounded
  durable reservation admission, tenant-validation, request-shape, and local
  outcome diagnostics while preserving the exact owner calls and error returns.

## Open results

1. **Evolve locations, reservations, and availability as one owner contract.**
   Introduce any location or reservation semantics through inventory APIs and
   preserve explicit available/in-stock behavior rather than relying on legacy
   variant quantity.
   **Depends on:** the inventory persistence model and product variant contract.
   **Done when:** writes, reads, and public projections express the same
   location/reservation semantics with targeted integration coverage.

2. **Cover channel-aware availability edge cases.** Exercise backorder policy,
   missing/depleted levels, tenant/channel/locale context, and checkout/catalog
   visibility through the owner public-channel projection.
   **Depends on:** commerce checkout and storefront projection consumers.
   **Done when:** integration tests prove that cart, checkout, and storefront
   read models cannot diverge from `InventoryService` policy.

3. **Run the verification/CI evidence slice for `InventoryReservationPort`,
   durable reservation owner diagnostics, and admin native/client error
   safety.** Execute the remote-adapter contract and fallback profiles before a
   `boundary_ready` promotion; retain native-only admin transport unless a
   public parity contract is introduced. Execute the focused error-safety
   guards, compile the default/hydrate/SSR package, and retain mounted browser
   plus server-function failure evidence before promoting any source-only
   status.
   **Depends on:** a runtime-composed commerce consumer and remote adapter
   environment.
   **Done when:** deadline, idempotency, typed-error, degraded-mode, owner
   invocation, durable reservation diagnostic sanitization, client-facade
   sanitization, and mounted public-envelope evidence covers every published
   port operation and native admin endpoint.

## Verification

- `npm run verify:inventory:admin-boundary`
- `node scripts/verify/verify-inventory-admin-native-error-safety.mjs`
- `node scripts/verify/verify-inventory-admin-client-transport-error-safety.mjs`
- `node scripts/verify/verify-inventory-reservation-owner-context.mjs`
- `node scripts/verify/verify-inventory-reservation-local-context.mjs`
- `node scripts/verify/verify-inventory-port-diagnostic-safety.mjs`
- `npm run verify:ecommerce:fba`
- `cargo xtask module validate inventory`
- `cargo xtask module test inventory`
- Targeted stock mutation, reservation, public-channel projection, and
  checkout-facing invariant tests.

## Change rules

1. Keep stock, reservation, and availability policy in this module.
2. Update local documentation, `rustok-module.toml`, and commerce documentation
   with any inventory/checkout/channel contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
4. Keep durable reservation diagnostics correlation-aware and shape-only; do
   not log complete port errors, raw context, reservation identities, external
   identities, exact quantities, or parser causes.

## Return-restock identity continuation (2026-10-31; source only)

The [Commerce source handoff](../../rustok-commerce/docs/implementation-plan.md#return-restock-identity-source-evidence)
and [accepted amendment](../../../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#return-restock-identity-amendment-2026-10-31)
record the bounded saved Order SKU/quantity read, tenant/Variant/allocation proof, exact
existing-ledger replay and same-identity recovery. Native Inventory stock/receipt atomicity
is not atomicity with Order completion or Commerce. No new schema/receipt or historical
migration rewrite is introduced. Saved allocations are not reselected. Exact archived-location
replay remains read-only; fresh writes need an unarchived saved receiving location under the
existing storage guard, with sorted location locks before levels/receipts. Archived receiving
refusals park same-identity reconciliation rather than a storage retry or implicit relocation.

Source definitions cover request/owner/outcome scope, SKU-less versus missing tracking,
allocation ambiguity, retained terminal facts, replay conflict, overflow, batch rollback and
selected capability use. The task adds 33 Rust/23 MJS regression definitions across owners,
all unrun; source syntax/guards/formatting do not establish Rust types, SQL or runtime.

- [x] Publish and compose the mandatory saved-binding read and native restock identity contract.
- [x] Preserve original projection semantics, immutable owner facts and bounded diagnostics.
- [ ] Execute compilation, regressions, PostgreSQL/SQLite locking/rollback/restart and mounted/
      remote evidence. No module readiness or Product step-8 completion is claimed.
- [x] Implement a separate bounded shipment/cancel/reopen native provenance source contract;
      see the continuation below. This return-identity slice still proves no historical movements.
- [ ] Validate shipment/return overlap and stock event/outbox/Index convergence; no automatic
      backfill, compensation, identity rotation or metadata repair is authorized.

## Shipment-stock provenance continuation (2026-11-01; source only)

The [Commerce handoff](../../rustok-commerce/docs/implementation-plan.md#shipment-stock-provenance-source-evidence)
and [owning ADR](../../../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#shipment-identity-and-native-movement-provenance-amendment-2026-11-01)
describe Inventory's bounded original-allocation/proof/arithmetic commands. F remains the
consumer/lifecycle owner and C composes selected saved Order facts before peer effects.
`inventory_shipment_stock_proofs` is private native provenance, not a second return receipt.

- [x] Implement original tenant/Variant/allocation/location proof, checked ship/restore/reconsume,
      global-key preparation probes with rollback, deterministic serialization and conservative
      return overlap exclusion. Unknown historical progress is never converted to native proof.
- [x] Register only additive `m20261101_000025_create_shipment_stock_proofs`, with no applied
      payload/global return-item PK rewrite or backfill; nonempty down refuses.
- [x] Deduct elapsed lock/read time before proof/stock writes, between batch writes and before
      return; preparation still rolls back on expiration. F keeps the remaining budget at its final
      stock/receipt commit gates. These are phase checks, not database statement timeouts.
- [ ] Compile and execute 16 Commerce physical, 22 leaf F, 7 C recovery/pagination, 7 I scope/
      budget, 4 focused internal F and 2 HTTP context definitions. All 58 Rust definitions and the
      companion's 30 Node cases are unrun; no runtime result is claimed.
- [ ] Coordinate I025 rollout with F's forward-only F121 provider-admission correction after
      F120; drain old stock/provider writers first. Both new migrations remain unexecuted.
- [ ] Execute migration, PostgreSQL/SQLite lock/contention/rollback/global-collision, carrier,
      mounted and remote-owner evidence. Deadlines are phase budgets, not SQL statement timeouts.
- [ ] Complete unpaid-return eligibility, inspection top-ups and event/outbox/Index convergence.

No readiness promotion or Product step-8 completion follows from source guards/formatting.

## Native stock outbox continuation (2026-11-01; source only)

- [x] Reuse the canonical registered root invalidation from I, in F's stock/proof/progress
      transaction and I's own returned-stock/receipt transaction. Required publication errors
      propagate and require rollback; no C/host publisher or direct broker call was added.
- [x] Retain already tenant-proved Product bindings; bound aggregate snapshots and refuse
      foreign/missing location ownership and nonrepresentable v1 global availability. Preserve
      global descriptive before/after semantics, including equal-availability shipments and
      shared snapshots across changed locations; no event movement accounting or stock lease.
- [x] Keep preparation, exact receipt replay, explicit SKU-less and no physical change free
      of publication/new outbox dependencies; only newly admitted return receipts create events.
      Preserve current human UUID or `None` for system/service, without provider/claims payloads.
- [x] Harden unconditional canonical root/typed Outbox inserts against zero-row false success;
      its separate correlated write-once conflict path is unchanged.
- [x] Trace existing Search invalidation to authoritative Product/stock refetch. No generic
      Index, relay durability or reservation-write convergence is inferred from that source trace.
- [ ] Compile and execute 16 additional focused Rust and 10 Node mutation definitions. They
      are registered source only and all unrun; the earlier 58 Rust/30 Node packet stays unrun.
- [ ] Execute PostgreSQL/SQLite snapshots, ownership, representability, suppressed/missing
      outbox, late F rollback, exact replay, contention and actual relay/Search delivery scenarios.
- [ ] Verify source-wired reservation notification delivery and finish generic Index plus
      consumer restart/lag recovery;
      unpaid-return admission and partial inspection top-ups remain separate policy/work items.

See the [native API](../CRATE_API.md#native-stock-outbox-2026-11-01-source-only),
[source evidence](../contracts/evidence/native-stock-outbox-source.json) and
[owning amendment](../../../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#native-stock-outbox-continuation-2026-11-01).
`npm run verify:inventory:stock-events` is read-only source verification;
`npm run test:verify:inventory:stock-events` is a maintainer-run regression command, not executed
by the agent. Cargo/rustc are absent. No readiness or Product step-8 promotion is claimed.

## Anonymous quantity reservation release (2026-11-01; source only)

Status: **source implemented / uncompiled / tests and runtime unexecuted**. This corrects a
prerequisite discovered while tracing reservation notifications; it does not close that tail.

The old quantity release independently drained arbitrary reservation rows and warehouse levels,
so a release could affect a different location or another consumer's identity receipt. It is
replaced by one private bounded Inventory plan, not a new public reservation capability. See the
[owner API](../CRATE_API.md#anonymous-quantity-reservations-2026-11-01-source-only) and
[owning decision](../../../../DECISIONS/2026-03-25-commerce-module-split-product-pricing-inventory.md#anonymous-quantity-reservation-release-2026-11-01).

Source changes:

- select only positively proved native anonymous rows; refuse terminal/contradictory history,
  explicit Cart/Order/line selectors and missing/foreign/duplicate original level bindings;
- bound unkeyed pool materialization to 4096+1 and refuse new anonymous rows at capacity, without
  capping/materializing keyed consumers' claims; aggregate all claims at affected locked levels;
- shared native item lock, complete pre-write plan, exact-one conditional row/level updates and
  transaction rollback for a suppressed write or an unrepresentable final signed quantity;
- preserve metadata and original locations, valid negative `continue` availability and read-only
  zero operations; no new receipt, migration, schema/digest, backfill or automatic reconciliation;
- synchronize native operator labels/Fluent bundles and FBA matrices. Existing SDK/native paths
  delegate to the same owner. No new Next.js mutation/GraphQL fallback is invented.

Evidence: [source record](../contracts/evidence/quantity-release-source.json). **27 Rust definitions
(14 pure, 13 persistence) and 10 Node source-guard definitions are uncompiled/unrun.** The
suppressed-update and suppressed-insert fault definitions are explicitly SQLite fixtures. PostgreSQL/SQLite contention,
locking/rollback, MySQL aggregate decoding, native/remote/UI and deployment behavior are unproven.
Static syntax/format/source checks are supplementary, never execution proof; see the evidence
record for actual statuses and the missing-Cargo architecture gate.

The subsequent [reservation-fact continuation](#reservation-fact-delivery-continuation-2026-11-01-source-only)
now wires the quantity-free Inventory family, current context, canonical transactional publisher,
acknowledged Memory/Iggy-local delivery and explicit Search refetch in source. `InventoryUpdated`
v1 remains unchanged and cannot be used to clamp or prohibit valid negative backorders.
Canonical Rust digest admission, real transport/SQL/restart/lag evidence and generic Index remain
open; no Product revision/refresh-ledger impersonation is permitted. Product step 8 is incomplete.

Maintainer-only regression commands (not executed by the agent):

```sh
cargo test --locked -p rustok-inventory --lib services::inventory::reservation_release::tests
cargo test --locked -p rustok-commerce --test inventory_service_test quantity_release
node scripts/verify/verify-inventory-quantity-release.test.mjs
```

Read-only source guard: `node scripts/verify/verify-inventory-quantity-release.mjs`. Historical
migration bytes and prior physical-stock/adoption evidence must not be rewritten or promoted.

## Reservation fact delivery continuation (2026-11-01; source only)

Owner contract: [reservation invalidation facts](../CRATE_API.md#reservation-invalidation-facts-2026-11-01-source-only).
Decision: [Inventory fact ownership](../../../../DECISIONS/2026-03-25-commerce-module-split-product-pricing-inventory.md#reservation-invalidation-facts-2026-11-01)
and [acknowledged existing-bus delivery](../../../../DECISIONS/2026-07-17-sealed-typed-event-families.md#acknowledged-local-contract-listeners-2026-11-01).

Source work (not execution evidence):

- [x] Add sealed quantity-free `InventoryReservationEvent::Changed` v1; preserve root wire.
- [x] Forward current `PortContext` from quantity ports/authenticated native calls; keep saved
      metadata out of actor construction and preserve replay/zero/adoption publication-free paths.
- [x] Couple one fresh fact to admitted positive counter/receipt effects and checked result,
      using the canonical Outbox helper on the same transaction. Count receipt writes exactly.
- [x] Share bounded checked signed availability across identity and quantity responses; no
      per-level saturation can disguise an unrepresentable result as committed success.
- [x] Extend existing Core EventHandler/dispatcher/bus/Memory with lifecycle-owned bounded awaited
      sealed delivery; propagate startup and handler errors, retaining root best-effort behavior.
- [x] Wire unchanged-envelope primary-first/local-ack fanout for `outbox_iggy`, plus durable
      artifact subscription projection and explicit Search family consumption on both profiles.
- [x] Serialize Search PostgreSQL product upsert/rebuild/delete before fresh reads using one
      tenant product-scope transaction lock and READ COMMITTED; widen availability subtraction.
- [x] Add source guard/FBA assertions and **34 Rust/10 Node regression definitions**, all unrun.

Remaining gates (not waived by source checks):

- [ ] Canonical Rust digest generation/admission on the full current tree, preserving existing
      pending families. No handwritten generated artifact; Cargo/rustc unavailable here.
- [ ] Compile the changed Rust API/runtime surfaces and run scoped regression sources.
- [ ] Prove PostgreSQL/MySQL mutation/locking/rollback and exact-one effects on actual backends.
- [ ] Prove PostgreSQL duplicate/reordered Search refetch, concurrent rebuild/delete, startup,
      local failure after Iggy send, real remote receive/cursor ACK, relay/restart/lag and DLQ.
- [ ] Prove artifact exact/wildcard source dedupe and worker delivery on a real backend.
- [x] Author the selected Inventory native source/revision/consumer and generic bounded replay
      worker without inventing a Product revision (source only, uncompiled/unrun; current section
      above). Search direct-table refetch remains separate and does not certify sellability.
- [ ] Integrate original-job failed/cancelled recovery and failure-only bounded retry/backoff;
      execute and admit min-version/stale/inbox/ACK/restart/backend evidence before cutover.
- [ ] Complete native/mounted/remote/deployment and full downstream Product step-8 verification.

[Source packet](../contracts/evidence/reservation-events-source.json) records actual check results
and maintainer-only test commands. Read-only guard: `npm run verify:inventory:reservation-events`.
The companion `npm run test:verify:inventory:reservation-events` is deliberately **not run**.
Earlier anonymous/physical/adoption evidence remains historical, not upgraded runtime evidence.

Source-history static handoff: 24 direct read-only verifiers, 19 passing and five byte-identical
to the hash-proven dirty task entry; no claim about clean main. Scalar and tree-sitter cover 19
changed Rust files; four MJS syntax checks, five JSON parses and nine Prettier checks pass.
These are not typing/SQL/runtime or regression execution. The packet records exact failure
codes/hashes and preserved historical migration/digest evidence. Native status/read APIs also
use the core budget helper; live scan is not an absence or commit-order certificate.
