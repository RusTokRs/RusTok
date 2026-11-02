# Split of `rustok-commerce` into `product`, `pricing`, and `inventory`

- Date: 2026-03-25
- Status: Accepted & Implemented

## Context

The exploratory migration plan for a Medusa-like model required ceasing to treat `rustok-commerce`
as a single module for catalog, pricing, and inventory. This contradicted the actual code structure, where:

- `CatalogService`, `PricingService`, and `InventoryService` lived in one crate;
- runtime wiring and `modules.toml` did not reflect separate platform modules;
- documentation simultaneously required a split and described `commerce` as a single optional module.

Without resolving this contradiction, it was impossible to honestly move forward to cart/order/customer/payment slices and to a
Medusa-compatible API surface.

## Decision

Accept and implement the first stage of the split directly in code:

- extract a common support crate `rustok-commerce-foundation` for shared DTO, entities, errors, and search helpers;
- extract `rustok-product` as a separate optional platform module for catalog, variants, options, and publishing;
- extract `rustok-pricing` as a separate optional platform module for the pricing slice;
- extract `rustok-inventory` as a separate optional platform module for the inventory slice;
- keep `rustok-commerce` as a transitional compatibility facade:
  - re-export shared contracts and extracted services;
  - maintain the legacy GraphQL/REST transport surface;
  - keep the order state machine and legacy migrations not yet extracted into separate modules.

The runtime manifest and server module registry should register `product`, `pricing`, `inventory`, and `commerce`
as separate optional modules, with `commerce` depending on `product`, `pricing`, `inventory`.

## Consequences

Positive:

- the contradiction between the exploratory plan and the actual module topology is resolved;
- platform/runtime wiring now reflects the real decomposition of the commerce domain;
- the next stage of the split can proceed from an honest base, not from a facade monolith.

Negative:

- `rustok-commerce` temporarily remains a transitional facade and still carries the transport/API surface;
- collections/categories/order-related parts are not yet extracted into separate modules;
- the inventory schema/model still requires further normalization according to the backlog migration plan.

Follow-up:

- bring `cart`, `order`, `customer`, `payment`, `fulfillment` to separate modules;
- extract remaining legacy migrations and transport surfaces from `rustok-commerce`;
- continue schema hardening and Medusa-compatible API contract tests.

## Pricing resolution continuation (2026-10-29)

The split remains authoritative, but its original extraction is not a sufficient
pricing contract for an axis-aware storefront. Pricing must evolve in its owner;
Product and UI adapters must not compensate with a second resolver.

- The stable Variant UUID is the SKU key. A bounded Product-owned identity read
  supplies tenant-scoped Variant-to-Product associations without loading complete
  merchandising projections or letting Pricing inspect Product's private tables.
- `PricingReadPort` supports bounded single and batch quotes. A batch shares one
  normalized currency/region/list/channel/quantity context, one resolution time,
  and one Pricing database snapshot. Missing prices are explicit per-SKU results;
  missing/foreign identities, rejected context, unsupported rules, inconsistent
  stored prices and ambiguous winners are errors, never base-price substitutions.
- Channel identity is resolved through `ChannelReadPort`; an ID-only or slug-only
  input is completed from that owner, and contradictory identifiers are rejected.
  Caller actor, locale, trace and deadline context are forwarded, not fabricated.
- Price-list metadata and rows are read in the same Pricing snapshot. Precedence
  is explicit list override, then matching base row plus the selected list rule,
  otherwise matching base row. Within a row class: channel, region, descending
  minimum quantity, ascending maximum quantity. UUID order is not a business rule;
  equal-priority winning rows fail closed and require operator correction.
- A percentage list rule is a non-stacking sale offer against the regular
  reference (`compare_at_amount`, otherwise the current amount). The lower of the
  existing selling price and the rounded offer wins: a discount cannot increase
  an existing sale. Sale flags and actual discount percentages are derived from
  the final rounded money, not copied from the authored rule. Explicit override
  rows remain authoritative and are not silently discounted again.
- Quotes expose the selected price-row identity and whether a base row, explicit
  list override or list rule supplied the result. They are observations, not
  reservations, tax calculations or guarantees that checkout will accept a
  previously displayed amount. Cart/checkout continue to reprice through Pricing.
- Raw catalog/base-price rows are inspection data, not permission to substitute
  another currency, quantity, scope, or SKU after a quote is absent or rejected.
