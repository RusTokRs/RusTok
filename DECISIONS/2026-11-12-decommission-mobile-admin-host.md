# Decommission Flutter Mobile Admin Host in Favor of Client SuperApp and Headless/Agentic Control

- Date: 2026-11-12
- Decision status: Accepted
- Implementation status: Implemented
- Owners: platform-core, rustok_mobile
- Extends: [2026-03-07-deployment-profiles-and-ui-stack.md](./2026-03-07-deployment-profiles-and-ui-stack.md), [2026-03-17-dual-ui-strategy-next-batteries-included.md](./2026-03-17-dual-ui-strategy-next-batteries-included.md)
- Supersedes: None
- Superseded by: None

## Context

In earlier research (`docs/research/flutter.md`), an experimental Flutter admin mobile host (`rustok_mobile/apps/rustok_admin_mobile`) and a pilot admin package (`rustok_mobile/packages/rustok_modules_mobile`) were scaffolded alongside the customer-facing mobile storefront host (`rustok_frontend_mobile`).

Over time and across dozens of platform domain modules, the reality of back-office and mobile ergonomics became clear:
1. **Back-office administrative work is desktop-first**: Configuring 40+ modules, deep taxonomies, liquid/page templates, batch imports/exports, multi-column analytics, and complex RBAC matrices requires wide displays, keyboards, and mice. Attempting to replicate full administrative workflows across a 6-inch phone screen provides near-zero user return on investment.
2. **Proliferation of redundant UI hosts**: The repository already supports dual web admin consoles (Next.js in `apps/next-admin`, Leptos in `apps/admin`, and Dioxus for desktop/web back-office). Introducing a third/fourth full admin host in Flutter required replicating module UI surfaces, routes, and translations into Dart, causing massive maintenance drag and synchronization friction.
3. **The Rise of Headless and Agentic Control**: Administrative and operational workflows are increasingly handled autonomously or interactively by AI agents (`rustok-ai`, GraphQL batch mutations, MCP tools) and lightweight approval/audit logs, rendering classic repetitive CRUD mobile screens obsolete.

## Decision

1. **Decommission `rustok_admin_mobile` and `rustok_modules_mobile`**:
   - Permanently remove `rustok_mobile/apps/rustok_admin_mobile` and `rustok_mobile/packages/rustok_modules_mobile`.
   - Remove these targets from `rustok_mobile/pubspec.yaml`.
2. **Focus Flutter on Client SuperApp / Storefront**:
   - `rustok_mobile` is exclusively dedicated to the client-facing host (`rustok_mobile/apps/rustok_frontend_mobile`) and module-owned customer packages (such as `rustok_catalog_mobile`).
   - Mobile tooling (`generate_mobile_manifest.py`, `verify_mobile_manifest.py`, `check_mobile_codegen.py`) defaults to the storefront surface.
3. **Administration Remains Desktop Web & Agentic**:
   - Back-office administration remains centered on canonical web admin hosts (`apps/next-admin` / `apps/admin`).
   - Operational automation, data entry, and maintenance tasks are delegated to headless/agentic interfaces (`rustok-ai`, MCP, GraphQL).

## Sources of truth and ownership

- **Platform Administration UI**: Owned by web admin hosts (`apps/next-admin`, `apps/admin`).
- **Client Mobile App**: Owned by `rustok_mobile/apps/rustok_frontend_mobile` mounting customer-facing module packages.
- **Agentic Control**: Owned by backend modules, `rustok-ai`, MCP tools, and GraphQL mutation contracts.
- **Manifest Codegen**: Generates customer-facing storefront route registries by default.

## Invariants

- The Flutter workspace contains only one customer-facing application host: `apps/rustok_frontend_mobile`.
- No domain module is required or permitted to maintain a mobile-specific Flutter administrative screen.
- Mobile manifest generation and freshness checks validate customer-facing storefront surfaces by default.

### Allowed states

- Single mobile host (`rustok_frontend_mobile`) mounting client-facing packages (`rustok_catalog_mobile` and future customer packages).
- Web admin consoles providing full back-office operator workflows.
- AI agents executing operational tasks through GraphQL and MCP tools.

### Forbidden states

- Authoring new Dart-based administrative screens or host packages for back-office tasks.
- Diverging mobile manifests that assume `admin_mobile` surfaces exist in Flutter apps.

## Non-goals

- Removing web admin consoles (`apps/admin`, `apps/next-admin`, `apps/dioxus-admin`).
- Disallowing future lightweight operational apps (such as dedicated courier or warehouse barcode scanners) if explicitly requested as targeted micro-apps rather than a monolithic full admin console.

## Data, transaction, and concurrency boundary

Not applicable. This is a frontend host consolidation decision; data and transaction boundaries remain owned by server domain modules.

## Context dimensions

- **Tenant / Locale / Auth**: Handled centrally by `rustok_frontend_mobile` app shell and `app_graphql` for customer sessions.
- **Role / Permissions**: Customer permissions apply to storefront endpoints; administrative permissions (`*:manage`) are exercised via web consoles or authenticated agent sessions.

## Events and projections

Not applicable.

## Failure semantics

- Tooling scripts fail fast if mobile manifest generation encounters invalid or duplicate routes.
- Mobile storefront client fails gracefully with error boundaries when modules or products are unavailable.

## Migration and cutover

- `apps/rustok_admin_mobile` and `packages/rustok_modules_mobile` are removed from disk and from `rustok_mobile/pubspec.yaml`.
- Tooling scripts default to `--surface storefront`.
- Documentation is synchronized across `docs/index.md`, `rustok_mobile/README.md`, `CONTRIBUTING.md`, and research docs.

## Alternatives considered

- **Maintain a 3-way/4-way Admin parity (Leptos + Next + Dioxus + Flutter)**: Rejected due to unsustainable maintenance overhead and zero business demand for managing 40+ modules on a phone screen.
- **Keep `rustok_admin_mobile` as an unmaintained skeleton**: Rejected as architectural debt and confusing to contributors.

## Verification

- `python -m unittest discover -s rustok_mobile/tooling/tests` passes 100%.
- `python rustok_mobile/tooling/scripts/check_mobile_codegen.py --repo-root .` passes.
- `python rustok_mobile/tooling/scripts/verify_storefront_graphql_contract.py --repo-root .` passes.
- `node scripts/verify/verify-adrs.mjs` and `node scripts/verify/verify-docs.mjs` pass.

## Consequences

- Reduced maintenance footprint: teams only write admin surfaces for web consoles.
- High focus for Flutter: 100% of mobile development effort goes into creating a high-performance customer SuperApp.
- Clear alignment with the platform's agentic roadmap (`rustok-ai`).
