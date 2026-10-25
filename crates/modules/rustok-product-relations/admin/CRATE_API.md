# Crate API: rustok-product-relations-admin

## Minimum Contract Set
- **Fluid Frontend Architecture (FFA)**: Strict separation between framework-agnostic domain logic in `core` and Leptos view components in `ui`.
- **Pure Rust Domain Logic**: View-model construction, data filtering, UUID validation, and copy resolution are verified with standard unit tests without requiring a Leptos or browser runtime.
- **Dual-Path Transport**: Supports both direct native SSR server functions and headless GraphQL transport via `ProductRelationsTransportContext`.
- **Zero Fallbacks**: Transport routes fail explicitly; no silent fallback masks errors.

## Platform Capability Bindings
- **`rustok-grid`**: Declarative table and grid definitions (`relation_grid_columns`) with sorting and text filtering.
- **`rustok-ui-core`**: String normalization via `normalize_ui_text`.
- **`rustok-ui-i18n`**: Bilingual support (`en`, `ru`) for relations panel labels and actions.
- **`rustok-ui-transport`**: Dual transport execution via `execute_selected_transport`.

## Domain Collaborations
- **`rustok-product-relations`**: Interacts with the backend product relations service and DTO models.
- **`rustok-product`**: References product catalog IDs for source and target relation associations.

## Public Surface
### Core
- `ProductRelationsTransportProfile`: Transport profile enum (`Native`, `Graphql`).
- `selected_transport_profile`: Resolves transport profile from string option.
- `build_product_relations_panel_copy`: Resolves localized panel copy.
- `relation_grid_columns`: Builds grid column definitions for relations.
- `matches_relation_filter`: Evaluates relation filtering against query and relation type.
- `filter_relations`: Filters relation items by query and relation type.
- `validate_target_product_id`: Validates non-empty UUID string for target product.
- `is_valid_relation_type`: Validates relation type against supported variants (`cross_sell`, `up_sell`, `related`, `accessory`, `alternative`).

### Models
- `ProductRelationItem`: Serialized product relation entity DTO.
- `CreateProductRelationDraft`: Input draft for creating relations.
- `ProductRelationsPanelCopy`: Localized UI strings for the relations panel.
- `ProductRelationsAdminFilters`: Filter parameters for relation lists.
- `ProductRelationsAdminCommand`: Command enum (`Add`, `Remove`, `Reorder`).
- `ProductRelationsAdminCommandResult`: Result of executing an admin command.

### Transport
- `ProductRelationsTransportContext`: Transport execution context with profile, access token, and tenant slug.
- `load_product_relations`: Asynchronous loader for relations using selected transport.
- `execute_product_relations_command`: Asynchronous command executor using selected transport.
- `PRODUCT_RELATIONS_FALLBACK_POLICY`: Fallback policy declaration (`never falls back`).

### UI Components
- `ProductRelationsAdmin`: Standalone full-page admin view component for product relations.
- `ProductRelationsPanel`: Embeddable component for managing product relations within a product detail view.
