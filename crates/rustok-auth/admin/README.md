# rustok-auth-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI adapter package for the `rustok-auth` module.

## Responsibilities

- Exposes authentication and user-management admin pages used by `apps/admin`.
- Keeps auth-specific UI, view models, transport DTOs, and translation lookup inside the module package.
- Participates in manifest-driven admin UI composition through `rustok-module.toml`.
- Keeps the admin surface in FFA shape: Leptos-free `core.rs`, module-owned `transport/` facade, and explicit `ui/leptos.rs` render adapter.
- Uses the host-provided effective locale for translation lookup; it does not introduce package-local locale negotiation.

## Entry Points

- `AuthAdmin` - module root admin page component.
- `Login`, `Register`, `ResetPassword`, `Profile`, `Security`, `Users`, `UserDetails`, and `OAuthAppsPage` - re-exported page components.
- `src/core.rs` - framework-agnostic auth/user/OAuth request preparation, validation, error classification, multiline/default policy, and display fallbacks.
- `src/model.rs` - framework-neutral user and OAuth transport DTOs.
- `src/transport/` - module-owned native server-function transport facade.
- `src/ui/leptos.rs` - Leptos render/bind aggregation adapter.

## Interactions

- Consumed by `apps/admin` via manifest-driven routing and module registration.
- Mounted by the Leptos admin host under the auth module route segment.
- Calls server auth, user, and OAuth app endpoints through module-owned transport adapters while keeping host routing outside the package.
- Native user and OAuth server functions use `HostRuntimeContext` for DB access and the host-provided `ModuleRuntimeExtensions` handle for owner mutation runtimes; the package has no Loco runtime dependency.
- The fast boundary contract is `npm run verify:auth:admin-boundary`.

## Documentation

- See [platform docs](../../../docs/index.md).
