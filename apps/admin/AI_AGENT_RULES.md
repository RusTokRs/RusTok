# AI Agent Rules for `apps/admin`

## Read These Guides First

Before making any changes to Leptos admin code or module-owned UI packages:

1. **[Implementation Guide](../../docs/UI/module-package-implementation.md)** - internal libraries, i18n, file structure, forbidden patterns
2. **[Architecture Guide](../../docs/UI/module-package-architecture.md)** - FFA (Fluid Frontend Architecture), `core/transport/ui` split
3. **[Verification Guide](../../docs/UI/module-package-verification.md)** - verification commands, common errors

## Critical Rules

### 1. Do Not Write Custom UI Components

**Always check first:** [Rust UI Component Catalog](../../docs/UI/rust-ui-component-catalog.md)

Before writing reusable code, check whether it already exists in shared libraries:

- `leptos-ui` - Button, Input, Badge, Alert, Card, Label, Spinner, Checkbox, Switch, Textarea, Select, LanguageToggle
- `leptos-ui-routing` - `UiRouteContext`, `module_route_base()`, `query_value()`
- `rustok-graphql` - framework-agnostic GraphQL HTTP client
- `rustok-graphql-leptos` - Leptos GraphQL hooks adapter
- `leptos-auth` - auth hooks and session
- `leptos-forms` - form state management
- `leptos-table` - table with pagination
- `leptos-zod` - schema validation
- `leptos-zustand` - serializable state DTOs only; do not treat it as a runtime store before owner approval

### 2. Do Not Invent Custom i18n

Always use `rustok-ui-i18n-leptos` for Leptos module-owned UI packages.

Never use `rustok-api` for UI i18n helpers, `leptos_i18n`, `t!(i18n, key)` macros, or custom locale negotiation.

`rustok-ui-i18n` is the framework-agnostic core. `rustok-ui-i18n-leptos` is the shared Leptos adapter. A sibling `rustok-ui-i18n-dioxus` adapter must be added when Dioxus enters the workspace.

Pattern:

```rust
use rustok_ui_i18n_leptos::LeptosUiMessages;

static MESSAGES: LeptosUiMessages = LeptosUiMessages::new(
    "en",
    &[
        ("en", include_str!("../locales/en.json")),
        ("ru", include_str!("../locales/ru.json")),
    ],
);

pub fn t(locale: Option<&str>, key: &str, fallback: &str) -> String {
    MESSAGES.t_for_locale(locale, key, fallback)
}
```

Locale comes from `UiRouteContext.locale` or another host-provided effective locale. Never read package-local cookies, headers, query parameters, or browser storage for module-owned UI locale selection.

### 3. Do Not Remove GraphQL When Adding `#[server]`

Always keep both transport contracts:

- native `#[server]` functions for SSR/hydrate monolith builds;
- GraphQL for CSR debug, headless clients, Next.js, and external consumers.

Do not make `#[server]` the only path for module-owned UI packages that expose public/headless-capable surfaces.

### 4. Do Not Write Leptos Code in `core.rs`

`core.rs` and `core/` must have zero `leptos::*` imports. CI enforces this.

Never put `#[component]`, `view!`, signals, or effects in `core`.

### 5. Do Not Put Module UI in Host

Module business UI belongs in `crates/rustok-<module>/admin/`.

Never place module CRUD screens or workflows in `apps/admin/src/`, except host shell and shared composition code under `widgets/app_shell` or `shared`.

### 6. FSD Architecture

This host follows Feature-Sliced Design layers:

- `app` - routing, shell
- `widgets` - app shell, header
- `features` - cross-module composition
- `shared` - shared contracts
- module-owned UI packages - `crates/rustok-*/admin/`

## Verification Commands

After any change:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
npm run verify:i18n:ui
npm run verify:i18n:contract
npm run verify:frontend:host-ffa-contract
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1
```

## What Is FFA?

FFA (Fluid Frontend Architecture) means module UI keeps framework-agnostic policy, transport contracts, and Leptos bindings separated.

Target runtime paths:

- monolith SSR/hydrate via native `#[server]`;
- standalone CSR/Trunk via GraphQL;
- headless Next.js/mobile via GraphQL.

Three-layer split:

```text
core/             - no Leptos imports; framework-agnostic logic
transport/        - adapters: native_server_adapter.rs and graphql_adapter.rs
ui/leptos.rs      - Leptos binding only: #[component], view!, signals
```

The `graphql_adapter` must use the framework-agnostic `rustok-graphql` client.

Target Dioxus shape:

1. Add `ui/dioxus.rs` and Dioxus-specific bindings when Dioxus enters the workspace.
2. Keep `core` and transport unchanged.
3. Keep GraphQL transport on `rustok-graphql`.
4. Treat Leptos and Dioxus as sibling framework adapters, not as old/new compatibility layers.

Full FFA concept: [Fluid Frontend Architecture](../../docs/research/fluid-frontend-architecture.md)

## Common Mistakes to Avoid

| Wrong | Right |
| --- | --- |
| `use leptos::*` in `core.rs` | Move logic to `ui/leptos.rs` or make it transport-neutral |
| `transport::graphql_adapter::fetch_x()` in UI | Call the public `transport::fetch_x()` facade |
| `#[server]` only, no GraphQL | Keep native and GraphQL in parallel where the surface is public/headless-capable |
| Removing `graphql_adapter.rs` | Keep GraphQL as the public/headless contract |
| `use_cookie("lang")` in a package | Use `UiRouteContext.locale` from the host |
| Writing `Button` locally | Use `leptos-ui::Button` |
| Raw HTTP client in `graphql_adapter` | Use `rustok-graphql` |
| `t!(i18n, key)` macro | Use `i18n::t(locale, "key", "fallback")` |
| Module UI in `apps/admin/src/features/` | Use `crates/rustok-<module>/admin/` |
| Writing `i18n.rs` without `rustok-ui-i18n-leptos` | Follow the standard `LeptosUiMessages` adapter boilerplate |
| Forgetting dependencies in module `Cargo.toml` | Each module must declare all direct dependencies explicitly |

## Full Documentation

- [apps/admin/docs/README.md](./docs/README.md) - host-level documentation
- [docs/UI/README.md](../../docs/UI/README.md) - UI documentation index
- [docs/index.md](../../docs/index.md) - platform documentation map
