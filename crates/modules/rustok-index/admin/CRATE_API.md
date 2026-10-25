# rustok-index-admin / CRATE_API

## Public surface

### Components
- `IndexAdmin`: Root Leptos admin component providing the comprehensive index administration and diagnostics dashboard (schemas catalog, storage foundation, replay runner controls, inbox/job metrics, and query diagnostics).

### Public modules
- `core`: Pure Rust framework-agnostic view models (`IndexAdminOverviewViewModel`, `IndexStatCardViewModel`, `IndexSchemaRowViewModel`, `IndexTableRowViewModel`, `IndexSourceRowViewModel`), grid column definitions (`schema_columns`, `storage_columns`, `source_columns`), filter matchers, and label formatters.
- `model`: Transfer DTOs (`IndexAdminBootstrap`, `IndexTenantSnapshot`, `IndexModuleSnapshot`, `IndexStorageSnapshot`, `IndexTableSnapshot`, `IndexSchemaSnapshot`, `IndexInboxMetricsSnapshot`, `IndexJobMetricsSnapshot`, `IndexQueryDiagnosticsSnapshot`, `IndexOperationsSnapshot`, `IndexSourceDescriptorSnapshot`, `TriggerReplayInput`, `ReplayActionResult`, `CancelJobInput`, `CancelActionResult`, `RetryJobInput`, `RetryActionResult`).
- `transport`: Module-owned transport facade and server function callers (`fetch_bootstrap`, `trigger_replay`, `cancel_job`, `retry_job`, `IndexAdminTransportError`).

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) admin package for the `rustok-index` module.
- Server-side and client rendering via `leptos` and `leptos_axum` (monolith SSR / Hydration).
- UI tokens and components: Uses `rustok-grid` (`rustok-grid-leptos`) for data grids, paginated tables, column sorting, and filtering.
- Full localization: Fluent bundles (`locales/en.ftl`, `locales/ru.ftl`) integrated via `rustok-ui-i18n`.

## Domain Collaborations
- Integrates with backend `rustok-index` storage and replay engine (`PgIndexStorage`, bounded replay runner, reconciliation scheduler, dead letter inspector).
- Enforces platform RBAC permissions: `SETTINGS_READ` and `SETTINGS_UPDATE`.
- Tenant scope enforcement: `require_index_admin_tenant_scope` guarantees tenant equality before permission admission.

## Minimum Contract Set

### Input DTOs/Commands
- `TriggerReplayInput`: module, entity, version, and optional locale for triggering index replays.
- `CancelJobInput`: unique job identifier to cancel running replay or reconciliation jobs.
- `RetryJobInput`: unique job identifier and optional reason for retrying failed replay jobs.

### Domain Invariants
- Symmetric FFA boundary: `core/` contains 0 UI framework imports (`leptos::*`, `dioxus::*`); all formatting, view model mappings, and grid definitions are pure Rust functions verifiable via standard unit tests.
- UI components in `ui/leptos` contain zero business rules, domain validations, or direct database access.
- Tenant isolation: All admin queries and mutations require valid tenant context; cross-tenant access is rejected with `ApiError::Forbidden`.
- Replay and reconciliation jobs cannot be triggered with nil tenant or invalid job identifiers.

### Events / Outbox Side Effects
- Initiates index schema replays and worker reconciliation jobs.
- Dispatches job cancellation and retry signals to the background worker subsystem.

### Errors / Failure Codes
- `IndexAdminTransportError::NativeServer`: Transport or server communication failure.
- `ApiError::Unauthorized`: Missing or invalid authentication token.
- `ApiError::Forbidden`: Tenant mismatch or insufficient permissions (`SETTINGS_READ`, `SETTINGS_UPDATE`).
- `ApiError::Validation`: Nil tenant ID, nil job ID, or invalid replay schema parameters.
- `ApiError::NotFound`: Requested schema, job, or tenant does not exist.
- `ApiError::Internal`: Unhandled server or database error.
