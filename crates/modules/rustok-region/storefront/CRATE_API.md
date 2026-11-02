# rustok-region-storefront / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA storefront UI components for region, country, and currency selection.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-region` module via native server functions and GraphQL.
- Collaborates with `apps/storefront` via route mounting and header region selectors.

## Public surface
- `pub use ui::RegionView`: Public storefront route component for region exploration and selection.

## Minimum Contract Set

### Input DTOs/Commands
- Region discovery and selection query parameters (`SELECTED_REGION_QUERY_KEY`, `RegionRouteState`).
- Storefront region summaries with countries, currency, and tax baselines.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Regional route state mapping is pure and framework-neutral.
- Missing selection falls back to the default or first available region.

### Events / Outbox Side Effects
- Read-only storefront exploration package; does not dispatch events or write mutations.

### Errors / Failure Codes
- Fallback evidence and error statuses (`native_unavailable`, `graphql_unavailable`) are preserved in error envelopes.