- Money normalization, exact representability and rounding remain owned by
  `rustok-core::money`; derived two-decimal persistence mirrors are not quote inputs.

This is a source implementation continuation, not runtime certification. It does
not introduce a general promotions registry, stacking engine, customer-group
ontology or a second pricing provider. New rule kinds must be typed, have an
actual consumer and reject unsupported persisted metadata. Durable mutation
receipts/CAS, event completeness for list changes, installed-state diagnostics,
axis-aware storefront interaction and mounted transport verification remain
separately tracked in the Pricing/Product implementation plans; the presence of
an idempotency key is not evidence of a durable write receipt.

## Anonymous quantity reservation release (2026-11-01)

The compatibility quantity API is not a durable reservation identity. Its former release
implementation reduced arbitrary active reservation rows FIFO, then independently drained
warehouse counters. It could release a different location and mutate another consumer's
keyed/Order-bound reservation. A caller's Cart/Order/line selector was also discarded by
quantity methods. This is an owner correctness defect, not projection-only work.

Inventory's quantity release now addresses only the positively proved anonymous pool created
by `InventoryService::reserve`: source `legacy_inventory_service`, matching saved Variant,
no external/line binding, no terminal disposition or checkout/adoption provenance. Missing or
unverified metadata is not permission to release. Exact identity allocations remain exclusively
with `InventoryReservationIdentityPort`; a scoped quantity request refuses instead of ignoring
its selectors. The compatibility API remains non-idempotent and must not be used by new checkout
consumers. A required transport key or retry hint is not a durable outcome receipt.

The owner plans the complete bounded request before writes (4096+1 unkeyed-pool and level probes),
chooses only eligible anonymous rows deterministically, and groups their release at each
original `(item, location)`. It proves the current tenant's saved location and exactly one level
for every touched key, aggregates every active ledger claim at that location (without materializing/capping keyed
consumers), and refuses
missing/duplicate/foreign/insufficient/terminal history without fallback or relocation. Hidden
locations are not reselected using fresh channel/publication eligibility. Existing inactive
location storage invariants remain authoritative; a planned nonzero result at an archived
location refuses rather than reactivating or crediting another warehouse.

New anonymous reserves cannot grow past that pool bound or over contradictory native history.
Existing oversized/unverified pools require owner-governed reconciliation, not guessed identities.
Every planned row/level update must affect exactly one saved conditional row or roll back.
All quantity/item/level effects share the existing native transaction. Shared item write locks
serialize compatibility writes with identity/adoption/shipment paths; row/location/level lock
orders are deterministic. No new receipt, schema, migration, historical rewrite or metadata
repair is introduced. Zero quantity remains read-only and creates no Inventory state. Signed
checked availability preserves backorder `continue` and refuses an unrepresentable DTO rather
than saturating, overflowing or redefining backorder policy.

Reservation invalidations remain separate unfinished work. Existing `InventoryUpdated` v1
requires nonnegative global quantities; reservations may legitimately produce negative
availability under `continue`. Reusing that event by clamping/fabricating quantities or rejecting
valid backorders is not accepted. A quantity-free Inventory-owned sealed fact needs a wired
canonical typed transport/consumer on both configured delivery profiles before notification
convergence may be claimed. Do not use Product-owned refresh events, a root-event fallback or
an unwired typed publisher as substitutes. Canonical event digests require the Rust generator,
not hand-authored values. This prerequisite fix promotes neither runtime evidence nor step 8.

## Reservation invalidation facts (2026-11-01)

`InventoryReservationEvent::Changed`, wire type `inventory.reservation.changed` version 1,
is the Inventory-owned **quantity-free** sealed fact for a newly committed reservation counter
change. It carries only Product and Variant UUIDs; tenant, current human actor and canonical
trace/correlation identity remain envelope metadata. It is an invalidation/refetch instruction,
not a stock snapshot, delta, receipt, authorization, Product revision or inventory lease.
Legitimate negative `continue` availability remains valid. `InventoryUpdated` v1 and its
nonnegative physical-stock contract remain unchanged; no clamping or Product refresh impersonation.

Both quantity-service methods now require current `PortContext`, not a tenant UUID alone.
Their in-process port and authenticated native admin callers forward current actor and effective
context instead of reading actor from reservation metadata. Tenant-scoped Variant and parent
Product identity are proved before publishing. A human actor must be a non-nil UUID;
Service/System maps to absent human actor, not an invented user. Global invalidation does not
change the allocation's original channel/location semantics or copy opaque retry metadata.

