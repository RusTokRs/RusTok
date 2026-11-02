# Unified variant axis architecture

- Date: 2026-09-18
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-product` / Product platform owner
- Extends: `2026-07-01-product-category-bound-attribute-schemas.md`, `2026-07-11-product-storage-integrity-and-request-trust.md`, `2026-07-22-channel-binding-policy-boundary.md`
- Supersedes: None
- Superseded by: None

## Implementation note (2026-10-29)

Current source already uses the unified attribute/axis model. Policy-preserving bindings,
effective-policy impact checks, atomic sparse initial selection and existing-axis preview /
explicit tuple reassignment / CAS now have owner and both admin-client source implementations.
Compilation, PostgreSQL concurrency execution, mounted-client recovery verification, storefront
tuple selection and runtime retirement inventory remain outstanding. This is not a completed cutover or evidence that installed data
is disposable. See the [Product implementation plan](../crates/modules/rustok-product/docs/implementation-plan.md#variant-axis-continuation-2026-10-29).
The context below records the decision's original motivation, not a fresh installation inventory.

The [bounded admin combination browser](../crates/modules/rustok-product/docs/variant-combinations.md)
is also implemented in source as a local projection: explicit picks reuse existing create
commands, without automatically materializing the matrix or writing prices/stock. It is not a
new Cartesian batch command or a completed storefront/retirement cutover; runtime gates remain.
Attribute/option lifecycle now also has a selected-owner, retained-ID source implementation in
both admin clients; [§10c](#10c-attribute-and-option-definition-lifecycle) records its bounds,
recovery semantics and unexecuted release gates.

## Context

RusToK has two parallel systems describing what differentiates product variants:

1. A legacy option system: `product_options`, `product_option_values`,
   `product_option_translations`, `product_option_value_translations` tables, a
   `variant_option_values` junction table, and denormalized `option1`/`option2`/
   `option3` VARCHAR columns on `product_variants`. This model has a fixed
   three-axis limit and no connection to the canonical attribute ontology.

2. The canonical attribute ontology introduced in July 2026:
   `product_attributes`, `product_attribute_options`,
   `product_attribute_values`, `product_variant_attribute_values`,
   `product_variant_attribute_value_options`, with full EAV type validation,
   category-bound schemas, channel settings, localization, and PostgreSQL
   trigger-enforced invariants for option/attribute ownership and select
   cardinality.

The two systems share the same semantic space but use separate storage, separate
identities, and separate translation targets. This violates the
initial-implementation and zero-legacy policy and produces:

- duplicate vocabulary (a Color attribute and a Color option exist independently);
- inconsistent integrity (options lack type validation, scope, category policy);
- extra translation-target providers duplicating attribute-option targets;
- GraphQL/DTO surface pollution (`option1`/`option2`/`option3` alongside EAV);
- no database-enforced uniqueness of variant combinations.

The already-accepted ADRs establish `product_attributes` as the single attribute
dictionary and `primary_category_id` as the product-form source. This decision
unifies variant identity on that foundation.

**This decision supersedes** the `product_options`, `product_option_values`,
`option1`/`option2`/`option3`, and `variant_option_values` subsystem. It also
supersedes any documentation, guardrail, or test fixture that treats the option
system as a current or target contract.

## Decision

### 1. Canonical ontology

`product_attributes` and `product_attribute_options` are the single attribute
and discrete-value dictionary. No second vocabulary exists for variant-defining
dimensions.

### 2. Category schema policy

Category-attribute bindings (`category_attributes` and
`product_attribute_schema_attributes`) carry a `variant_axis_policy` with
values `forbidden`, `allowed`, and `required`, plus a separate
`default_variant_axis` boolean hint.

`variant_axis_policy` is a constraint: it determines whether an attribute
**may** (`allowed`), **must** (`required`), or **must not** (`forbidden`)
participate as a variant axis for products in that category.

`default_variant_axis` is a UX initialization hint: when an operator creates a
product in this category, axes marked as default are pre-selected. The operator
may add or remove allowed axes; only `required` is an enforced invariant.

The two dimensions are intentionally separate. Merging constraint and
initialization into a single enum conflates enforcement with convenience.
`default_variant_axis` must be `FALSE` when `variant_axis_policy` is
`forbidden`. For `required` policy, the hint is semantically redundant (the axis
will be present regardless) but is not prohibited; implementations may ignore it.

#### Effective policy precedence

When a category uses a reusable schema via `category_attribute_schema_assignments`,
the effective variant axis policy for a given attribute is resolved as:

1. If the category has a local `category_attributes` binding for the attribute,
   the local binding's `variant_axis_policy` and `default_variant_axis` are
   authoritative.
2. Otherwise, the schema-level `product_attribute_schema_attributes` binding
   provides the values.
3. If neither binding exists, the attribute is not in the effective schema and
   cannot be an axis.

There are never two competing sources for the same (category, attribute) pair.

### 3. Product-level axis configuration

Each product selects its concrete variant axes through
`product_variant_axes` and specifies the allowed discrete values for each
axis through `product_variant_axis_values`.

These tables are **configuration**, not assignment. They declare which
canonical attributes serve as identity dimensions for this product and
which canonical options are permitted for each dimension.

Eligibility for the initial implementation: only `active`, tenant-owned,
effective-schema attributes with `value_type` in `{select}` and `scope` in
`{variant, both}` may be configured as axes.

### 4. Actual variant assignment

The existing canonical variant EAV is the single storage for actual
per-variant attribute values:

- `product_variant_attribute_values` — one row per (variant, attribute);
- `product_variant_attribute_value_options` — option assignment for select values.

No second assignment system is introduced. A variant's identity-axis values
are a subset of its attribute values, distinguished by the presence of the
corresponding attribute in `product_variant_axes`.

### 5. Completeness

Every variant of a product with configured axes must have exactly one canonical
option for every configured axis and no additional identity-axis assignments.
This is an exact-match invariant, not a subset or superset:

```
variant identity axes == product configured axes
```

If a product has axes `{Color, Size}`, a valid variant contains exactly one
Color value and exactly one Size value. A variant missing Size, or carrying an
extra axis not in the product configuration, is rejected.

This invariant is enforced as a **deferred database constraint** (constraint
trigger with `INITIALLY DEFERRED`), evaluated at transaction commit. Deferred
validation is required because atomic axis reconfiguration may temporarily
produce an inconsistent intermediate state within a single transaction; the
invariant must hold at commit, not at each individual statement.

### 6. Combination identity

Canonical variant identity is the order-independent set of
`(attribute_id, option_id)` assignments for all configured axes.

The physical `combination_identity` column stores the canonical encoding:
sorted `attribute_uuid:option_uuid` pairs joined with `;`. Both UUIDs are
present so the physical representation matches the semantic identity exactly
and survives future model changes without relying on the current option →
attribute ownership lookup. Example:

```
0a1b2c3d-...:7e8f9a0b-...;4c5d6e7f-...:1a2b3c4d-...
```

`combination_identity` is **database-maintained derived state**: a canonical
PostgreSQL function computes it from the current variant EAV rows, and a
constraint trigger ensures it is always consistent. The application layer may
pre-compute for validation or display, but the database is the final authority.

For a product with **no configured axes**, exactly one variant is permitted
with `combination_identity IS NULL`. When axes are configured, every variant
must have a non-null identity. A partial unique index
`UNIQUE (product_id, combination_identity) WHERE combination_identity IS NOT NULL`
combined with a `UNIQUE (product_id) WHERE combination_identity IS NULL` index
prevents both duplicate combinations and multiple default variants.

Combination identity never replaces the variant UUID. Downstream consumers
(pricing, inventory, orders, index) continue to reference `variant.id`.

### 7. Sparse matrix semantics

Cartesian generation of variants from the axis value space is an explicit
operator command, not automatic domain behavior. A product may have a
configured matrix `Color:{Red, Blue} × Size:{S, M, L}` but only materialize
the variants `Red/M` and `Blue/L`. `Green` may appear in axis values without
any variant using it.

The direction of constraint is:

```
axis configuration
        ↓ constrains
