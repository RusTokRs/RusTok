# Navigation Module

## Purpose

Provide one owner for storefront navigation independent of page and commerce content owners.

## Responsibility Zone

The module owns `menus`, localized menu copies, nested items and active location bindings. A binding is identified by `(tenant_id, channel_id, location)`.

## Integration

GraphQL and HTTP use tenant and channel contexts resolved by the host. Storefront components are contributed through the generic `header_navigation` and `footer_navigation` slots.

## Verification

Owner-run compilation, migrations, GraphQL schema generation and transport checks are required before release.

## Related Documents

- [Implementation plan](implementation-plan.md)
- [Platform manifest contract](../../../docs/modules/manifest.md)