Fresh identity reserve/release and positive anonymous quantity reserve/release persist one
canonical sealed envelope inside the same owner transaction as level and receipt changes.
Write-count/arithmetic/publication failures refuse and roll back that local transaction.
Identity and quantity results share the bounded checked signed availability evaluator; neither
per-level saturation nor a clamped final DTO can turn an unrepresentable balance into success.
Identity replay, released replay, zero quantity, adoption-only rebinding, and failed/no-op
movements acquire no new outbox dependency and emit no fact. Quantity commands remain explicitly
non-idempotent; adding an event does not create a result receipt or authorize automatic retries.
No migration, historical receipt rewrite, automatic repair or new reservation capability is added.

Delivery uses the acknowledged local contract-listener amendment to the sealed-events decision
on both configured profiles, with no root-family or raw-JSON fallback. Search opts into the exact
Inventory family and refetches its existing Product projection; repeated/reordered facts do not
apply counters from a payload. Search's product-scope writes/rebuilds/deletes serialize before
fresh reads, so concurrent refresh completion cannot install an older observation last. This
is not a generic Index refresh: Index still needs an Inventory-owned source/revision contract,
not Product's immutable revision/ledger fact. That follow-up and full step-8 convergence remain
explicitly open. Canonical digests require the Rust generator; absent Cargo is a release blocker,
not permission to hand-author the generated artifact or declare the contract runtime-verified.

## Catalog deletion retention (2026-11-01)

The original native lifecycle cleanup did not distinguish empty bookkeeping from Inventory-
owned stock or history: it deleted reservation rows and levels unconditionally. That behavior
is superseded at the existing `BootstrapService` boundary. Product must not make a stable
SKU disappear by discarding another owner's live state or retained operational proof.

The selected transaction-sharing Product/Inventory collaboration may remove only positively
proved empty Inventory bookkeeping. It takes the current tenant and a concrete caller
transaction, admits at most 256 distinct SKUs and a complete 4096-level plan, and rejects
missing identities, foreign/missing original locations, any individually nonzero stock/
reserved/incoming counter, or **any** reservation/shipment/returned-stock history reference.
Zero, soft deletion, terminal metadata, nullable proof labels and net-zero availability are
not purge permission. Tenant proof is structural identity verification, not a second Product
publication/eligibility policy. Opaque history existence checks are bound to these proved
parent identities and disclose no foreign payloads.

Variant/item locks use stable Variant order shared with Inventory movement writers. Original
locations precede current level locks; the complete key/binding set is checked before effects.
PostgreSQL READ COMMITTED is required rather than trusting an older snapshot; unsupported
isolation fails closed. MySQL locking reads and SQLite transactional snapshot conflicts must
be verified on their actual backends. Exact-one conditional deletes share Product's rollback
and lifecycle-event transaction. The helper never deletes history, independently commits,
reactivates a location or publishes a fabricated stock quantity/revision. New reference
indexes are additive; historical migrations, global return-item receipt identity and stored
metadata are not rewritten. Existing database cascades are not a substitute for this
multi-row owner rule, nor certification of arbitrary direct SQL outside the owner boundary.

Refusal is preferable to implicit stock disposal or implicit archival: Product owns the
operator's archival decision, while Inventory owns reconciliation and retained proof. A new
retention/disposal workflow, if needed, needs its own explicit owner contract. Both Product
callers must bound and tenant-check their full target set, preserve default-SKU/publication
rules and propagate typed actionable refusals through all error adapters.

This prerequisite does not settle the Inventory → generic Index revision/baseline/tombstone
contract. Every Inventory writer, including native bootstrap quantity updates and set/adjust,
still needs to participate before a source cutover can be claimed. The quantity-free
reservation event remains unchanged. Source implementation is uncompiled and regression/
SQL/migration/concurrency evidence remains maintainer-run; no readiness status is promoted.

## Quantity writer consolidation (2026-11-01)

The Inventory Index continuation found that Product bootstrap selected the first level and
silently ignored missing state, while native set/adjust used unchecked/saturating arithmetic
and different quantity semantics. These are not admissible sources of positive Index revisions.
The existing native transaction-sharing exception is retained, but both paths must delegate to
one Inventory quantity plan under the current tenant, with positive identity, a complete bounded
level set, original-location/current locks, checked signed arithmetic and exact conditional effects.
Structural tenant proof includes the saved Product parent without adding publication eligibility
or acquiring a Product lock after the Variant lock; canonical tenant ownership is immutable.

