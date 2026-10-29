# rustok-ui-core / CRATE_API

## Public surface

### Modules
- `badges`: status badge tones, CSS container classes, and tone resolution helpers.
- `css`: CSS class composers for accent colors, backgrounds, and hex color normalization.
- `money`: price and discount badge formatting helpers for presentation components.
- `navigation`: breadcrumbs, navigation groups, active route matching, module group ordering, and icon resolution.
- `pagination`: sort directions (`Asc`, `Desc`), sort state, and pagination state models.
- `route_selection`: admin route query schemas, query key dependencies, legacy parameter sanitization, and URL query mapping.
- `selection`: multi-criteria filtering rules (`UiFilterRule`), operators (`UiFilterOperator`), and selection state models.
- `ui`: shared route contexts (`UiRouteContext`), route query update models (`UiRouteQueryUpdate`), text normalizers, and busy-state key helpers.

### Primary Public Types & Enums
- `UiBadgeTone`: status tone enum (`Success`, `Warning`, `Danger`, `Info`, `Neutral`).
- `UiBreadcrumb`, `UiNavGroup`, `UiNavItem`, `UiChildPageEntry`: navigation models.
- `UiPaginationState`, `UiSortState`, `UiSortDirection`: pagination and ordering contracts.
- `AdminRouteQuerySchema`, `AdminQueryKey`, `AdminQueryDependency`: admin search/filter routing contracts.
- `UiSelectionState`, `UiFilterRule`, `UiFilterOperator`: entity selection and filtering models.
- `UiRouteContext`, `UiRouteQueryUpdate`, `UiRouteQueryIntent`: route query lifecycle models.

## Invariants

- **Zero UI Framework Dependencies**: Must contain no references or dependencies on Leptos, Dioxus, Next.js, or browser DOM types (`web-sys`).
- **Framework Agnostic**: All types must be serializable or plain Rust structs testable via standard unit tests (`#[test]`).
- **Canonical Presentation Logic**: Badge tones, pagination bounds, route query sanitization, and navigation structures must be centralized here to prevent frontend divergence.
- **Strict Query Sanitization**: Route query sanitization must preserve known keys while removing invalid or legacy query parameters according to declared schemas.
