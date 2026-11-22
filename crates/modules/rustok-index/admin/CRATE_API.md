# rustok-index-admin / CRATE_API

## Public surface

### Components
- `IndexAdmin`: Root Leptos admin component providing the comprehensive index administration and diagnostics dashboard (schemas catalog, storage foundation, replay runner controls, inbox/job metrics, and query diagnostics).

### Public modules
- `core`: Pure Rust framework-agnostic view models (`IndexAdminOverviewViewModel`, `IndexStatCardViewModel`, `IndexSchemaRowViewModel`, `IndexTableRowViewModel`, `IndexSourceRowViewModel`), grid column definitions (`schema_columns`, `storage_columns`, `source_columns`), filter matchers, and label formatters.
- `model`: Transfer DTOs (`IndexAdminBootstrap`, `IndexTenantSnapshot`, `IndexModuleSnapshot`, `IndexStorageSnapshot`, `IndexTableSnapshot`, `IndexSchemaSnapshot`, `IndexInboxMetricsSnapshot`, `IndexJobMetricsSnapshot`, `IndexQueryDiagnosticsSnapshot`, `IndexOperationsSnapshot`, `IndexSourceDescriptorSnapshot`, `TriggerReplayInput`, `ReplayActionResult`, `CancelJobInput`, `CancelActionResult`, `RetryJobInput`, `RetryJobKind`, `RetryActionResult`).
- `transport`: Module-owned transport facade and server function callers (`fetch_bootstrap`, `trigger_replay`, `cancel_job`, `retry_job`, `IndexAdminTransportError`).

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) admin package for the `rustok-index` module.
- Server-side and client rendering via `leptos` and `leptos_axum` (SSR / hydrate); the CSR profile dispatches commands via `rustok-graphql`.
- UI tokens and components: Uses `rustok-grid` (`rustok-grid-leptos`) for data grids, paginated tables, column sorting, and filtering.
- Full localization: Fluent bundles (`locales/en.ftl`, `locales/ru.ftl`) integrated via `rustok-ui-i18n`.

## Domain Collaborations
- Integrates with backend `rustok-index` storage and replay engine (persisted schema/job snapshots and the selected `SharedIndexReplayRuntime`).
- Requires `SETTINGS_READ` for diagnostics and `MODULES_MANAGE` for job operations.
- Verifies active request tenant equality with authenticated actor scope before querying or modifying jobs.

## Minimum Contract Set

### Input DTOs/Commands
- `TriggerReplayInput`: module, entity, version, and optional locale for triggering index replays.
- `CancelJobInput`: non-nil job identifier for a replay job.
- `RetryJobInput`: non-nil job identifier, exact `Replay` / `Reconciliation` kind, and non-empty bounded audit reason. A job of a different kind is not requeued.

### Domain Invariants
- Symmetric FFA boundary: `core/` contains 0 UI framework imports (`leptos::*`, `dioxus::*`); all formatting, view model mappings, and grid definitions are pure Rust functions verifiable via standard unit tests.
- UI components in `ui/leptos` contain zero business rules, domain validations, or direct database access.
- Tenant isolation: All admin queries and mutations require valid tenant context; cross-tenant access is rejected before dispatch.
- Only the selected schema replay can be triggered; the dashboard does not create reconciliation requests.

### Events / Outbox Side Effects
- Runs a bounded replay chunk. Does not publish an independent acknowledgment or start a worker.
- Cancels replay or requeues an original failed job through the guarded runtime. Requeue appends an audit entry in the same transaction.

### Errors / Failure Codes
- `IndexAdminTransportError` is the shared `UiTransportError` for the selected path. Typed
  results distinguish busy, cancelled, scheduled retry and exhaustion from completion.
  Uncertain responses never trigger a command on another path.
- Native browser calls use `AuthorizedBrowserClient`; CSR GraphQL calls pass the captured
  bearer and tenant to `rustok-graphql`. Only the internal combined bootstrap requires a
  native endpoint in CSR.
- The server provides `IndexAdminReplayStop` for bounded native replay. Only the standalone
  SSR host configures `IndexAdminUpstream`; its relay uses a trusted API origin. See
  [Index operations](../docs/operations.md).