actual variant combinations
```

Axis values are not derived from existing variants.

### 8. Atomic mutation

Axis configuration and affected variant consistency change atomically within
one domain operation. Intermediate states where configured axes and existing
variant assignments are inconsistent are never committed.

When an axis is added to a product with existing variants, the operation must
either provide values for all existing variants or be rejected. When an axis
value is removed and existing variants use it, the operation is rejected with
a structured diagnostic listing affected variant identifiers.

Axis reconfiguration is a single atomic domain operation that validates the
proposed configuration, validates or reconciles affected variants, and commits
the entire consistent state — or rolls back entirely.

### 9. Category transition

Changing a product's `primary_category_id` to a category whose schema creates
an axis-policy conflict is rejected with a precise diagnostic:

- `required` axis not in the new schema → reject;
- `forbidden` axis in the new schema, but the product has it configured → reject;
- the operation does not silently drop axes, truncate the matrix, or destroy
  variant-owned data.

### 10. Channel awareness

Attribute groups, attributes, and their storefront capabilities may be
channel-aware through channel-scoped overlays. Channel configuration must not
redefine canonical attribute identity, product variant axes, or variant
combination identity.

Channel-specific assortment is expressed by visibility and availability of
canonical groups, attributes, options, and variants — not by cloning or
redefining them. If a channel restricts a product's available Colors to
`{Black, White}` while the canonical axis permits `{Black, White, Red}`, the
`Red` variants remain valid canonical entities with stable identities; they are
simply not visible in that channel.

Attribute groups are a semantic grouping mechanism for admin and storefront
presentation. The channel-aware group model uses:

- `product_attribute_groups` — canonical tenant-scoped groups;
- `product_attribute_group_attributes` — attribute membership and ordering;
- `product_attribute_group_channel_settings` — per-channel visibility,
  ordering, and presentation overrides.

These extend the existing `product_attribute_channel_settings` pattern already
established for per-attribute channel overrides.

### 10a. Existing-axis preview and application contract

The existing `set_variant_axes` command is the only application path. Its owner-defined,
read-policy preview uses the selected Product command runtime as a planning capability;
remote providers must implement it or explicitly report unavailability, never use a local
service behind the selected provider's back. Preview does not create a receipt or persist data.

A new application supplies the proposal-specific predecessor token from preview and optional
explicit **full target tuples** for reassigned existing variants. Unmentioned variants retain
their current active EAV assignments for the proposed axes. The owner rechecks schema admission
and computes the same bounded plan under schema → Product locks before any data mutation.
The token covers the tenant/Product, document revision, axis configuration (including ordering),
complete variant membership/SKU identities, relevant EAV rows and the exact proposal. It deliberately does not
use the document revision alone: assignment-only writes need not update that revision. Labels,
stock and price changes are not combination identity. Relevant schema eligibility is revalidated
under the schema fence on both paths.

The synchronous contract is bounded to 256 existing variants, 32 axes and 256 allowed values
per axis; it returns all preview rows or refuses an oversized request, never a truncated plan.
Conflicts have 16 bounded entries with up to 8 variant UUID examples each and explicit more flags.
No Cartesian generation, SKU consolidation or deletion is implied. Removing an axis or allowed
value preserves ordinary EAV rows unless the operator explicitly reassigns that target axis.
Retained configuration row IDs and all variant UUIDs remain stable. Combination swaps are
staged with transaction-unique identities before updating EAV and final identities. Position
swaps use bounded nonnegative holes disjoint from predecessor and target positions, preserving
immediate uniqueness and historical nonnegative CHECK constraints. Changed
configuration/assignments advance the Product document revision and required index invalidation
in the same transaction as the existing ProductUpdated event and durable receipt. No-op plans
complete their receipt without a spurious revision/event.

The command response remains the existing axes array. New optional request fields are omitted
when absent/empty during canonical serialization so completed pre-preview receipts retain their
request hash and result shape. Completed receipts replay before new-application checks; an
uncompleted old request with no predecessor token is refused rather than silently rebased.
Client recovery must retain the exact token, tuples, scope, locale and caller key after an unknown
outcome. Next and Leptos now implement this in source; terminal axes receipts do not supply a
new document revision or authorize rebasing a dirty Product document. See the
[operator/recovery contract and unexecuted gates](../crates/modules/rustok-product/docs/variant-axis-reconfiguration.md).

### 10b. Standalone SKU caller recovery

Standalone creation uses the existing `create_variant` owner command and receipt namespace;
there is no second writer or receipt version. Before owner IO, both admin clients durably
retain the complete normalized request (tuple, SKU/barcode, shipping profile, all supplied
prices, inventory quantity/policy), tenant/actor/Product identity, original transport locale
and caller key. Unknown outcomes admit only replay of that saved request, never changed form
fields, a fresh key, another transport or another provider. A Product read or tuple membership
is not evidence that the command committed.

Leptos axes and standalone creation share an owner-local journal envelope and browser storage
adapter, not copied browser primitives. Existing axes storage keys, URL markers and serialized
pending records retain their shape. Each operation has its own typed state; a frozen SKU
request remains in its terminal record for inspection/correction. Next extends its existing
Product editor journal. Both clients fence other Product writes while recovery is unresolved
and expose recovery independently of the current Product projection. Missing/corrupt/changed
storage fails closed. Only a validated owner success acknowledgement or explicit durable
owner refusal settles a pending operation; admission, authentication, key conflicts and
transport failures do not. A terminal record requires an explicit new-intent action.

Native standalone creation authenticates the expected scope against host extraction and uses
the host-selected `ProductCatalogCommandRuntime`; GraphQL uses the same owner command and
metadata-preserving executor. The original receipt hash, response and stable Variant UUID
remain authoritative. This bounded cutover does not certify other lifecycle writers or
runtime recovery. Implementation progress and unexecuted gates remain in the Product plan.

### 10c. Attribute and option definition lifecycle

Dictionary lifecycle uses `ProductCatalogSchemaWritePort::update_attribute_definition` and the
existing Product receipt ledger. `option_id = null` targets the attribute; a non-null option ID
must belong to that same tenant/attribute. Update, archive and restore are explicit actions of
this one command, not deletion/recreation, EAV reassignment or a second option ontology.

UUIDs, codes, value type, scope and localization/storage shape are immutable in ordinary
presentation updates. The patch exposes exact-locale copy, presentation flags, search weight
and ordering. Validation/default/storage-shape changes require a separately validated migration;
they are not silently folded into a label edit. Omitted top-level members preserve current state;
a supplied translation object replaces the **complete exact-locale copy**, materializing omitted
nullable text members as null before caller persistence. A null/omitted translation does not
delete an existing translation. Missing exact copy is not a translated fallback waiting to be
written. Historical empty labels/negative positions remain readable; new supplied labels must be
nonempty and position/search-weight writes nonnegative.

An owner-issued lowercase SHA-256 state token binds tenant, entity, locale, immutable storage
identity, archive state and exact-locale presentation. An option token includes parent archival.
It is a **state fingerprint**, not a monotonic revision or Product document revision. Immutable
rules/defaults/metadata are not editable projection fields; archive rechecks their current impact.
Under schema -> attribute locks the owner reloads that state before applying a change. Translation
apply uses schema-shared -> parent `FOR UPDATE`; lifecycle reads take the schema-shared fence
and PostgreSQL parent `FOR SHARE` before exact-copy probes/fetches, and writes take the exclusive
schema fence and parent `FOR UPDATE`. Option creation and reference-creating binds/virtual rules
participate in that fence. SQLite uses the existing writer reservation; unsupported lifecycle
backends/capabilities are unavailable, never a local-provider fallback. No-op receipts complete
without spurious events. Attribute/OptionUpdated events, exact-copy Translation change journals
and actual lifecycle results commit atomically with the rows and receipt.

Archive inspects current canonical references: configured axes/allowed options, active **and
detached** Product/Variant EAV values, schema/category bindings, retained clone snapshots,
attribute groups/channel settings, option rules/defaults and virtual-category predicates. It
refuses rather than removing/rebinding them. Restore reactivates the same row and UUID; an option
requires its parent attribute to be active. Children are not recursively archived/restored. New
rule/binding/virtual-Eq references must identify active same-parent options. New select-default
metadata accepts null, a stable-code/UUID string or a bounded string array: explicit UUIDs cannot
reference foreign/archived options. Creation preserves deferred stable-code hints, since a new
attribute has no options yet; these hints are **not applied to EAV/SKUs**. Unknown historical
default/rule shapes are retained and cannot prove an option unused. No physical deletion or
historical migration rewrite is introduced.

Read impact is advisory; archive recomputes it under the write fence. Examples are bounded to
eight typed owner IDs plus an explicit more flag. All JSON sources share **1000 rows/4 MiB**, at
most 1 MiB per row, plus **1000 clone bindings across the inspection**, not per source/clone.
SQL length/window/row-number probes keep oversized and overflow/probe payloads null on the wire;
bounded raw text is decoded explicitly, and malformed/deep/future shapes make inspection
incomplete. Option rules include foreign-parent clone bindings and explicit UUID defaults, not
only currently valid references. Parent archive also collects at most 1000 retained child UUIDs
(with a 1001st overflow probe) and checks their foreign rules/defaults/clones using that same JSON
budget; it cannot bypass child protections by retiring the parent. Directory/options use a 1000+1
overflow check and refuse rather than truncate. Retained definition/copy/option text probes separately cap 65,536 combined bytes
per row and 8,388,608 per query. Oversized projection text is unavailable, not truncated copy.
Refusals (stale, in use, incomplete inspection, missing target, archived parent) are typed
**receipt results**. Replay returns the original refusal/ack even after later changes. Context,
auth, provider, storage, request/key mismatch and uncertain commit failures do not permit rotation;
the frozen command locale must match the port context before receipt admission.

`attribute_id` and optional `option_id` are shared typed Product query keys; an option requires
its parent key. There is no `target_id`/generic-ID alias or auto-select-first. For a missing
selected option the owner returns the actual parent/options with **null impact**, not parent
impact or a substitute target. Both clients retain that UUID visibly, disable its edits, and
allow an explicit different selection only in a Ready/fresh intent. Target changes are full
navigations retaining the existing recovery marker; explicit Fresh preserves the original
selected option. Fresh publishes that frozen nullable target **before** committing the next Ready
baseline, so failed full navigation cannot reload a different option. Directory failure does not
remove selected-target/recovery availability. Raw duplicate, orphan, noncanonical, rewritten
selection and literal `option_id=attribute` are rejected before baseline/IO; `attribute` is only an
internal UI sentinel. Browser journal selection is checked again after acquiring the write lock.

Both clients persist complete command/key/actor/tenant/attribute/original locale before IO using
their existing caller journal. Definition scopes/namespaces (`attribute_recovery`) are distinct
from Product/SKU scopes; deployed Product envelope/prefixes remain unchanged. Unknown permits
only exact replay, without new IDs, key, normalization or GET dependency. A terminal ack/refusal
must be persisted before explicit Fresh. A retained record hides/disables the new-command editor
and locks sibling authoring; saved owner impact takes precedence over late advisory reads.
Reads/replies are bound to live authenticated scope and target provenance. Native SSR/hydrate and
GraphQL project the same selected schema ports/key framing, with no post-uncertainty fallback.

This is a **source implementation**, not runtime certification. Owner/impact/native-core/shared
routing/Next journal+SDK+action regressions and an ignored PostgreSQL lifecycle/receipt/fence
contract are written but not executed. Rust compilation/macro expansion, PostgreSQL/SQLite SQL,
mounted clients, installed-state inventory and realistic snapshot/lock cost remain maintainer
gates. Storefront tuple selectors and downstream owner verification remain separate work.

### 11. Projection and index

Axis configuration is part of the Product projection for admin, storefront,
and search surfaces. Each indexed variant carries its axis-value pairs for
faceted search and storefront display.

Any change to axis configuration, allowed values, or actual axis assignment
invalidates the complete affected Product projection and the required Variant
projections atomically, using the existing `index_revision` bump mechanism.
Variant index projections must preserve **combination correlation**: the search
representation for a product's variants encodes the actual axis-value tuples,
not independent aggregated sets of `{colors}` and `{sizes}`.

Channel overlays for visibility/availability apply after canonical combination
semantics, not as a replacement.

### 12. Events and observability

Variant configuration mutations participate in the existing Product
transactional event and index-refresh architecture. Any change capable of
altering storefront variant selection or search semantics invalidates the
complete affected Product projection and the required Variant projections
atomically.

No parallel event mechanism is introduced. Whether axis configuration
changes emit a specialized domain event (e.g. `ProductVariantConfigurationChanged`)
or integrate through the canonical `ProductUpdated` root event with
specialized refresh-ledger entries is an implementation decision made at
execution time, consistent with the existing revision/ledger/outbox pipeline.

### 13. Database enforcement

All axis configuration tables use tenant-composite foreign keys in the style
of the existing product catalog schema:

- `(tenant_id, product_id)` references `products(tenant_id, id)`;
- `(tenant_id, attribute_id)` references `product_attributes(tenant_id, id)`;
- `(tenant_id, axis_id)` on axis values;
- `(tenant_id, option_id)` references `product_attribute_options(tenant_id, id)`.

Parent tables must expose the corresponding composite unique keys.

Option/attribute ownership within axis configuration
(`option.attribute_id == axis.attribute_id`) is database-enforced. Where a
composite foreign key can express the constraint declaratively, it is
preferred. Where it cannot (as with the existing
`rustok_product_validate_attribute_option` trigger pattern), a validated
trigger is used.

Axis eligibility constraints (effective schema membership, `select` value type,
`variant`/`both` scope, non-archived, non-forbidden policy) are enforced by the
domain service at mutation time; they are not row-level database triggers.

### 14. Ordering

`product_variant_axes.position` and `product_variant_axis_values.position`
use gap ordering (e.g. positions `0, 100, 200`) rather than dense sequential
integers. This avoids the need for deferrable unique indexes on position
columns and simplifies reorder operations: a swap or insert between existing
positions writes only the affected rows without temporarily violating
uniqueness. The unique constraint on `(product_id, position)` and
`(axis_id, position)` remains immediate.

### 15. Zero-legacy cutover

`product_options`, `product_option_values`, `product_option_translations`,
`product_option_value_translations`, `variant_option_values`, and the
`option1`/`option2`/`option3` columns on `product_variants` are dropped. All
repository-owned callers, transports, entities, DTOs, GraphQL types,
translation-target providers, tests, fixtures, seeds, scripts, and current
documentation are updated atomically. No compatibility layer, deprecated
alias, or dual-read path is retained.

Since RusToK is a pre-release initial implementation with no publicly deployed
migration history, pending migrations that create the legacy option tables are
amended directly to produce the target schema. No backfill migration is
created to import data from a model that has no external consumers.

## Sources of truth and ownership

- `rustok-product` owns canonical product attribute definitions, canonical
  discrete attribute options, product-level variant-axis configuration, variant
  identity semantics, and the actual variant EAV assignments.
- `product_attributes` and `product_attribute_options` are the only
  attribute/value ontology.
- `product_variant_axes` and `product_variant_axis_values` are configuration,
  not actual assignment.
- `product_variant_attribute_values` and
  `product_variant_attribute_value_options` remain authoritative for actual
  per-variant values.
- `combination_identity` is database-maintained derived state and never a
  second write authority.
- `rustok-channel` owns channel identity/resolution; Product owns Product
  channel overlays that reference canonical Product entities.
- Product/search/storefront projections are derived and rebuildable. Pricing,
  inventory, and orders continue to identify variants by stable `variant.id`.

## Invariants

### Allowed states

- A product may have zero or more eligible configured axes.
- A configured axis selects one canonical `select` attribute from the
  effective product schema.
- Axis allowed values are a product-owned subset of that attribute's canonical
  options.
- A product may materialize any sparse subset of the configured value space.
- Channel overlays may hide or reorder canonical groups, attributes, options,
  and variants without changing their canonical identity.

### Forbidden states

- A second ProductOption/ProductOptionValue ontology.
- An axis value whose option belongs to a different attribute.
- Cross-tenant Product/Attribute/Option/Axis references.
- A variant missing a configured identity axis or carrying an extra identity
  axis.
- More than one option for one configured select axis on one variant.
- Two variants of the same product with the same canonical combination.
- More than one default/no-axis variant for a product.
- Channel-specific axis identity or channel-specific combination identity.
- Silent axis/category changes that delete or invalidate business-owned variant
  data.
- Hash-only uniqueness as the source of variant-combination correctness.

## Non-goals

This decision does not:

- make Cartesian matrix generation automatic domain behavior;
- replace the stable variant UUID used by pricing, inventory, orders, or other
  downstream owners;
- move general custom-field semantics from the canonical Product attribute
  system into variant configuration;
- define channel resolution itself;
- create a second assignment table alongside canonical variant EAV;
- require every allowed axis value to have a currently materialized variant.

## Data, transaction, and concurrency boundary

Axis configuration changes, affected variant assignments, derived combination
identity, revision invalidation, and required Product event/outbox effects form
one atomic domain mutation where they are causally coupled.

PostgreSQL is the final authority for tenant referential integrity, option
ownership, exact combination uniqueness, no-axis uniqueness, select
cardinality, and deferred commit-time axis completeness. The service layer owns
effective-schema/policy eligibility and actionable conflict diagnostics.

Axis reconfiguration uses explicit optimistic/revision semantics from the
Product owner where concurrent writes can race; last-write-wins must not bypass
the canonical revision boundary. Retryable commands must either be naturally
idempotent or use the Product owner's canonical receipt/idempotency mechanism
when such a receipt is part of the surrounding write contract.

## Context dimensions

- **Tenant:** every canonical Product/Attribute/Option/Axis reference is
  tenant-scoped and database protected.
- **Primary category:** determines the effective attribute schema and therefore
  axis eligibility/policy.
- **Channel:** affects visibility, availability, ordering, and presentation
  overlays only; it never changes canonical variant identity.
- **Locale:** affects translated labels/copy, not attribute, option, axis, or
  combination identity.
- **Principal/auth/policy:** mutation authorization is resolved from canonical
  request context and is not accepted from Product input payloads.
- **Trace/correlation:** follows the existing Product write/event pipeline.

### 10d. Storefront SKU selection and cart handoff (2026-10-30)

- Buyer selectors project `variant_axes` and the actual variants' canonical
  `(attribute_id, option_id)` tuples. They never enumerate a Cartesian product,
  create a SKU or repair an absent tuple by choosing a different SKU. No axes
  means exactly one default SKU; configured axes start unselected unless a valid
  `variant_id` link supplies a real tuple.
- A complete SKU link uses `variant_id`. Partial/unavailable selections preserve
  `axis.<attribute_uuid>=<option_uuid>`; contradictory, duplicate, foreign or
  malformed selectors are refusals, not first-value defaults. `quantity` and
  the existing Pricing query keys retain the same meaning in both web clients.
- The bounded buyer projection supports at most 256 actual SKUs, 32 axes and
  256 allowed options per axis. An oversized or inconsistent projection refuses
  rather than truncating. This read bound does not delete installed SKUs or
  reduce the catalog write contract to a Cartesian matrix limit.
- Pricing owns selector validation and authoritative quotes. A SKU/quantity
  change invalidates the displayed quote immediately; stale or partial-error
  responses cannot enable purchase. Raw catalog prices are never purchase
  prices. A sole available catalog currency may initialize the display selector;
  it is still sent to Pricing for an authoritative quote.
- A price-list URL is a preview, not customer eligibility. Cart does not accept
  a buyer-supplied amount, source or arbitrary list identity. A list-scoped
  preview therefore cannot enable the ordinary cart command.
- Commerce owns the cross-owner add-to-cart workflow. The selected Product read
  port supplies public SKU/fulfillment/inventory projection; Pricing reprices
  the exact SKU and quantity in the cart's actual context. A buyer context
  expectation must match that normalized quote. Cart checks the context read
  for repricing again under its existing cart-row write lock before insertion.
  A mismatch does not mutate the cart or silently replace its context.
- The resolved public host tenant scopes browser request metadata. A missing canonical
  public tenant may be bound through the existing auth tenant key before refresh; an
  existing foreign tenant is refused, never overwritten. This does not issue a session,
  serialize a bearer/capability or replace server request authentication. Actor-bound
  recovery hints cannot authorize access or infer principal continuity from token rotation.
- These checks are not price reservations, a cross-owner transaction, stock
  reservations, durable cart command receipts or customer-group policy. Guest
  capability, authenticated customer ownership, tenant, locale and channel
  continue through the platform/owner context, not a Product-local session.

Implementation and unexecuted release gates are recorded in the
[Product plan](../crates/modules/rustok-product/docs/implementation-plan.md).

### 10e. Downstream SKU contracts (2026-10-30)

Stable Variant UUID remains the owner link across Pricing, Cart, Inventory and Order.
A missing/foreign/stale selected SKU refuses; an axis tuple, SKU text, label, position or
another available variant is never a substitute. Catalog changes do not recatalog immutable
Cart/Order/journal labels or prices and do not authorize deletion of persisted owner links.

Commerce aggregates **stock demand only** per UUID with checked i32 addition. Separate Cart/
Order line IDs, quantities, tier-price observations, metadata and recovery keys remain intact.
New checkout processing admits at most 1024 lines; this is not a historical storage/retention
policy. The direct and staged paths check the exact current Product/tenant/SKU, publication,
channel, fulfillment kind and effective shipping-profile snapshot, then correlate availability
with the requested SKU/aggregate quantity. Preflight is an observation, not a reservation.

Cart add reads Inventory for existing + new same-SKU demand in the canonical quote channel;
Product's global descriptive stock count/boolean is not quantity admission. Responses must
match the exact UUID/demand, and caller waiting is capped at two seconds. Local Cart backorder
math is removed. This is not a stock hold or selected remote Inventory certification; the
subsequent Cart-material-input fence is described in section 10f; current REST/GraphQL/native composition explicitly supplies the
in-process owner as staged checkout does. Per-line Pricing quantity is not changed by stock
aggregation.

Pricing remains the one money engine. Cart/checkout quote observations must match exact SKU/
parent, quantity, currency, region and saved Channel context. Arbitrary explicit Price List
handoff is refused; automatic-list provenance and global source rows can still satisfy a
channel-scoped request. Source-row scope is not the request context or an eligibility proof.
Pricing is observed before the Cart write lock; the plan is validated and applied under that
existing owner lock. This is not a cross-owner transaction or price hold. Recovery retains
frozen money. The shared Cart quote adapter uses saved selectors only, including global scope,
and saved-or-admitted-caller locale while retaining supplied caller metadata. It correlates
SKU/parent/quantity/context before price writes and does not introduce a second money engine.

The selected Product runtime carries its lean SKU identity capability alongside catalog reads.
Pricing receives that runtime explicitly; missing/failed external identity capability refuses
without a local Product substitution. Exact responses include tenant scope and preserve every
requested UUID in order and its optional expected parent. Pricing validates that projection
before reading tenant-less price rows. Catalog inspection uses the same selected Product
runtime for metadata and bounded identities; this does not turn an observation into a stock
hold, authorization grant or cross-owner transaction. Direct embedded construction remains an
explicit baseline, not a fallback after an external capability fails. Remote Pricing writes,
CAS/receipts and runtime certification are separate gates.

The Channel owner completes/validates explicitly saved ID/slug selectors. No selectors means
global, never a default/page context; errors never become omitted selectors. Commerce bounds
its wait to two seconds without claiming database cancellation. Saved representations remain
unchanged in Cart/journals. Nil guest actor IDs describe Commerce service delegation, not a
fabricated User or authorization; guest/customer ownership stays at the admitted boundary.

New durable inventory allocation probes 4096+1 candidate levels and refuses overflow. Only
active tenant-owned channel-visible locations remain candidates, including for backorders.
Existing matching receipts precede new eligibility and preserve their original location;
release is independent of current visibility. `allocated_channel_slug` is owner provenance on
new rows only, not a new identity dimension or historical backfill. Public Inventory port
paths use one admitted wrapper with private persistence construction. Numeric reservation
outcomes remain descriptive/global, not current scoped eligibility.

This source slice does not certify the full downstream chain: selected Product identity for
Pricing is composed in source, but real remote execution is still unverified. Missing read
capability and unsupported external SKU authoring refuse explicitly; native preview retains
the authenticated caller and proves tenant/SKU before price rows. Cart quantity/tier mutations,
Order/Fulfillment/shipment/return/cancellation effects, event/outbox/Index convergence, remote
write coordination and selected-runtime execution remain open. No historical migration or installed SKU/price/stock/history is rewritten or
retired. See the [Commerce plan](../crates/modules/rustok-commerce/docs/implementation-plan.md)
and owner plans for source regressions and unexecuted release gates.

### 10f. Cart quantity and priced-write adoption (2026-10-30)

The next bounded downstream slice strengthens the existing Cart pricing guard instead of
introducing a second revision table, price engine or stock policy. Code inspection found that
the public owner provider dropped the guard, batch repricing could scale an old tier quote by
a newer quantity, and quantity adapters still checked stock through older per-line helpers.

The Cart owner binds the observed tenant/Cart, saved price/access context, update witnesses
and exact line identity/quantity/money/fulfillment snapshot (at most 1024 observed lines), then compares it
under the existing Cart row lock before materializing a priced mutation. The public provider
retains that observation. Guest admission may bind current authorized capability evidence, but
must not replace the original content/context with a fresh successful snapshot after owner IO.
Marketplace Cart snapshot writers also acquire that same row lock before reading/inserting/
rebinding their line projections; read-only methods do not become writes. Bulk price updates explicitly carry quoted quantity and reject duplicate/unknown/stale lines
before any write. Priced insertion also refuses at its 1024-line operation capacity. Explicit
removal uses a target-only purpose/observation (not the priced-cart validity/size precondition),
so larger or invalid-price historical carts can be reduced; it cannot authorize priced writes. The unquoted storefront quantity-write operation is retired after migrating
its actual consumers; missing SKU identity cannot silently become stored-price scaling.

Commerce's shared quantity workflow uses selected owners, checked aggregate **replacement**
demand for Inventory and target-line quantity for Pricing. REST, GraphQL and native adapters
retain their existing public request shapes and use the same workflow/Cart adoption contract.
A decrement-to-remove retains its original observation, not a fresh post-race line quantity.
No automatic retry/key rotation is authorized by a conflict or retry hint.

This is a conservative Cart material-input fence, not a monotonic whole-Cart revision, durable
receipt, cross-owner transaction, price hold or stock reservation. At this quantity-adoption
boundary context patch and repricing were still separate; §10g closes that Cart-local split.
Remote Inventory/write protocols, retained unknown-outcome recovery and runtime/SQL/concurrency
execution remain separate gates. No
historical migration, stored line, price, stock, snapshot or journal is transformed or deleted.
See the [Cart contract](../crates/modules/rustok-cart/CRATE_API.md#priced-mutation-adoption-2026-10-30-source-continuation)
and owner plans for the source-only evidence; existing readiness labels are not promoted.

### 10g. Atomic Cart context/price adoption (2026-10-31)

Context patch followed by a separate repricing write is not an atomic Cart operation: owner
failure or a concurrent mutation could leave new context with old money. The context command
therefore becomes one owner-defined request with original observation and explicit pricing
intent. Buyer requests require a complete target-scope price plan; owner-only Preserve intent
may retain prices for a nonempty Cart only without changing the current unit-price basis
(currency, region, saved channel pair and locale). Country/shipping feed existing tax/shipping
recalculation rather than silently adding a second Pricing input model.

Commerce projects the target through Cart-owned normalization, obtains selected Pricing batches
grouped by expected Product parent and per-line quantity, and correlates exact scope/identity/
order/coverage. Grouping must retain expected parent proof before tenant-less money-row IO;
stock aggregation does not become a quantity tier. One inherited bounded wait budget covers
preparation. Unsupported/unpriced/malformed/error/timeout results never become stored-price
fallbacks. For Reprice, no Cart write precedes successful complete preparation. Variants are
deduplicated per parent/quantity group and chunked at Pricing's 256 bound before being fanned out
to distinct original lines. Preserve is explicit, never a fallback from failed preparation.

Under the existing Cart row lock the owner verifies the original observation and normalized
target scope/complete quantities, then writes context, line money, pricing-owned adjustments,
marketplace projections and Cart tax/shipping totals in one transaction. No Pricing IO runs
under that lock. Guest binding changes only authorized capability evidence, not original inputs.
Public REST/GraphQL/native request shapes remain unchanged and internal dual context DTOs are
replaced by one request. Direct availability-only checkout remains explicit: it cannot change
unit-price basis without a plan, and recovery reads frozen state before considering mutations.
The snapshot prepare adapter delegates to the existing atomic checkout owner operation instead
of begin-then-context writes; already-locked callers use read/recovery, not mutable preparation.

This is source implementation, not compiled/SQL/native/remote/concurrency proof or a distributed
transaction/hold/receipt. No historical migration, saved line/price/stock/snapshot/journal is
rewritten or deleted. Existing readiness labels and remaining outbox/receipt gates are unchanged.

### 10h. Inventory-owned reservation adoption into Order (2026-10-31)

Checkout reservation allocation and shipment consumption already belong to Inventory, but
Commerce's adoption service directly mutated Inventory reservation rows while maintaining its
own mapping journal. That bypasses the selected Inventory provider and its reservation/item locks,
can accept a wrong external identity or location, and races exact release/shipment.

The existing `InventoryReservationIdentityPort` therefore owns one bounded, explicit batch
`adopt_inventory_by_identity` command. It correlates tenant, checkout operation/Cart/Order,
reservation/external identity, original Cart-line UUID, stable Variant UUID, exact quantity,
original allocation location and target Order-line UUID. Duplicate/nil/oversized bindings refuse.
The location is an expected saved allocation, never a new selection. Archived/hidden allocations
remain eligible for exact adoption/replay; current Product publication and stock visibility are
not reinterpreted as permission to relocate a durable reservation.

Inventory locks the same items used by identity reserve/release, in deterministic order, and
locks/revalidates reservation rows to coordinate shipment mutations. It validates the entire
batch before any binding write, preserves quantities/levels/allocation/external identities and
source provenance, and commits the binding metadata/line IDs atomically. Released, consumed,
wrong-tenant, partially fulfilled or contradictory rows refuse. Exact already-adopted bindings
return their original result without rewriting timestamps or metadata.

Commerce passes its already-selected Inventory identity provider through the actual order-stage
composition, correlates the complete owner response before updating its own journal, and only
then checkpoints `order_created`. No foreign Inventory row read/write remains in that adoption
service. These are distinct owner transactions, not a distributed commit: an owner success followed
by a lost response/journal/checkpoint failure is recovered using the same immutable identities.
No single-row/local-provider fallback, inferred success, automatic key rotation or reallocation
is permitted. Owner errors keep their exact retry advice and sanitized public messages.
A false retry hint is not a no-effect receipt: an unverified nonretryable handoff is parked through
the existing lease-fenced `reconciliation_required` journal transition, with atomic existing
park/admission events. Retryable owner/transport failures and transient post-owner journal failures
retain the same operation, Order and reservation identities for explicit recovery. The final staged
storefront projection keeps this advice through journal/recovery wrappers without logging their
private causes; no new buyer DTO, lifecycle status or client key-rotation rule is introduced.

The existing identity allocator overwrites metadata `source` with `inventory_identity_port`;
new adoption preserves it. Known prior Commerce-adopted `checkout_operation` rows are accepted
only with the same complete target binding. The applied oversize-metadata compaction migration
retains reservation/external/Variant/Cart-line identity but drops Cart/operation labels. Its explicit
`metadata_truncated=true` permits only those absent/null labels, never contradictory retained data.
Commerce's immutable journal still owns/checks Cart and operation identity; missing historical labels
are not reconstructed or backfilled. Unknown malformed provenance refuses rather than being healed.
The current identity release writer and historical disposal/supersession migrations also own
terminal metadata markers. A positive apparently active row carrying non-null
`inventory_disposition`, `superseded_by`, `released_at`, `disposed_at` or `superseded_at` is
contradictory, not authorization to resurrect an allocation. Adoption refuses without clearing those
markers; missing/null markers remain valid for current allocations and known compaction.

Adding the target Order fields must fit the existing physical 32 KiB reservation-metadata limit.
Inventory prepares the full preserved metadata and measures the largest changed row in one bounded
backend-native projection before any binding update: PostgreSQL uses `octet_length(jsonb::text)`;
SQLite uses byte length of the JSON object text. A deterministic size refusal is nonretryable
`inventory.adoption_metadata_too_large`, not generic temporary storage unavailability. No automatic
truncation, payload dropping, historical repair or retry loop is allowed. Exact adopted replay does
not rewrite or resize unchanged metadata. Only the supported PostgreSQL/SQLite size representations
are accepted; another backend must expose an explicit unsupported refusal, not guessed capacity.
An inherited zero/missing deadline is rejected before invoking the selected owner; the same admitted
public wrapper covers both crate-root and `ports` factory/concrete entry paths.
A local journal/read/provenance error before this attempt's owner call does not prove that a prior
attempt never committed. All adoption-stage failures therefore retain bounded same-identity recovery
classification (transient local storage errors retryable, other unverified local failures parked).
The recovery service must preserve the original typed failure before its compensation lookup; a
successful reconciliation park must not replace that cause with a synthetic compensation failure.
Original bounded owner codes stay in journal/bounded diagnostics; existing buyer envelopes retain
their own public unavailable/reconciliation codes and static messages, not a new owner receipt.

Existing operational reservation/Cart-line/Order-line metadata and journals are live provenance,
not deletion candidates. No schema or historical migration is rewritten; no new receipt, stock
hold or reservation outbox publisher is introduced. Source regressions remain maintainer-run;
compilation, SQL/contention/crash recovery, mounted/remote provider parity and full downstream
shipment/cancellation/return/event convergence require separate retained evidence.

## Events and projections

Variant-axis mutations use the existing Product root-event, revision,
refresh-ledger, and transactional outbox architecture. No parallel event bus,
outbox, or variant-config publication mechanism is introduced.

Any mutation that can change storefront variant selection or search semantics
invalidates the affected Product projection and required Variant projections.
Projection/search representations must preserve actual combination correlation
and must be deterministically rebuildable from Product-owned authoritative
state.

## Failure semantics

- Invalid axis eligibility, option ownership, tenant ownership, duplicate
  combination, or completeness violations are rejected.
- A category transition that conflicts with configured axes is rejected with
  structured diagnostics.
- Removing an axis/value that is used by existing variants is rejected unless
  the caller invokes an explicit reconciliation/destructive command whose
  consequences are part of that command contract.
- Missing channel overlay data does not redefine canonical identity.
- Persistence, event, or revision failure rolls back the complete atomic
  mutation; no best-effort repair path is the correctness mechanism.

## Migration and cutover

The cutover is zero-legacy and repository-atomic. Pending unreleased Product
migrations are consolidated into the canonical target schema rather than
preserving a historical internal option model through backfill/fixup
migrations. Every repository-owned caller, transport, UI, fixture, seed,
translation target, verifier, and current document moves in the same cutover.

This ADR supersedes the legacy option subsystem as a target architecture, but
does not rewrite historical ADR text that records why earlier structures
existed.

## Alternatives considered

- **Keep `product_options` beside attributes:** rejected because it creates two
  competing vocabularies and duplicate localization/integrity paths.
- **Add a separate variant-axis assignment table:** rejected because canonical
  variant EAV already owns actual values and a second assignment store would
  create dual authority.
- **Use a hash as combination identity:** rejected because collision-free
  semantic uniqueness must not depend on a probabilistic accelerator.
- **Derive allowed axis values from existing variants:** rejected because
  configuration constrains variants; current materialization does not define
  the allowed space.
- **Make channels redefine axes/combinations:** rejected because stable variant
  identity must not depend on presentation/assortment context.
- **Automatically materialize the Cartesian product:** rejected because sparse
  variants are canonical and matrix generation is an operator convenience.

## Verification

Implementation is not complete until evidence proves, at minimum:

- tenant-composite foreign keys reject cross-tenant axis configuration;
- option/attribute ownership is database-enforced;
- deferred completeness accepts atomic reconfiguration but rejects an invalid
  committed variant;
- database-maintained `combination_identity` exactly matches canonical EAV;
- duplicate combinations and multiple no-axis/default variants are rejected;
- category policy and category-transition conflicts fail closed;
- destructive axis/value changes never silently remove variant-owned data;
- Product/Variant revision and projection invalidation occurs on all
  identity-relevant mutations;
- search/storefront projections preserve real variant combination correlation;
- channel overlays affect availability/presentation without changing identity;
- repository-wide stale-name scans find no executable/current references to the
  retired option subsystem after cutover.

## Consequences

- The `product_options` subsystem is eliminated; no code may reference it
  except as historical migration or ADR provenance.
- Variant identity becomes database-enforced and semantically rigorous.
- Translation-target providers for options are eliminated; canonical
  attribute-option translation targets cover the same vocabulary.
- GraphQL variant types lose `option1`/`option2`/`option3` and gain
  structured axis-value pairs.
- Admin UI replaces the option editor with an axis configuration interface
  backed by the effective category schema.
- Axis reconfiguration requires explicit operator decisions when existing
  variants are affected; silent data destruction is impossible.
- Category reassignment with axis-policy conflicts is fail-closed.
- Channel overlays compose on top of canonical identity without forking it.
- The three-axis limit is removed; the number of axes is bounded only by
  effective category schema and operational choice.
- Existing downstream identity contracts (pricing, inventory, orders) are
  unaffected because variant UUID remains the stable reference.
- Combination identity is always consistent with actual EAV state because
  the database maintains it as derived state.
- Cartesian matrix generation is a separate convenience operator command,
  not part of the core cutover.