Absolute quantity means global available units. Only the existing owner-default target level's
stocked counter changes; reserved/incoming/history and every other location remain unchanged.
Default candidate selection retains existing code/oldest-active policy with deterministic ties;
its saved UUID is proved and locked, never reselected or replaced by a new warehouse/level.
Missing/ambiguous identity or level, foreign/archived target, overflow, unsupported snapshot,
changed binding and unrepresentable v1 stock notification refuse rather than clamp or succeed.
No-op stock plans are read-only and have no notification dependency. Product's surrounding
metadata/price/document command remains its own transaction and may still have Product effects.

PostgreSQL and MySQL quantity writes require verified active READ COMMITTED transactions.
MySQL session defaults do not prove the isolation of an already-open transaction. Admission
must use its active explicit transaction's Performance Schema metadata on the same connection;
unavailable/inactive metadata refuses, without changing isolation or falling back to the session
variable. This requires current-transaction instrumentation and metadata SELECT access on MySQL.
Real metadata access, current rereads and SQLite snapshot conflicts remain backend gates.
Canonical root stock publication and exact effects commit with the owner transaction; no second
event bus, legacy raw publisher, helper commit or hidden repair is allowed.
Finite typed refusals cross native/Product/HTTP/GraphQL without provider text. Existing durable
Product receipt classes, not false retry hints, determine terminal refusal versus unknown outcome.
This fixes quantity writers only: explicit baseline, Inventory revisions, retained tombstones,
all-writer publication and selected generic Index integration remain a separate incomplete cutover.

## Inventory-owned source history (2026-11-01; source continuation)

Inventory owns a new, explicit **current-state-only** baseline and positive per-tenant/SKU
source revisions. No migration or normal write initializes an installation, assigns historical
versions, infers historical deletes, or borrows Product revisions. Unknown history remains
unknown. Baseline absence means uninitialized, never an empty authoritative source.

A permission-checked native owner command starts a durable baseline identity. Bounded pages
(up to 128 Variants and 4096 complete levels total, plus sentinels) copy positive tenant/parent/
item/original-location identity and checked counters. Page progress and immutable source-change
rows commit together; only the final complete page activates owner tracking. Caller-selected
smaller pages handle dense SKUs; overflow refuses the entire page without partial progress.
The original baseline identity is retained; every identical page request replays its saved
original progress, stale new or changed page requests refuse, and no reset/key rotation is inferred.

While building, actual stock/creation/deletion writes refuse and roll back. PostgreSQL uses
nonblocking shared writer/exclusive baseline transaction advisory gates, including writers
which already hold Product/Variant/item locks: neither path waits for the other's advisory gate,
so baseline does not invert existing owner lock order. PostgreSQL requires READ COMMITTED.
SQLite uses transaction write/snapshot exclusion; initialization is unsupported on MySQL.
Before explicit initialization, existing domain writes remain untracked and are not certified
Index inputs. Once active, missing/mismatched revision state refuses rather than silently
recreating a baseline. New SKU creation initializes version 1 only for an actual new owner
identity. Real quantity/reservation/physical effects advance once per SKU per owner plan even
when net availability is equal. Replay, zero, preparation and adoption-only metadata do not.

The owner record is a transactionally maintained snapshot of global stocked/reserved/incoming
counters and immutable SKU/item/parent identity; it deliberately excludes Product publication,
SKU copy, policy and location/channel visibility. Stable level-binding proof prevents implicit
repair from becoming tracked identity. Checked i64 aggregates preserve signed availability.
An exact history-preserving delete retains a higher-version tombstone without foreign-key
cascade. All records and immutable changes retain their baseline identity and original UUIDs.
A missing targeted load is unverified, not zero, authoritative absence or a tombstone.

The journal is **not** a second publisher, delivery receipt, global commit-order cursor or
Index readiness claim. Its stable change UUID and committed source version are intended for
the selected canonical delivery continuation. Per-SKU revision paging is ordered by that
SKU's serialized commits; no unsafe global sequence watermark is exposed. No source code writes
Index tables. At this owner-history handoff canonical versioned publication/relay was unwired;
the 2026-11-02 continuation below adds that source slice. Selected Distribution conversion,
source/event/worker registration, persist-before-ACK and native/HTTP/GraphQL/Next/Leptos
operator parity remain deliberately unwired and must precede any Index cutover. Existing stock
and UUID-only reservation notification contracts remain unchanged. Source compilation, tests,
SQL/migration/lock/runtime evidence are unrun; no readiness status is promoted.

