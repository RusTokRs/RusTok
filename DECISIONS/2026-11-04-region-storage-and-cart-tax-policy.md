# Region storage ownership and Cart tax policy reads

- Date: 2026-11-04
- Decision status: Accepted
- Implementation status: Implemented
- Owners: Region and Cart
- Extends: [2026-09-18-canonical-native-module-reference-contract.md](./2026-09-18-canonical-native-module-reference-contract.md)
- Supersedes: [2026-03-25-commerce-module-split-product-pricing-inventory.md](./2026-03-25-commerce-module-split-product-pricing-inventory.md)
- Superseded by: None

## Context

Region owns regional configuration and its migrations. Shared Region entities
in `rustok-commerce-foundation` let Cart recreate the policy projection from
private tables. Cart reads that policy while updating its own tax lines and
totals in a database transaction. Country rules and the base policy must come
from the same database snapshot.

## Decision

Region owns private mappings for `regions`, `region_translations`, and
`region_country_tax_policies`. Consumer fixtures apply Region migrations and
create records through `RegionService`.

For native composition sharing a database, Cart calls
`RegionService::read_tax_policy_in_tx` with its current `DatabaseTransaction`,
the admitted Cart tenant, the selected region, and the saved channel. The
owner returns `RegionTaxPolicy`. A tenant-filtered left join reads the base
policy and country rules in one statement. Region resolves the channel
provider map into the typed projection. Tax owns provider validation and tax
calculation.

The caller retains transaction ownership. The query returns `None` for a
missing or foreign region and propagates `DbErr` for storage failure. Cart
preserves its existing missing-region validation and database error handling.
Both failures roll back the Cart command through its existing transaction.

This entrypoint describes the local shared-database deployment profile.
Separately deployed consumers use owner ports with explicit snapshot and
adoption semantics before replacing the local transaction boundary.

## Sources of truth and ownership

- `rustok-region` is the single canonical owner of regional configuration, `regions`, `region_translations`, and `region_country_tax_policies` tables.
- `rustok-cart` owns cart line items, cart discounts, and checkout tax calculations; it consumes regional tax policy via `RegionTaxPolicy` projection.
- `rustok-tax` owns tax calculation algorithms, nexus determination, and external tax provider integration.

## Invariants

- Region is the single owner of its persistence mappings and provider-map policy.
- Cart reads a typed projection within its active write transaction.
- Base rate and country rules use one statement and one tenant-scoped region.
- Channel overrides retain string, `provider_id`, and `provider` forms with the existing trimming, precedence, and blank-value behavior.
- Translation presence and fallback do not affect the tax projection.
- Region schema identities and historical migrations keep their persisted names.

## Non-goals

- Cart does not manage regional configuration or countries.
- Region does not compute checkout tax amounts or manage cart line items (owned by Tax and Cart).
- This decision does not alter external tax provider calculation contracts.

## Data, transaction, and concurrency boundary

In native composition sharing a database, Cart passes its active `DatabaseTransaction` to `RegionService::read_tax_policy_in_tx`. All reads occur within Cart's transaction isolation level. Database-enforced foreign keys and unique constraints in `rustok-region` migrations guarantee regional integrity. In distributed deployment profiles (FBA), an explicit asynchronous/cached snapshot port is used instead of direct in-tx execution.

## Context dimensions

- Tenant: Mandatory tenant filtering on all region and tax policy queries to eliminate cross-tenant data leakage.
- Channel: Channel overrides determine provider mapping precedence.
- Currency: ISO-4217 alphabetic currency code associated with the region.
- Locale: Localization used for region names, but does not affect numeric tax policy calculations.

## Events and projections

Region mutations produce domain events (`RegionUpdated`, `TaxPolicyChanged`) for cache invalidation. Projections do not mutate source records. Cart tax policy reads do not emit asynchronous events.

## Failure semantics

Missing region, tenant mismatch, or database storage errors fail closed and return `None` or propagate `DbErr`, causing transaction rollback of the Cart command.

## Migration and cutover

Region entities are removed from shared foundation (`rustok-commerce-foundation`), relocated into `crates/modules/rustok-region/src/entities/`. Consumer fixtures are updated to use `RegionService` and canonical migrations.

## Alternatives considered

- Direct SQL queries from Cart to `regions` table: rejected, violates module boundary and table sovereignty.
- Copying tax policies into Cart table: rejected, creates duplicate source of truth and state synchronization drift.

## Verification

Run scoped Region and Cart suites, including Cart provider override and discount cases. Verify missing and foreign regions, country rules, storage failures, and rollback of the Cart line and prior tax snapshot. Run the same owner tax projection scenario on SQLite and PostgreSQL. Check Commerce consumer compilation and the affected fixtures after removing public entities.

## Consequences

Cart depends on the Region owner crate for the native query. The shared foundation loses the Region entity API. Consumer fixtures use generated owner identities and the shipped schema. A remote profile needs a separate explicit adoption contract before it can provide the same workflow guarantees.
