# rustok-seo-admin / CRATE_API

## Public surface

### Components
- `SeoAdmin`: Root Leptos admin component providing the comprehensive SEO control-plane dashboard (URL-owned `tab` routing: Index repair/replay, Bulk meta editor, Redirects, Sitemaps, Robots.txt preview, Defaults/Templates, and Diagnostics).

### Public modules
- `core`: Pure Rust framework-agnostic models, view-models, form states, input builders, and validators (`SeoAdminTab`, `SeoAdminBusyKey`, `SeoIndexReplayForm`, `SeoBulkFilterForm`, `SeoBulkActionForm`, `SeoRedirectForm`, `SeoSettingsForm`, `SeoTemplateForm`, `ROBOT_DIRECTIVE_PRESETS`, `trim_to_option`, `normalize_index_target_type`).
- `sections`: Admin section view components (`IndexSection`, `BulkSection`, `RedirectsSection`, `SitemapsSection`, `RobotsSection`, `DefaultsSection`, `DiagnosticsSection`).
- `transport`: Module-owned transport facade and server function callers for SEO admin operations.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) admin package for the `rustok-seo` module.
- Server-side and client rendering via `leptos` and `leptos_axum` (monolith SSR / Hydration).
- UI tokens and components: Uses `rustok-ui-core`, `leptos-ui-routing` (`UiRouteContext`), and design token primitives.
- Full localization: Fluent bundles (`locales/en.ftl`, `locales/ru.ftl`) integrated via `rustok-ui-i18n`.

## Domain Collaborations
- Integrates with backend `rustok-seo` services (`SeoMetadataService`, `SeoRedirectService`, `SeoSitemapService`, `SeoBulkService`, `SeoRoutingService`).
- Shared SEO target registry (`rustok-seo-targets`) for dynamically deriving bulk target kinds and display names.
- Enforces platform RBAC permissions: `SEO_READ`, `SEO_UPDATE`, `SEO_GENERATE`, `SEO_MANAGE`.
- Tenant scope enforcement: Enforces tenant equality before permission admission.

## Minimum Contract Set

### Input DTOs/Commands
- `SeoIndexRepairReplayInput`: target type, limit (1..=500), and replay_historical flag with explicit confirmation.
- `SeoBulkApplyInput`: bulk filter, selection, field patches, and explicit remediation modes (PreviewOnly, ApplyMissingOnly, OverwriteGeneratedOnly, ForceOverwriteExplicit).
- `SeoBulkExportInput`: filtered export parameters for tenant-scoped CSV artifact generation.
- `SeoBulkImportInput`: file artifact reference and import options.
- `SeoRedirectInput`: source path, destination URL, match type, and HTTP status code.
- `SeoModuleSettings`: tenant-level default robots, allowed canonical/redirect hosts, and fallback locale.

### Domain Invariants
- Symmetric FFA boundary: `core/` contains 0 UI framework imports (`leptos::*`, `dioxus::*`); all form validations, request builders, and preset matchers are pure Rust functions verifiable via unit tests.
- UI components in `ui/leptos` and `sections/` contain zero business rules, domain validations, or direct database access.
- Tenant isolation: All queries, mutations, and artifact downloads require valid tenant context.
- Safety confirmations: Historical index replay and repair execution require explicit affirmative confirmation before dispatch.
- Bulk preview safety: Preview mode never triggers metadata publishing side-effects.

### Events / Outbox Side Effects
- Queues background bulk apply, export, and import jobs.
- Dispatches sitemap regeneration and index repair/replay worker tasks.

### Errors / Failure Codes
- `ApiError::Unauthorized`: Missing or invalid authentication token.
- `ApiError::Forbidden`: Tenant mismatch or insufficient SEO permissions.
- `ApiError::Validation`: Invalid JSON patch, malformed redirect target, or unknown index target kind.
- `ApiError::NotFound`: Requested redirect, job, artifact, or settings record not found.
- `ApiError::Internal`: Unhandled server or transport communication error.
