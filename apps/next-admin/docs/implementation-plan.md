# Next Admin App — Implementation Plan

## Focus

Strengthen `apps/next-admin` as the primary admin UI with contract synchronization with the backend and a unified operational quality baseline.

## Improvements

### Architecture debt

- Complete FSD structure normalization and restrict import dependencies between layers.
- Centralize data-access/auth integrations in `shared` to eliminate copy-paste across pages.
- Simplify widget reuse across admin sections.
- Remove legacy import paths after passing type-check/build, so canonical `shared/*`, `entities/*`, `widgets/*` become the only live API.

### API/UI contracts

- Align GraphQL/REST response contracts with `apps/server` for critical admin scenarios.
- Establish unified UX patterns for tables, forms, notifications, optimistic updates.
- Synchronize RBAC navigation and action-level permissions with backend policy.

### Observability

- Add client-side telemetry events for critical admin flows.
- Propagate trace/correlation identifiers in backend calls.
- Define SLIs for UX: screen load time, submit success rate, recoverable error frequency.

### Security

- Strengthen client route/action protection via RBAC guards and fail-closed behavior.
- Add secure token/session handling and audit of sensitive operations.
- Verify CSP/XSS/CSRF measures for admin forms and rich content inputs.

### Test coverage

- Expand e2e coverage for critical sections (auth, users, content, settings).
- Add contract tests for API mapping and typed client validation.
- Increase unit/component coverage for shared UI and form logic.
- Keep `pnpm --filter next-admin type-check` and `pnpm --filter next-admin build` at green baseline after every FSD/UI structure change.

## Blog/Forum rich-text (Tiptap) and Pages GrapesJS Builder readiness

- [x] Production post form uses real Tiptap-based `RtJsonEditor` and serializes rich-text to canonical `rt_json_v1`.
- [x] Separate routes added for scenarios:
  - `/dashboard/blog/page-builder` for the visual `GrapesJS` `PageBuilder` (page functionality inside blog menu).
  - `/dashboard/forum/reply` for `ForumReplyEditor` (`rt_json_v1`) inside forum menu.
- [x] `ForumReplyEditor` uses the same Tiptap-based `RtJsonEditor` and the same `rt_json_v1` contract as the blog production CRUD-flow.
- [x] Placeholder IDs replaced with real entity selection (page/topic selectors) via live GraphQL queries.
- [x] `PageBuilder` saves pages in canonical body format `grapesjs_v1`; legacy `blocks` remain read-compatible until a separate storefront migration slice.

## Stack parity (Leptos/Next.js)

- Any feature for admin/storefront is planned, decomposed, and tracked for both implementations (Leptos and Next.js) in the same delivery cycle.

### Capability-first parity contract (Phase 1, 2026-05-23)

Must-have parity between `apps/next-admin` and `apps/admin`:

- unified backend payload contract `grapesjs_v1` (`body.format`, `body.contentJson`);
- capability surfaces `preview/tree/properties/publish` on both host stacks;
- publish/write actions and compatibility rules for legacy `blocks/body` do not diverge;
- unified write-path error UX pattern (`validation/sanitize/runtime`) for rich/page-builder forms;
- for page-builder save/publish errors, Next Admin uses the same typed catalog as `rustok-pages`: `validation`, `sanitize`, `runtime`, `feature-disabled` (`FEATURE_DISABLED`) with operator-guidance for disabled publish capability.

Host-specific UX that is allowed without drift:

- different visual components, layout and interaction within capability surface;
- different depth of visual tree/preview with unchanged payload contract;
- different route-shell composition if RBAC/navigation semantics match.

### Feature readiness checklist

- [x] Implemented in Leptos variant.
- [x] Implemented in Next.js variant.
- [x] API/UI contracts match at capability level.
- [x] Navigation and RBAC behavior are equivalent for `pages` write/publish surfaces.

## FSD/UI follow-up backlog

- Clean up compatibility imports from `components/`, `lib/`, `hooks/` and migrate consumers to canonical FSD-layer paths.
- Align widget/shared boundaries for tables, form shells and app-shell compositions.
- Complete parity-check with `apps/admin` for loading/error/permission-gated UX and navigation contract.
- Maintain `@iu/*` and `UI/docs/api-contracts.md` as source of truth for cross-stack UI API.

### Current rich-text/blog-forum and GrapesJS pages status

- **Admin (Leptos, `apps/admin`)**: [~] Partially implemented (`pages` got capability surfaces `preview/tree/properties/publish`, rich/page-builder write-path errors aligned to a common pattern).
- **Admin (Next.js, `apps/next-admin`)**: [~] Partially implemented (production blog/forum already use real Tiptap-based editor and canonical `rt_json_v1`, pages migrated to `GrapesJS` + `grapesjs_v1`, forum flow uses live entity selection, parity-check with storefront rendering slice remains).
- **Storefront (Leptos SSR, `apps/storefront`)**: [ ] Not started (rich-text rendering parity for blog/forum/pages planned).
- **Storefront (Next.js, `apps/next-frontend`)**: [ ] Not started (rich-text rendering parity for blog/forum/pages planned).
