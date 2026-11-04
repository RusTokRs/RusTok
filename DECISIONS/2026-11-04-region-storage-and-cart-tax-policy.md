# Region storage ownership and Cart tax policy reads

- Date: 2026-11-04
- Decision status: Accepted
- Implementation status: In progress
- Owners: Region and Cart
- Supersedes the Region entity-sharing part of [the initial commerce split](2026-03-25-commerce-module-split-product-pricing-inventory.md)
- Extends [the native module reference contract](2026-09-18-canonical-native-module-reference-contract.md)

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

## Invariants

- Region is the single owner of its persistence mappings and provider-map policy.
- Cart reads a typed projection within its active write transaction.
- Base rate and country rules use one statement and one tenant-scoped region.
- Channel overrides retain string, `provider_id`, and `provider` forms with the
  existing trimming, precedence, and blank-value behavior.
- Translation presence and fallback do not affect the tax projection.
- Region schema identities and historical migrations keep their persisted names.

## Verification

Run scoped Region and Cart suites, including Cart provider override and discount
cases. Verify missing and foreign regions, country rules, storage failures,
and rollback of the Cart line and prior tax snapshot. Run the same owner tax
projection scenario on SQLite and PostgreSQL. Check Commerce consumer compilation
and the affected fixtures after removing public entities.

## Consequences

Cart depends on the Region owner crate for the native query. The shared
foundation loses the Region entity API. Consumer fixtures use generated owner
identities and the shipped schema. A remote profile needs a separate explicit
adoption contract before it can provide the same workflow guarantees.