Baseline start/page commands are creator-actor bound and permission-checked under the current
context. They use the canonical core timeout helper for acquisition/work/commit and await error
rollback. Commit timeout remains unknown: recover the original baseline/page identity rather
than reconstructing an actor, rotating keys or assuming no effect. Existing error envelopes
conservatively keep source provenance failures nonterminal; new actionable client recovery is
not implemented by this owner slice. Apply empty-table I027 before matching binaries.

## Inventory source canonical delivery (2026-11-02; source continuation)

Two additive sealed Inventory facts use the existing canonical `sys_events`/OutboxRelay path:
`inventory.source.changed` carries only the exact baseline UUID, Variant UUID and positive
committed Inventory source version; `inventory.source.baseline_activated` carries the explicit
baseline UUID. Neither widens `InventoryUpdated` v1 or the UUID-only reservation fact, borrows
Product revisions, embeds generic Index DTOs, or claims an owner receipt/availability lease.

Every real tracked creation/quantity/reservation/physical/deletion effect writes source state,
its retained change row, and the changed fact on the **same caller DatabaseTransaction**.
The retained change UUID is the exact write-once canonical envelope UUID. The current caller
supplies the actor; Bootstrap and both Product deletion paths must forward it explicitly.
No source/journal reader or relay reconstructs authority from baseline/receipt metadata.
Replay, no-op, preparation and adoption do not create another version or source fact.

Building pages retain staged version-1 records without publishing per-SKU ready events. The
final page writes one baseline-activation fact, using the globally unique baseline UUID as its
canonical envelope identity, in the checkpoint/receipt/activation transaction. This includes an
explicit empty baseline. Original page replay publishes nothing. The selected Distribution
consumer must durably arrange the initial bounded source replay before acknowledging activation;
this source delivery slice does not pretend that consumer/rebuild wiring already exists.

The existing acknowledged module-listener path is mandatory for Inventory reservation and
source facts in memory and Iggy fan-out profiles. A missing/inactive/mismatched handler must
refuse before broker-only success; handler failure retains the unchanged outbox identity.
Other explicitly transport-consumed sealed families keep their existing remote cursor policy.
Local completion and a broker consumer cursor are separate acknowledgements. A future selected
Index consumer must perform authoritative load, minimum-version/stale fencing and durable
mutation persistence before its handler/transport acknowledgement.

No old migration is changed and no delivery history is inferred. A baseline already activated
under an earlier binary may have no activation event or earlier changed facts: status/journal
observation never manufactures them. Explicit operator reconciliation/cutover, selected
Distribution generic source/event/rebuild/worker composition, digest admission and full native/
HTTP/GraphQL/Next/Leptos recovery remain open. Do not start a live baseline or promote Index/
step-8 readiness from source checks. No tests, SQL, migrations or runtime are executed here.

## Selected Inventory Index consumption (2026-11-02)

The canonical Inventory source facts are consumed by a selected Distribution bridge through
the existing generic Index catalogs, storage and module-work lifecycle. The bridge owns
conversion; Inventory owns all source reads/revisions/tombstones; Auth owns current worker
service context. Host code authenticates genuine, deployment-bound OAuth service credentials
using the canonical secret and current-principal machinery, not event actors or saved claims.
No service UUID, grant, system/admin bypass or historical source publication is fabricated.

Activation durably enqueues initial replay with its original baseline UUID as job UUID before
ACK. Bounded source pages and exact attempt-fenced checkpoints survive retry/restart. Current
source facts converge concurrent writes, including keys inserted behind a live scan cursor;
a live scan is not a repeatable snapshot or proof of absence. No Inventory absence provider
is selected. Changed delivery loads at least its committed owner version and ACKs only after
generic durable mutation persistence. The loaded stable source mutation identity is distinct
from an older notification identity; never bind a mutable current payload to one old inbox ID.

This is source-only continuation. Existing active baselines without canonical history require
explicit reconciliation; old migrations, event wire contracts and historical digest are not
rewritten. Backend/locking/restart/credential/runtime evidence and complete cross-client
operator recovery remain prerequisites for any Inventory Index/Product step-8 promotion.
