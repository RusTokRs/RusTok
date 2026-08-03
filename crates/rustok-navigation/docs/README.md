# Navigation Module

## Purpose

Provide one owner for storefront navigation independent of page and commerce content owners.

## Responsibility Zone

The module owns `menus`, localized menu copies, nested items and active location bindings. A binding is identified by `(tenant_id, channel_id, location)`.

## Integration

GraphQL and HTTP use tenant and channel contexts resolved by the host. Storefront components are contributed through the generic `header_navigation` and `footer_navigation` slots.

## Translation target

`NavigationMenuTranslationTargetProvider` registers the exact
`navigation/menu` owner target. A resource is an entire menu locale aggregate:
the translated `menu_name` and one dynamically keyed `item:<uuid>:title` field
for every menu item. The provider exposes only exact source and target rows;
it never renders or applies fallback values.

`MenuService` owns apply. It validates the full item set, uses the menu
resource revision plus the source and target menu-locale revisions for CAS, and
updates the target menu row and all target item title rows in one transaction.
The shared owner-operation receipt ledger provides replay-safe apply identity.
`navigation_translation_changes` records content-free resource revision and
lifecycle evidence for cursor repair. Navigation has no generic menu-change
outbox event, so the journal must not be treated as one.

## Verification

Owner-run compilation, migrations, GraphQL schema generation and transport checks are required before release. The focused Translation target checks are:

- `cargo test -p rustok-navigation translation_target --lib`
- `cargo clippy -p rustok-navigation -p rustok-translation-targets --lib -- -D warnings`

## Related Documents

- [Implementation plan](implementation-plan.md)
- [Platform manifest contract](../../../docs/modules/manifest.md)
