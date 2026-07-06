# Admin App (Leptos) — Implementation Plan

## Focus

Bring `apps/admin` to a stable production level as a Rust/Leptos admin with strong UI/API contracts and observable client scenarios.

## Improvements

### Host composition update (2026-07-02)

- [x] Generated search page uses host-owned `SearchAdminComposition`, which connects public product metadata DTO/helper and search option props without importing owner internals.
- [x] Host passes effective locale from `UiRouteContext`, auth token and tenant slug, checks tenant enablement of the `product` module; no package-local locale fallback.
- [x] Product transport keeps native `#[server]` as the primary Leptos path and GraphQL as a parallel contract; fast search boundary guardrail fixes wiring without long Rust compilation.

### Architecture debt

- Remove residual compatibility paths after build verification (`src/components/`, `src/api/`, `src/providers/`, `src/i18n.rs`, `src/modules/`, `src/app.rs`) so live API matches current FSD structure.
- Complete FSD structure consolidation with minimized cross-layer imports.
- Eliminate business logic duplication between widgets/features and shared-integration layer.
- Build a unified set of UI primitives and reuse policy.
- Add the missing aggregate `widgets/user_table` instead of local tables/wrappers per page.

### API/UI contracts

- Stabilize the GraphQL operation contract and error typing in user forms.
- Synchronize UI behavior with `apps/next-admin` (loading/error/empty states).
- Standardize the localization contract for all new screens and system messages.

### Observability

- Add client-side UX flow metrics (critical actions, failures, latency).
- Propagate correlation id in requests to link with backend traces.
- Document frontend incident checklist for API degradation and auth flows.

### Security

- Introduce centralized permission guard checks at route and action level.
- Protect client forms from unsafe payload mutations and XSS injections in rich fields.
- Expand token control with explicit session storage/refresh policy.

### Test coverage

- Increase unit/component test coverage for shared UI and critical forms.
- Add e2e smoke scenarios for core admin workflows.
- Introduce contract checks for i18n keys and API-type drift.
- Get `cargo build -p rustok-admin` and `cargo-udeps --package rustok-admin` to green baseline without `cargo-udeps.ignore` for legacy UI/FSD remnants.

## Stack parity (Leptos/Next.js)

- Any feature for admin/storefront is planned, decomposed, and tracked for both implementations (Leptos and Next.js) in the same delivery cycle.

### Phase 1 update (2026-05-23): capability-first parity for page builder

`rustok-pages-admin` in `apps/admin` now has minimal page-builder surfaces on top of the existing backend contract `grapesjs_v1`:

- `preview` — contract-safe document preview from `body.contentJson`;
- `tree` — projectData tree + snapshot legacy blocks;
- `properties` — host-owned metadata (`locale`, `channels`, `template`, `body format`);
- `publish` — publishing via the same backend flow as the pages table.

### Must-have parity (required between `apps/next-admin` and `apps/admin`)

- Canonical write-path: `body.format = grapesjs_v1`, payload in `body.contentJson`.
- Legacy page compatibility: writing `body` does not automatically remove `blocks`.
- Unified capability model `preview/tree/properties/publish`.
- Unified write-path error pattern for rich/page-builder forms: `validation/sanitize/runtime`.

### Host-specific UX (allowed)

- Different layout and visual components (React/Leptos/Flutter host-specific).
- Different level of preview/tree "visualness" while maintaining the same capability contract.
- Different screen navigation packaging if backend payload and RBAC semantics remain unchanged.

### Feature readiness checklist

- [x] Implemented in Leptos variant.
- [x] Implemented in Next.js variant.
- [x] API/UI contracts match at capability level.
- [x] Navigation and RBAC behavior are equivalent for `pages` write/publish surfaces.

## FSD/UI follow-up backlog

- Close all cross-layer imports that violate the rule `pages -> widgets -> features -> entities -> shared`.
- Remove compatibility aliases and old import paths after confirmed build and smoke verification.
- Align shared UI/state contracts with `apps/next-admin` for loading, empty, error, and permission-gated scenarios.
- Establish a repeatable verification runbook for FSD layers and UI contracts together with RBAC/navigation checks.

### Current rich-text status (blog/forum/pages)

- **Admin (Leptos, `apps/admin`)**: [~] Partially implemented (module-owned `pages` got capability surfaces `preview/tree/properties/publish`, error UX `validation/sanitize/runtime` aligned for rich/page builder write-path).
- **Admin (Next.js, `apps/next-admin`)**: [~] Partially implemented (blog/forum use real Tiptap, pages migrated to GrapesJS + `grapesjs_v1`, needs parity discipline with Leptos and storefront rendering slice).
- **Storefront (Leptos SSR, `apps/storefront`)**: [ ] Not started (rich-text rendering parity for blog/forum/pages planned).
- **Storefront (Next.js, `apps/next-frontend`)**: [ ] Not started (rich-text rendering parity for blog/forum/pages planned).
