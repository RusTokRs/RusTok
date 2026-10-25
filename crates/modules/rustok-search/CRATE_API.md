# rustok-search / CRATE_API

## Public surface

### Public modules
`analytics`, `audience_security`, `diagnostics`, `dictionaries`, `engine`, `graphql`, `ingestion`, `migrations`, `models`, `module`, `pg_engine`, `ports`, `presets`, `projection_source`, `projector`, `ranking`, `search_settings`, `storefront_category_scope`, `storefront_channel_authority`, `suggestions`.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `SearchModule` implementing `RusToKModule` (slug `"search"`, kind `ModuleKind::Core`) and `MigrationSource`.
- **Search Engine Selection & Abstraction**:
  - `SearchEngine` trait, `SearchEngineKind`, `SearchConnectorDescriptor`
  - `PgSearchEngine` default Postgres full-text search backend
  - Dynamic connector registry for external search engines
- **Ingestion & Projection**:
  - `SearchIngestionHandler` for domain projection events
  - `SearchProjector` and `SearchProjectionSource` registry
  - Cross-module projectors (`blog`, `forum`, `pages`, `docs`)
- **Search Ports & Transports**:
  - `SearchQueryPort` (`execute_search`)
  - `SearchSuggestionPort` (`suggest`)
  - Storefront channel authority & category scoping
- **Search Tuning & Diagnostics**:
  - `SearchSettingsService`, `SearchRankingProfile`
  - `SearchFilterPresetService`, `SearchDictionaryService`
  - `SearchAnalyticsService`, `SearchDiagnosticsService`

## Domain Collaborations
- **Core & Portability**: Integrates with `rustok-core` module system and `rustok-api` port context, call policies, and errors.
- **Content Projections**: Consumes event streams from `rustok-content`, `rustok-forum`, and other content modules to update projections.
- **Storefront & Channel Isolation**: Validates tenant and channel scoping against authoritative contexts before executing queries.

## Primary Public Types and Signatures
- `pub struct SearchModule`
- `pub struct PgSearchEngine`
- `pub trait SearchEngine`
- `pub trait SearchQueryPort`
- `pub trait SearchSuggestionPort`
- `pub struct SearchQuery`
- `pub struct SearchResult`
- `pub struct SearchSuggestionQuery`
- `pub struct SearchSuggestion`
- `pub struct SearchSettingsRecord`

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-api`
- `rustok-content`
- `rustok-events`
- `rustok-telemetry`

## Common AI Mistakes
- Exposing raw `SearchQueryPort` directly to unauthenticated public storefront endpoints instead of going through storefront channel authority.
- Bypassing tenant filtering on search queries or projection sweeps.
- Manually parsing search queries without respecting search settings or synonym dictionaries.
- Using wildcard re-exports instead of explicit named imports.

## Minimum Contract Set

### Input DTOs/Commands
- `SearchQuery` (`query`, `tenant_id`, `locale`, `facets`, `filters`, `page`, `page_size`, `sort`)
- `SearchSuggestionQuery` (`tenant_id`, `query`, `locale`, `limit`, `channel`)
- `SearchAudienceRequest` (`tenant_id`, `locale`, `candidates`, `auth`, `request_context`, `transport`)
- `SearchAudienceDocumentScope` (`gated_source_modules`, `authorized_document_ids`)
- `StorefrontSuggestionAudience` (`registry`, `auth`, `request_context`, `transport`)
- `StorefrontSearchCategoryScopeRequest`

### Domain Invariants
- Strict tenant isolation: Search requests cannot cross tenant boundaries; requests with nil or mismatched tenant IDs are rejected with `PortError::validation` or `PortError::forbidden`.
- Channel authority: Storefront searches must validate channel eligibility through `TrustedStorefrontChannel`.
- Safe projection ingestion: Documents are idempotently ingested and updated through projection sources without data corruption on event replay.
- Declared corpus: every source module is declared exactly once as `SearchAudienceClass::Public` or `SearchAudienceClass::Gated`; duplicate declarations are rejected and an undeclared module is never publicly discoverable.
- Fail-closed boundaries: storefront GraphQL and native SSR reads require a composed `SearchAudienceRegistry` and return an error when it is missing; host composition propagates registration failures.
- Authorized aggregation: totals, facets, ordering and pagination are computed by PostgreSQL over the authorized row set; facets are never re-aggregated outside the engine.
- Pinned query rules may reorder an authorized page and may only inject an extra document when no gated module is in scope.
- Owner-declared category scope: `StorefrontSearchCategoryScopePort` declares `source_module()` and `expand_category_scope()`; the resolver consults it only when the request is scoped to exactly that module, and otherwise keeps the caller's exact `category_ids`. The engine has no per-module predicate.
- Bounded candidate work: one policy call receives at most `MAX_SEARCH_AUDIENCE_CANDIDATES` candidates, scan pages are bounded by `AUDIENCE_CANDIDATE_SCAN_PAGE_SIZE`, and one request scans at most `MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS` gated documents. There is no second, legacy bound: the retired Forum storefront port kept its own `MAX_FORUM_SEARCH_RESULT_CANDIDATES` constant and it is gone.

### Events / Outbox Side Effects
- Event ingestion handlers subscribe to content lifecycle events (`created`, `updated`, `deleted`) to schedule or execute projection updates.
- Analytics queries record query performance and zero-result metrics for monitoring.

### Errors / Failure Codes
- `PortError` with error kinds `validation`, `not_found`, `conflict`, `forbidden`, `unavailable`, `timeout`, and `invariant_violation`.
- Audience policy errors keep their taxonomy: `NotFound`/`Forbidden` exclude the module and keep the request successful; `Unavailable`/`Timeout` become operational errors; `Validation`/`Conflict` stay caller errors; `InvariantViolation` becomes an internal error.
- `StorefrontChannelAuthorityError` and `ForumSearchContractIngressError` for domain-specific contract violations.
- A policy that returns an unrequested id, a duplicate id, or more ids than it received is an invariant violation, not a partial result.
