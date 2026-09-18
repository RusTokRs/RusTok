# Unified variant axis architecture

- Date: 2026-09-18
- Status: Accepted

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
