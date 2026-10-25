# Crate API: rustok-pages-storefront

## Minimum Contract Set
- **Fluid Frontend Architecture (FFA)**: Strict separation of framework-agnostic models, page link formatting, empty state structures, and content summarization in `core` from Leptos template rendering.
- **Pure Rust Domain Invariants**: Verification of page title/slug fallbacks, count labels, and format summarization via unit tests without Leptos or DOM runtime.
- **Dual-Path Transport**: Seamless routing and content resolution through both native server functions and headless GraphQL transport adapters.
- **Deterministic Landing Artifact Projection**: High-performance rendering of static landing artifacts (`STATIC_LANDING_PAYLOAD_BODY_FORMAT`) alongside dynamic Fly page builder documents without host DOM leakage.

## Platform Capability Bindings
- **`fly` / `rustok-page-builder-storefront`**: Core layout engine, static landing renderer (`StaticLandingStorefront`), and dynamic page builder storefront views (`PageBuilderStorefront`).
- **`rustok-ui-core`**: Core UI traits, normalized view-models, and design token integration.
- **`rustok-ui-i18n`**: Bilingual support (`en`, `ru`) for published page directory headers and empty states.
- **`rustok-content`**: Article profile rich-text rendering via `PagesRichTextRenderer` for rich-text component blocks.

## Domain Collaborations
- **`rustok-pages`**: Consumes published page details, localized page translations, and canonical slug routes.
- **`rustok-page-builder`**: Conforms to canonical Page Builder document specifications (`PAGE_BUILDER_DOCUMENT_FORMAT`).

## Public Surface
### Core
- `selected_page_title`: Extracts localized title or falls back to default.
- `selected_page_slug`: Extracts localized slug or falls back to default.
- `selected_page_effective_locale`: Resolves effective page locale.
- `summarize_page_content`: Generates content summary using format-aware summarizer.
- `selected_page_empty_state`: Builds empty state view model for unselected or missing pages.
- `raw_body_format_summary`: Formats body format name and length into display template.
- `count_label`: Interpolates item count into localized template string.
- `page_link_href`: Constructs canonical route URI query for target page slug.
- `storefront_page_list_item_view`: Assembles complete view item for storefront page listings.

### Page Builder & Landing Rendering
- `is_page_builder_body`: Checks whether page body format matches supported builder or artifact formats.
- `decode_page_builder_body`: Decodes JSON document for dynamic page builder rendering.
- `decode_static_landing_payload`: Decodes static landing payload (HTML and scoped CSS).
- `PageBuilderPageBody`: Leptos component dispatching between static landing artifacts, dynamic page builder, and fallback projections.
- `PAGE_BUILDER_DOCUMENT_FORMAT`, `STATIC_LANDING_PAYLOAD_BODY_FORMAT`, `STATIC_LANDING_URL_BODY_FORMAT`: Canonical format constants.

### Transport & Routing
- `StorefrontPageRouteDecision`: Routing resolution outcome (`Landing`, `NotFound`, `Redirect`).
- `StorefrontPageRouteDisposition`: Routing disposition details.
- `resolve_storefront_page_route`: Resolves incoming storefront request route to published page artifact.

### UI Components
- `PagesView`: Primary Leptos storefront component providing published pages navigation and detail rendering.
