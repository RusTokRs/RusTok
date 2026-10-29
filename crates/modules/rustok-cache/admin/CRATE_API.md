# Crate API: rustok-cache-admin

## Public surface

### Core
- `CacheAdminFormDraft`: Draft representation of cache settings form.
- `CacheAdminLabels`: Localized labels for admin interface.
- `CacheDiagnosticsViewModel`: View-model for cache backend health and metrics.
- `CacheModeOption`: Enum for supported caching modes (in-memory, redis).
- `build_cache_settings_payload`: Serializes cache settings draft into JSON string.
- `parse_cache_settings_json`: Parses platform settings JSON into draft form.

### Models
- `CacheHealthPayload`: Backend health payload.
- `GraphqlCacheHealthResponse`: GraphQL response envelope for cache health.
- `PlatformSettingsData`: Raw settings payload from platform settings service.
- `GraphqlPlatformSettingsResponse`: GraphQL response envelope for platform settings.
- `UpdatePlatformSettingsPayload`: Payload returned on settings update.
- `GraphqlUpdatePlatformSettingsResponse`: GraphQL response envelope for update operation.

### Transport
- `fetch_cache_health`: Loads cache health diagnostics.
- `fetch_cache_settings`: Loads current cache platform settings.
- `update_cache_settings`: Saves modified cache configuration.

### UI Components
- `CacheAdmin`: Root Leptos administration view for cache subsystem.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-cache`.
- Dual-transport execution (native server functions on SSR/hydrate, GraphQL queries on CSR/headless).
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).

## Domain Collaborations
- Collaborates with `rustok-cache` runtime engine and platform settings manager.
- Coordinates cache invalidation policies and connection verification.

## Minimum Contract Set

### Input DTOs/Commands
- `CacheAdminFormDraft`: Form draft holding mode, redis host/port/auth settings.
- `UpdateSettingsInput`: Settings mutation command with category and JSON string.

### Domain Invariants
- Redis connection settings are required when `CacheModeOption::Redis` is selected.
- Zero business logic inside UI components; validation and parsing reside in `core`.
- Settings mutations must be idempotent and preserve platform configuration integrity.

### Events / Outbox Side Effects
- Updates platform settings storage, prompting cache backend reinitialization upon change.

### Errors / Failure Codes
- Validation failures reported before mutation submission.
- Network and backend errors surfaced through standard UI alert components.

