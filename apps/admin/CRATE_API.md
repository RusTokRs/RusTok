# apps/admin / CRATE_API

## Purpose

`rustok-admin` (`apps/admin`) is the canonical Leptos 0.8 administration dashboard application host for RusToK. It serves as the primary operator UI host for platform administration, composing module-owned Leptos admin UI packages, enforcing security boundaries (CSP, nonces, strict transport headers), and hosting shared navigation, routing, and i18n contexts.

## Architecture & Responsibilities

- **Host & Composition Root:** Acts as the host container and composition root for all module-owned Leptos admin surfaces (`crates/modules/*/admin`). Modules register their admin navigation entries, pages, and dashboard widgets through the manifest-driven registry.
- **Security Boundary & Headers:** Implements `admin_security_headers` middleware and `CspNonce` lifecycle management. Enforces strict script-src, style-src nonces, frame-ancestors restrictions, nosniff, and HSTS in production. Isolates richtext editing iframe surfaces.
- **Routing & Navigation Hierarchy:** Provides primary admin routes (`/dashboard`, `/modules/:module_slug`, `/users`, `/profile`, `/security`, `/apps`, `/cache`, `/email`, `/design-system`) protected under `ProtectedRoute` and wrapped in `AppLayout`.
- **Locale & i18n Integration:** Provides reactive `AdminLocaleContext` and `AdminLocaleProvider` using `rustok-ui-i18n` catalog definitions for host UI messages (`en`, `ru`) while delegating module-specific translation to individual module packages.
- **Transport Symmetry:** Integrates with `apps/server` via dual-transport paths (native `#[server]` functions and GraphQL), ensuring parity with `apps/next-admin`.

## Public Surface

- `AdminLocaleContext`: Host-provided reactive locale signal wrapper with `.current()`, `.set()`, and `.translate()`.
- `AdminLocaleProvider`: Leptos component providing `AdminLocaleContext` to descendant view trees.
- `Locale`: Effective admin locale enum (`En`, `Ru`) with `from_effective_locale()` and `as_str()`.
- `use_admin_locale()`: Context hook retrieving the current `AdminLocaleContext`.
- `app::security::admin_security_headers`: Axum middleware generating nonces and injecting hardened security headers.
- `app::security::request_csp_nonce`: Extracts request-scoped `CspNonce` from Leptos render context.
- `app::security::validate_admin_security_profile`: Validates that production deployments require HTTPS/HSTS.
- `i18n`: `t()`, `format()`, `format_attribute()`, `validate()` for host-level messages.
- `app::modules`: Module administration registry types (`AdminSlot`, `AdminComponentRegistration`, `AdminChildPageRegistration`, `AdminPageRegistration`, `register_component`, `register_page`, `components_for_slot`, `page_for_route_segment`).
- `app::router::App`: Root Leptos application component.

## Invariants & Guarantees

1. **Security Isolation:** Standalone admin UI strictly enforces CSP with dynamic nonces; inline scripts and unsafe inline styles are forbidden in the main UI. Production mode requires HTTPS/HSTS configuration.
2. **Fail-Closed Module Registration:** If a module is disabled or not present in `enabled_modules`, its registered pages, nav items, and dashboard widgets are filtered out from the operator UI.
3. **i18n Validation:** Host catalog (`MESSAGES`) initializes with zero diagnostic errors across all supported locales (`en`, `ru`).
4. **Fluid Frontend Architecture (FFA):** The host container coordinates routing and shell layout without absorbing module-internal domain rules. Module pages remain sovereign.

## Dependencies & Boundaries

- **Upstream UI Primitives & Systems:** Depends on `leptos`, `leptos-router`, `rustok-auth-ui`, `leptos-ui`, `leptos-ui-routing`, `rustok-ui`, `rustok-ui-core`, `rustok-ui-i18n`, `rustok-ui-transport`, `rustok-graphql`, `rustok-web`.
- **Module Admin Packages:** Composes module admin crates (`rustok-*-admin`) via registered entrypoints and route manifests.
- **Zero Cross-Module Direct Access:** Never accesses internal databases or private module tables directly; all operations execute through public module endpoints or GraphQL queries.
