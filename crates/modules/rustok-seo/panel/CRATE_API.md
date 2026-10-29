# rustok-seo-panel / CRATE_API

## Platform Capability Bindings

- **Runtime**: Leptos 0.8 / Client & SSR compatible
- **Transport**: GraphQL (`SEO_META_QUERY`, `UPSERT_SEO_META_MUTATION`, `PUBLISH_REVISION_MUTATION`, `GENERATE_SEO_META_QUERY`)
- **Localization**: Host locale detection, RFC 5646 normalization via `rustok_api::normalize_locale_tag`, English/Russian bilingual fallbacks
- **Entity Targets**: Dynamic target resolution via `rustok_seo_targets::SeoTargetSlug`

## Domain Collaborations

- **Upstream Module**: `rustok-seo` backend module and `rustok-seo-targets` contract library
- **UI Hosts**: `apps/admin` entity editors (products, pages, blog posts, categories, etc.)
- **Control-plane Integration**: Outbox event delivery monitoring, remediation issue mapping (`duplicate_canonical_url`, `missing_sitemap_candidate`, `fallback_only`)

## Minimum Contract Set

### Input DTOs/Commands
- `SeoEntityForm`: Client-side draft state for SEO fields, canonical URL, and JSON-LD schema
- `validate_target_id`: Validates non-empty, well-formed, non-nil UUID target entity identifier
- `derive_control_plane_widget_state`: Resolves widget state from connection inputs

### Domain Invariants
- **Strict FFA Boundary**: View-model calculation, score assessment, JSON-LD parsing, and target validation reside in `model.rs` and are verifiable with pure unit tests without a Leptos runtime.
- **Non-nil Identity**: All target entity IDs must be non-empty, valid UUIDs, and strictly non-nil (`00000000-0000-0000-0000-000000000000` is rejected).
- **Locale Normalization**: Entity locale inputs are trimmed and normalized through `rustok_api::normalize_locale_tag` before issuing GraphQL mutations.
- **JSON-LD Schema Type Normalization**: Structured data `@type` fields tolerate leading `@` and whitespace, normalizing consistently for schema matching.
- **Score Bounds**: Completeness score is clamped within `0..=100` and monotonically reflects field presence and length quality.

### Events / Outbox Side Effects
- Form saving dispatches `UPSERT_SEO_META_MUTATION` GraphQL requests.
- Revision publishing dispatches `PUBLISH_REVISION_MUTATION` mutations.
- AI metadata generation invokes `GENERATE_SEO_META_QUERY`.

### Errors / Failure Codes
- Target validation failures emit localized user-facing strings ("Target id is required.", "Invalid target id.").
- Host locale validation failures emit "Host locale is required." / "Invalid host locale.".
- Structured data parsing errors provide human-readable JSON diagnostics.
- Network and GraphQL transport failures are represented via `rustok_graphql::GraphqlHttpError`.

## Public surface

### Components
- `SeoEntityPanel`: Primary embedded SEO editor panel for entity edit screens
- `SeoCapabilityNotice`: Status banner for entity SEO support availability
- `SeoSummaryTile`: Key-value summary metric display card
- `SeoSnippetPreviewCard`: Live search-engine snippet preview card
- `SeoSchemaPreviewCard`: JSON-LD structured data preview card
- `SeoRecommendationsCard`: Completeness audit recommendations list
- `SeoDeliveryStatusCards`: Event delivery queue health metrics
- `SeoRemediationHintCard`: Contextual remediation action banner
- `SeoControlPlaneWidgets`: Composite widget cluster for admin control-plane status

### Types & Models
- `SeoEntityForm`: Client-side draft state for SEO fields, canonical URL, and JSON-LD schema
- `SeoMetaView`: Deserialized SEO metadata record projection
- `SeoRevisionView`: SEO revision counter view
- `SeoCompletenessReport`: Bounded completeness score (0-100) and recommendation list
- `SeoRecommendation`: Typed recommendation enum (title, description, canonical, OpenGraph)
- `SeoEventDeliverySummary`: Counter aggregation across delivery states (pending, sent, retry, failed, dead_letter)
- `SeoControlPlaneWidgetState`: Control-plane health state (loading, ready, empty, permission_denied, error)
- `SeoControlPlaneWidgetStateKind`: State discriminant enum
- `SeoRemediationAction`: Action enum (open_entity_editor, open_bulk_job, run_reindex)
- `SeoRemediationHint`: Remediation issue code, action, and explanation key

### Functions & Validators
- `validate_target_id(value: &str) -> Result<Uuid, String>`: Validates non-empty, well-formed, non-nil UUID target entity identifier
- `derive_control_plane_widget_state(has_target: bool, is_loading: bool, message: Option<&str>) -> SeoControlPlaneWidgetState`: Pure helper resolving widget state from connection inputs
- `remediation_hint_for_issue_code(issue_code: &str) -> SeoRemediationHint`: Maps diagnostic issue codes to prescriptive remediation actions
