# Mobile Admin and Storefront Separation: Dedicated Business Operator Host Without System Superadmin Settings

- Date: 2026-11-12
- Decision status: Accepted
- Implementation status: Implemented
- Owners: platform-core, rustok_mobile
- Extends: [2026-03-07-deployment-profiles-and-ui-stack.md](./2026-03-07-deployment-profiles-and-ui-stack.md), [2026-03-17-dual-ui-strategy-next-batteries-included.md](./2026-03-17-dual-ui-strategy-next-batteries-included.md)
- Supersedes: None
- Superseded by: None

## Context

The RusTok mobile workspace (`rustok_mobile`) provides mobile clients for the platform on iOS and Android.
As the platform expands to support both customer-facing marketplace surfaces and business-facing operational management (CRM, orders, merchant catalog, courier, warehouse), a structural question arose:
Should business management be mixed into the customer storefront application (`rustok_frontend_mobile`), or should it remain a distinct host? Furthermore, what constitutes "admin" on mobile devices?

1. **Anti-Pattern: Monolithic Frontend Entanglement**:
   Attempting to merge customer-facing storefront flows (smooth marketplace catalog, cart, promotional feeds, customer checkout) with business operator flows (CRM lead funnels, order fulfillment, inventory adjustments, merchant analytics) inside a single mobile host creates tight coupling, bloated bundle size, conflicting UX paradigms, and release bottlenecks.
2. **Anti-Pattern: Replicating System SuperAdmin Plumbing on Phones**:
   Early mobile prototypes attempted to duplicate low-level server engine management (such as toggling platform modules, inspecting infrastructure failure compensation hooks, and configuring system migrations) in Flutter. These technical DevOps controls are strictly desktop-first and increasingly automated via AI agents (`rustok-ai`, MCP tools, CLI).
3. **The Odoo Pattern (Business Management vs Customer Frontend)**:
   Modern business platforms (such as Odoo, Kaspi, Shopify, and Ozon) separate the public customer storefront from the business operator suite. The business suite is not a "system devops console", but an operational CRM/ERP workplace for merchants, managers, and fulfillment workers.

## Decision

1. **Preserve Clean Host Separation**:
   - `apps/rustok_frontend_mobile` is the dedicated customer-facing Storefront / SuperApp host.
   - `apps/rustok_admin_mobile` is the dedicated business-facing Operator / Admin host (CRM, fulfillment, merchant management).
   - Under no circumstances shall customer storefront code and business administration workflows be entangled into a single monolithic UI package.
2. **Strict Exclusion of System SuperAdmin Settings from Mobile**:
   - `rustok_admin_mobile` is explicitly scoped to **business operations, CRM, merchant tools, and fulfillment workflows**.
   - Technical system settings (module toggling, server migrations, engine lifecycle hooks, low-level RBAC schema editing) are **strictly excluded** from mobile clients and reserved for desktop web consoles (`apps/next-admin`, `apps/admin`) and AI agent control plane (`rustok-ai` / MCP).
3. **Dual Surface Codegen Support**:
   - Platform module manifests declare `provides.storefront_ui` and `provides.admin_ui`.
   - Tooling scripts (`generate_mobile_manifest.py`, `verify_mobile_manifest.py`, `check_mobile_codegen.py`) support both `--surface storefront` and `--surface admin`, generating dedicated deterministic manifests (`storefront_mobile_manifest.g.dart` and `mobile_manifest.g.dart`).

## Sources of truth and ownership

- **Customer Mobile Surface**: Owned by `apps/rustok_frontend_mobile` and mounted customer packages (e.g. `rustok_catalog_mobile`).
- **Business Operator Mobile Surface**: Owned by `apps/rustok_admin_mobile` and mounted operator packages.
- **System / SuperAdmin Control**: Owned by web admin consoles (`apps/next-admin`, `apps/admin`) and headless/agentic interfaces (`rustok-ai`).
- **Transport Contracts**: Both mobile hosts consume the canonical platform GraphQL endpoint (`/api/graphql` and `/api/graphql/ws`).

## Invariants

- Customer storefront surfaces and business administration surfaces remain decoupled in separate host applications and packages.
- Mobile admin applications shall not expose platform devops or server engine lifecycle controls.
- Both mobile hosts share foundation packages (`app_core`, `app_ui_kit`, `app_graphql`, `app_route_contracts`, `app_module_contracts`) without duplicating transport or design token logic.

### Allowed states

- Independent build, testing, and deployment of `rustok_frontend_mobile` and `rustok_admin_mobile`.
- Business users accessing CRM, orders, and catalog via `rustok_admin_mobile`.
- Buyers accessing search, catalog, and checkout via `rustok_frontend_mobile`.

### Forbidden states

- Importing business admin or operator packages directly into `rustok_frontend_mobile` storefront routes.
- Adding technical engine management (e.g. `toggleModule` or system recovery hooks) to production mobile admin workflows.

## Non-goals

- Merging mobile admin and storefront into a single mobile binary.
- Replacing desktop web admin consoles for deep analytical, financial, or configuration tasks.

## Data, transaction, and concurrency boundary

Not applicable. Both mobile hosts interact with backend domain modules through authenticated GraphQL queries and mutations.

## Context dimensions

- **Tenant / Locale / Auth**: Centralized at the host layer in both applications via `app_graphql` and `app_shell`.
- **RBAC**: Enforced server-side; `rustok_admin_mobile` respects operator roles (e.g. sales, courier, manager).

## Events and projections

Not applicable.

## Failure semantics

- Both mobile hosts handle network and GraphQL errors gracefully with retry policies and typed error boundaries.

## Migration and cutover

- Both `apps/rustok_admin_mobile` and `apps/rustok_frontend_mobile` are maintained in `rustok_mobile/pubspec.yaml`.
- Tooling validates both storefront and admin manifest outputs against their respective snapshots.

## Alternatives considered

- **Monolithic Single Mobile Host**: Rejected because mixing customer and operator workflows creates bloated applications and UX confusion.
- **Porting Full SuperAdmin DevOps to Mobile**: Rejected because server configuration on a mobile phone has near-zero utility and creates extreme maintenance burden.

## Verification

- `python -m unittest discover -s rustok_mobile/tooling/tests` passes 100%.
- `python rustok_mobile/tooling/scripts/check_mobile_codegen.py --surface storefront` and `--surface admin` pass with zero diffs.
- `python rustok_mobile/tooling/scripts/verify_storefront_graphql_contract.py` passes.

## Consequences

- Clear, consistent development path: storefront team develops `rustok_frontend_mobile`, merchant/operator team develops `rustok_admin_mobile`.
- Zero risk of mixing customer UI with business admin UI.
- Developers are freed from porting low-level server configuration screens to Dart.
