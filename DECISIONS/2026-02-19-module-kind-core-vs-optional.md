# Module split into Core and Optional

- Date: 2026-02-19
- Status: Accepted & Implemented

## Context

In the current architecture, the `RusToKModule` trait does not distinguish between infrastructure modules (which must always be active) and domain/optional modules (which a tenant can enable/disable via `ModuleLifecycleService`).

This leads to several problems:

1. `rustok-tenant`, `rustok-rbac`, `rustok-index` implement `RusToKModule` but are not registered in `build_registry()` — their health status is invisible, and on_enable/on_disable hooks are not called.
2. `ModuleLifecycleService::toggle_module()` theoretically allows disabling `content`, on which `blog` and `forum` depend, if `dependencies()` are not filled.
3. There is no machine-readable way to distinguish what is core from what is an optional extension.

## Decision

Introduce a `ModuleKind` field in the `RusToKModule` trait:

```rust
pub enum ModuleKind {
    Core,     // always active, toggle forbidden
    Optional, // managed per-tenant via ModuleLifecycleService
}

pub trait RusToKModule {
    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional  // safe default
    }
}
```

Modules with `ModuleKind::Core` are registered in `ModuleRegistry` in a separate `core_modules` bucket. `ModuleLifecycleService::toggle_module()` returns `ToggleModuleError::CoreModuleCannotBeDisabled` when attempting to disable them.

The following modules are marked as Core:
- `IndexModule` (`rustok-index`) — CQRS read-path, critical for storefront
- `TenantModule` (`rustok-tenant`) — tenant lifecycle hooks and health
- `RbacModule` (`rustok-rbac`) — RBAC lifecycle hooks and health

The following components **do not receive `ModuleKind`** — they are not `RusToKModule`:
- `rustok-outbox` — infrastructure component, initialized via `build_event_runtime()`, not through the registry; is Compile-time Infrastructure
- `rustok-test-utils` — exclusively `[dev-dependencies]`, does not enter the production binary
- `utoipa-swagger-ui-vendored` — vendored Swagger UI static assets, not a platform module

The following modules remain Optional:
- `ContentModule`, `CommerceModule`, `BlogModule`, `ForumModule`, `PagesModule`

`BlogModule` and `ForumModule` additionally fill in `fn dependencies() -> &["content"]`.

## Consequences

**Positive:**
- Explicit boundary between infrastructure and domain.
- Health endpoint `/health/modules` will now display Tenant, RBAC, Index.
- `toggle_module()` becomes safe: it is impossible to accidentally disable the core.
- Documentation and tooling can automatically build dependency graphs.

**Negative:**
- Small Breaking Change in the `RusToKModule` trait — all implementations must get `fn kind()` (with Optional as the default, this is non-breaking for existing modules).
- Requires updating `ModuleRegistry` and `ModuleLifecycleService`.

**Follow-up:**
- Update the `modules.toml` schema, adding `required = true` for Core modules.
- Update documentation in `docs/modules/overview.md`.
