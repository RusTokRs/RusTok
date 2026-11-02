# rustok-product-bundles-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for product bundle and kit administration.
- Integrates with `rustok-grid-leptos`, `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-product-bundles` backend module for bundle CRUD, discount validation, and item compositions.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub mod core`: Pure Rust view-models, validation rules, discount calculators, and filter functions.
- `pub mod model`: Transport DTOs (`BundleAdminListItem`, `BundleAdminDirectory`, `BundleAdminRecord`, `BundleAdminFilters`).
- `pub mod transport`: Transport facade and context (`BundleAdminTransportContext`, `load_bundle_directory`).
- `pub use ui::leptos::ProductBundlesAdmin`: Root admin view component.

## Minimum Contract Set

### Input DTOs/Commands
- `BundleAdminFilters`: Admin pagination and search parameters.
- `BundleAdminCommand`: Bundle lifecycle and modification commands.
- `BundleAdminRecord`: Complete bundle definition including items and translations.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Bundle slug must follow lowercase kebab-case rules.
- Bundle name must not be blank.
- Percentage discounts are constrained between 0% and 100%.

### Events / Outbox Side Effects
- Admin UI adapter package; mutation commands delegate event publishing to `rustok-product-bundles`.

### Errors / Failure Codes
- Transport and validation failures produce typed `UiTransportResult` error variants.
