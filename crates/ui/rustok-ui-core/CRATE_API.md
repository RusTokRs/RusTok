# rustok-ui-core / CRATE_API

## Purpose

`rustok-ui-core` provides framework-agnostic route, input, busy-state, pagination, navigation, formatting, and presentation contracts shared across RusToK UI adapters (Leptos, Dioxus, Next.js/React) and module surfaces.

## Architecture & Responsibilities

- **Zero Framework Runtime Imports**: Strict framework-neutral core with zero imports of `leptos::*`, `dioxus::*`, React, or browser DOM types (`web-sys`).
- **Canonical Presentation Contracts**: Centralizes status badge tones, CSS hex color sanitization and accent classes, price/discount presentation, and navigation models.
- **Route Query Lifecycle & Sanitization**: Defines `UiRouteContext`, `UiRouteQueryUpdate`, `UiRouteQueryIntent`, and query application helpers (`apply_ui_query_pairs`) that enforce uniform query string encoding and legacy key stripping.
- **Data Navigation & Pagination State**: Encapsulates pagination offset/page calculations (`UiPaginationState`), bidirectional sorting state (`UiSortState`), breadcrumb hierarchy generation (`build_ui_breadcrumbs`), and module navigation group ordering (`build_ui_nav_groups`).
- **Selection & Filtering Primitives**: Provides `UiSelectionState` for tabular selection operations (toggle, all, partial) and structured query filtering rules (`UiFilterRule`, `UiFilterOperator`).

- **Zero UI Framework Dependencies**: Must contain no references or dependencies on Leptos, Dioxus, Next.js, or browser DOM types (`web-sys`).
- **Framework Agnostic**: All types must be serializable or plain Rust structs testable via standard unit tests (`#[test]`).
- **Canonical Presentation Logic**: Badge tones, pagination bounds, route query sanitization, and navigation structures must be centralized here to prevent frontend divergence.
- **Strict Query Sanitization**: Route query sanitization must preserve known keys while removing invalid or legacy query parameters according to declared schemas.

### Product dictionary selection

`AdminQueryKey::AttributeId` (`attribute_id`) and `OptionId` (`option_id`) belong to the Product
route schema alongside `ProductId`. An option key depends on the attribute key. Other known
route schemas reject these foreign selection keys. There is no generic `target_id` alias for
the dictionary and no default row selection. Recovery annotations remain independent document
evidence; sanitizing a nested selection is not permission to rotate an unresolved caller key.
## Public Surface

### Primary Data Types & Enums
- `UiBadgeTone`: Status tone enumeration (`Success`, `Warning`, `Danger`, `Info`, `Neutral`).
- `UiBreadcrumb`, `UiNavGroup`, `UiNavItem`, `UiChildPageEntry`, `UiNavigationEntry`: Navigation tree and breadcrumb models.
- `UiPaginationState`: Pagination state with offset, total pages, and clamping calculations.
- `UiSortState`, `UiSortDirection`: Sort field and direction (`Asc`, `Desc`) with toggle support.
- `AdminRouteQuerySchema`, `AdminQueryKey`, `AdminQueryDependency`: Route query schemas and dependency trees.
- `UiSelectionState`: Selection state tracker across item sets.
- `UiFilterRule`, `UiFilterOperator`: Search filter conditions (`Equals`, `Contains`, `StartsWith`, `In`, `Between`, `GreaterThan`, `LessThan`).
- `UiRouteContext`, `UiRouteQueryIntent`, `UiRouteQueryUpdate`, `UiRouteQueryWrite`: Framework-agnostic route query synchronization commands.

### Methods & Helpers
- `status_badge_tone(status: &str) -> UiBadgeTone`: Maps domain status strings to UI tones.
- `status_badge_class(tone: UiBadgeTone) -> &'static str`: Resolves Tailwind CSS classes for badge tone.
- `ui_badge_container_class(tone: UiBadgeTone) -> &'static str`: Container classes for tone badges.
- `normalize_css_hex_color(hex: &str) -> Option<String>`: Validates and normalizes 3- or 6-digit hex colors.
- `css_hex_accent_class(hex: &str) -> &'static str`: Maps arbitrary hex color to nearest Tailwind accent palette.
- `css_background_accent_class(name: &str) -> &'static str`: Maps legacy background envelope names to CSS classes.
- `format_ui_price(amount_cents: i64, currency: &str) -> String`: Formats monetary amounts for presentation.
- `format_ui_discount_badge(original_cents: i64, current_cents: i64) -> Option<String>`: Computes percentage discount badges.
- `build_ui_breadcrumbs(path: &str, titles: &HashMap<&str, &str>) -> Vec<UiBreadcrumb>`: Generates breadcrumb trail.
- `ui_href_is_active(href: &str, current_path: &str) -> bool`: Checks active navigation item highlighting.
- `apply_ui_query_pairs(current_url: &str, updates: &[(&str, Option<String>)]) -> String`: Encodes query replacements and removals.
- `parse_ui_csv(input: &str) -> Vec<String>`: Splits comma-delimited strings with trimming and empty filtering.
- `ui_busy_key(scope: &str, action: &str) -> String`: Generates deterministic busy-state identifier.

## Invariants & Guarantees

1. **Zero UI Framework Dependencies**: The crate MUST NOT depend on Leptos, Dioxus, React, router crates, or browser DOM APIs.
2. **Deterministic CSS Normalization**: Hex colors must be validated before palette mapping; invalid hex values fallback gracefully to neutral defaults.
3. **Pure Logic & Serialization**: All models implement `Serialize`, `Deserialize`, `Clone`, `Debug`, and `PartialEq` where appropriate, ensuring seamless serialization across boundaries.
4. **Stable Navigation Hierarchy**: Breadcrumbs and navigation groups produce deterministic hierarchies regardless of leading/trailing slash variants.
5. **Idempotent Query Serialization**: `apply_ui_query_pairs` strips duplicates, removes empty keys, and preserves unmodified parameters without mutating URL path structure.

## Dependencies & Boundaries

- **Upstream Dependencies**:
  - `serde`, `serde_json`: Universal serialization support.
  - `url`: Canonical URL parsing and parameter encoding.
- **Downstream Consumers**:
  - `rustok-ui-forms`, `rustok-grid`, `leptos-ui`, `rustok-ui-dioxus`, `rustok-ui-leptos`.
  - All module UI packages (`crates/modules/*/admin`, `crates/modules/*/storefront`).
- **Boundary Rules**:
  - UI framework adapters must only wrap or bind these models to framework signals/hooks.
  - No database, server, or transport dependencies.
