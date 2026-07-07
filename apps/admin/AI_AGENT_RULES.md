# AI Agent Rules for `apps/admin`

## READ THESE GUIDES FIRST

Before making ANY changes to Leptos admin code or module-owned UI packages:

1. **[Implementation Guide](../../docs/UI/module-package-implementation.md)** — internal libraries, i18n, file structure, forbidden patterns
2. **[Architecture Guide](../../docs/UI/module-package-architecture.md)** — FFA (Fluid Frontend Architecture), `core/transport/ui` split
3. **[Verification Guide](../../docs/UI/module-package-verification.md)** — verification commands, common errors

## Critical Rules

### 1. DO NOT Write Custom UI Components
✅ **ALWAYS check first:** [Rust UI Component Catalog](../../docs/UI/rust-ui-component-catalog.md)

**Before writing ANY reusable code, check if it exists in shared libraries:**

Available internal libraries:
- `leptos-ui` — Button, Input, Badge, Alert, Card, Label, Spinner, Checkbox, Switch, Textarea, Select, LanguageToggle
- `leptos-ui-routing` — UiRouteContext, module_route_base(), query_value()
- `leptos-graphql` — GraphQL client for Leptos (temporary Leptos-specific; will become framework-agnostic during Dioxus migration)
- `leptos-auth` — Auth hooks and session
- `leptos-forms` — Form state management
- `leptos-table` — Table with pagination
- `leptos-zod` — Schema validation
- `leptos-zustand` — Cross-component state

### 2. DO NOT Invent Custom i18n
ALWAYS use `rustok-ui-i18n-leptos` for Leptos module-owned UI packages.
NEVER use `rustok-api` for UI i18n helpers, `leptos_i18n`, `t!(i18n, key)` macros, or custom locale negotiation.

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

Locale comes from `UiRouteContext.locale` or another host-provided effective locale, NEVER from package-local cookies, headers, query parameters or browser storage.
### 3. DO NOT Remove GraphQL When Adding `#[server]`
✅ **ALWAYS keep both:** native `#[server]` (SSR/hydrate) + GraphQL (CSR/headless)
❌ **NEVER make `#[server]` the only path** — CSR/Trunk debug requires GraphQL

### 4. DO NOT Write Leptos Code in `core.rs`
✅ `core.rs` / `core/` must have **ZERO `leptos::*` imports** — CI enforces this
❌ **NEVER put** `#[component]`, `view!`, signals, effects in `core`

### 5. DO NOT Put Module UI in Host
✅ Module business UI belongs in `crates/rustok-<module>/admin/`
❌ **NEVER place** module CRUD/workflows in `apps/admin/src/` (except `widgets/app_shell`, `shared`)

### 6. FSD Architecture
This host follows **Feature-Sliced Design** layers:
- `app` — routing, shell
- `widgets` — app_shell, header
- `features` — cross-module composition
- `shared` — shared contracts
- Module-owned UI packages: `crates/rustok-*/admin/`

## Verification Commands

After ANY change:
```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
npm run verify:i18n:ui
npm run verify:i18n:contract
npm run verify:frontend:host-ffa-contract
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1
```

## What is FFA?

**FFA (Fluid Frontend Architecture)** = same UI code runs in:
- Monolith SSR/hydrate (via `#[server]`)
- Standalone CSR/Trunk (via GraphQL)
- Headless Next.js/mobile (via GraphQL)

**Three-layer split:**
```
core/             — NO Leptos imports, framework-agnostic logic
transport/        — adapters (native_server_adapter.rs + graphql_adapter.rs)
                    graphql_adapter currently uses leptos-graphql (Leptos-specific)
                    → will use framework-agnostic client during Dioxus migration
ui/leptos.rs      — ONLY Leptos binding (#[component], view!, signals)
```

**Goal:** When migrating to Dioxus:
1. Only `ui/leptos.rs` → `ui/dioxus.rs` changes
2. Core and transport stay unchanged
3. GraphQL adapter switches to framework-agnostic client (transparent to UI layer)
4. Both Leptos and Dioxus adapters coexist during migration

Full FFA concept: [Fluid Frontend Architecture](../../docs/research/fluid-frontend-architecture.md)

## Common Mistakes to Avoid

| ❌ WRONG | ✅ RIGHT |
|---------|---------|
| `use leptos::*` in `core.rs` | Move logic to `ui/leptos.rs` or make transport-neutral |
| `transport::graphql_adapter::fetch_x()` in UI | Call `transport::fetch_x()` facade |
| `#[server]` only, no GraphQL | Keep both in parallel |
| Removing `graphql_adapter.rs` | Keep it forever, even after adding native path |
| `use_cookie("lang")` in package | Use `UiRouteContext.locale` from host |
| Writing `Button` component locally | Use `leptos-ui::Button` |
| Raw HTTP client in graphql_adapter | Use platform GraphQL client (`leptos-graphql` now, framework-agnostic later) |
| `t!(i18n, key)` macro | Use `i18n::t(locale, "key", "fallback")` |
| Module UI in `apps/admin/src/features/` | Use `crates/rustok-<module>/admin/` |
| Writing `i18n.rs` without `rustok_api` pattern | Follow standard `build_ui_message_catalog` boilerplate |
| Forgetting to declare deps in module `Cargo.toml` | Each module must declare ALL dependencies explicitly (for `cargo check`/`cargo test`), even if host has them. `workspace = true` coordinates version only. |

## Full Documentation

- [apps/admin/docs/README.md](./docs/README.md) — host-level documentation
- [docs/UI/README.md](../../docs/UI/README.md) — UI documentation index
- [docs/index.md](../../docs/index.md) — platform documentation map
