# rustok-customer-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-customer` module.

## Responsibilities

- Exposes the customer operations admin root view used by `apps/admin`.
- Keeps customer list/detail/create/update UX inside the customer-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses native Leptos server functions as the primary admin transport behind the module-owned `admin/src/transport/mod.rs` facade instead of inventing a new umbrella GraphQL or REST layer; the server-function path consumes `HostRuntimeContext` and no longer depends on the previous runtime crate.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.
- Keeps framework-agnostic customer list defaults, submit-command validation/preparation, submit/transport error message mapping, shell/list/detail header view models, field placeholder DTOs, detail section/profile-empty copy, timestamp/user/locale/visibility display labels, list/detail view models, page-state view models, refresh/open action-state policy, editor action-state policy, active-row CSS policy, and form snapshots in `admin/src/core.rs` so render adapters do not own pagination, display fallback, header copy, form placeholders, detail section copy, timestamp/profile labels, error formatting, action disabled state, editor mode/disabled, or form mapping policy.
- Keeps native Leptos server functions in `admin/src/transport/native_server_adapter.rs`; `admin/src/transport/mod.rs` remains the module-owned facade consumed by UI.
- Keeps Leptos render/bind code in `admin/src/ui/leptos.rs`; `admin/src/lib.rs` only wires modules and re-exports `CustomerAdmin`.

## Entry Points

- `CustomerAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Reads and writes through `rustok-customer::CustomerService`, with optional profile enrichment through `rustok-profiles::ProfileService`; both services are created from the neutral host runtime database handle.
- Reads the effective UI locale from `UiRouteContext.locale`; customer writes use that host-owned locale instead of a package-local locale override.

## Documentation

- See [platform docs](../../../docs/index.md).
- Fast boundary guardrail: `node scripts/verify/verify-customer-admin-boundary.mjs`.
