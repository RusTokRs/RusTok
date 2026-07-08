---
id: doc://docs/UI/module-package-implementation.md
kind: project_overview
language: markdown
status: active
---

# Module UI Package Implementation Guide

Read this document when **writing or modifying code** in a Leptos UI package.

For the architectural rationale behind the structure, see
[Module UI Package Architecture](./module-package-architecture.md).
For verification commands, see [Module UI Package Verification](./module-package-verification.md).

---

## Required File Structure

Every module-owned Leptos UI package (`admin/` or `storefront/` sub-crate) should follow
this target layout when it has enough DTO and transport surface to justify separate files:

```
admin/                            (or storefront/)
├── Cargo.toml                    ← must declare ALL dependencies explicitly
├── README.md                     ← must link to this guide
├── locales/
│   ├── en.json                   ← nested JSON, identical keys to apps/next-admin/messages/
│   └── ru.json
└── src/
    ├── lib.rs                    ← wiring only: mod declarations + root re-export
    ├── core.rs                   ← NO leptos::* imports — CI enforces this
    ├── model.rs                  ← transport-neutral DTOs shared by adapters when needed
    ├── i18n.rs                   ← written manually using rustok-ui-i18n helpers
    ├── transport/
    │   ├── mod.rs                ← public facade; only entry point for ui/leptos.rs
    │   └── graphql_adapter.rs    ← mandatory for dual-path/headless parity; uses rustok-graphql
    │  (└── native_server_adapter.rs)  ← #[server] path for SSR/hydrate profiles
    └── ui/
        ├── mod.rs
        └── leptos.rs             ← #[component], view!, signals — nothing else
```

### Cargo.toml dependencies

**IMPORTANT:** Each module UI package must **explicitly declare** all its dependencies, even if the host already depends on them.

```toml
[dependencies]
leptos.workspace = true
rustok-api = { workspace = true, default-features = false }
rustok-ui-core.workspace = true
rustok-ui-i18n-leptos.workspace = true
leptos-ui.workspace = true
leptos-ui-routing.workspace = true
serde.workspace = true
```

Declare `rustok-graphql.workspace = true` only when the package has a GraphQL adapter.
Documented native-only operator/bootstrap exceptions do not need `rustok-graphql`.

**Why?** In Rust workspace:
- `workspace = true` means "use version from root Cargo.toml"
- But **each crate must declare what it needs**
- Transitive dependencies from host don't automatically become available

**Practical reasons for explicit dependencies:**

1. **Compilation checks** — `cargo check -p rustok-blog-admin` verifies the module compiles
2. **Unit/integration tests** — `cargo test -p rustok-blog-admin` runs module-local tests
3. **IDE intelligence** — rust-analyzer needs explicit deps for autocomplete/diagnostics
4. **Dependency analysis** — `cargo tree -p rustok-blog-admin` shows what the module actually needs
5. **Future host reuse** — Another host (Dioxus, mobile) can clearly see module requirements

**Reality check:** You **cannot run** a module UI package as a standalone app (it has no `main.rs`, no routing, no shell). But you **can** compile and test it independently for development/CI verification.

**Common mistake:** Assuming that if `apps/admin` depends on `leptos-ui`, then module packages automatically get it. They don't — you must declare it explicitly in module's `Cargo.toml`.

Use `core/` subdirectory instead of `core.rs` when the core layer grows to multiple
subdomains (`view_model`, `policy`, `error`, `ports`, `identifiers`).

Use `ui/leptos/` subdirectory instead of `ui/leptos.rs` when page components multiply.

Use `transport.rs` instead of `transport/mod.rs` only for small facades. Once the package
has multiple adapters or subdomains, split it into `transport/`.

Use `model.rs` when request/response DTOs are shared by adapters or reused outside one
facade. Small native/bootstrap packages may keep transport-neutral DTOs in `core.rs` or
`transport.rs`; do not create a mechanical `model.rs` that only moves one local type.

If the package currently has only one transport adapter, this is a valid current-state
exception only when it is documented. Capture it in the module's
`docs/implementation-plan.md` as one of:

- "single-adapter state, GraphQL, native parity plan pending";
- "single-adapter state, native-only internal operator/bootstrap surface, no GraphQL/REST
  contract yet, parity or exemption plan pending".

Do not leave a single-adapter state undocumented, and do not copy native-only exceptions
into new public/headless surfaces.

---

## `lib.rs` — Wiring Only

```rust
mod core;
mod i18n;
// Include when shared request/response DTOs exist.
// mod model;
mod transport;
mod ui;

pub use ui::BlogAdmin;  // re-export root component only
```

No business logic, no transport calls, no components. Just module declarations and the
single public re-export. Modules are private — only the root component is re-exported.

---

## `core.rs` — Framework-Agnostic Logic

**Zero `leptos::*` imports.** The CI architecture guard will reject violations.

Put here:
- request/command construction, normalization, validation
- view-model mapping (display labels, fallback values, status classes, CSS policy)
- transport-agnostic error envelopes and policy results
- state transitions (busy, selected, empty, error)
- pagination/filter/sort state
- route/query intent helpers — use `rustok_ui_core::normalize_ui_text`, `parse_ui_csv`,
  `UiRouteQueryUpdate`, and `UiRouteQueryIntent`
- busy-key helpers — use `rustok_ui_core::ui_busy_key*` helpers for reusable action/id
  key encoding and matching; keep only module-specific enum names locally

Do **not** put here:
- i18n label bindings that don't affect policy (`crate::i18n::t(locale, key, fallback)` stays in `ui/leptos.rs`)
- DOM layout, event binding, Leptos signals/resources/effects
- reset/refresh side effects that depend on adapter state
- mechanical wrappers with no reuse value that only add DTO boilerplate

The rule: move something to `core` only if at least one of these is true:
- it affects transport payload or domain semantics
- it produces computable fields or display policy reused across components
- it must be testable without a UI runtime
- the same pattern already exists in two or more packages

---

## `transport/mod.rs` — Public Facade

The only transport facade that `ui/leptos.rs` imports from the transport layer. It delegates
to the active adapter and returns transport-neutral domain types, usually from `model.rs`
when those types are shared.

**GraphQL-only package (current single-adapter state for headless-compatible work):**

```rust
// transport/mod.rs — single GraphQL adapter
mod graphql_adapter;
pub use graphql_adapter::ApiError;

pub async fn fetch_posts(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<PostList, ApiError> {
    graphql_adapter::fetch_posts(token, tenant_slug, locale).await
}
```

**Dual-path package (native `#[server]` + GraphQL selected path):**

```rust
// transport/mod.rs — dual-path with profile selection
mod graphql_adapter;
mod native_server_adapter;

pub async fn fetch_posts(req: PostListRequest) -> Result<PostListResponse, TransportError> {
    #[cfg(feature = "ssr")]
    return native_server_adapter::fetch_posts(req).await;
    #[cfg(not(feature = "ssr"))]
    graphql_adapter::fetch_posts(req).await
}
```

If the package currently has only one adapter, this is a valid current-state exception only
when the missing counterpart and parity/exemption plan are documented in the module's
`docs/implementation-plan.md`.

Never expose raw `GraphqlHttpError`, `#[server]` types, or adapter internals through this
facade — only transport-neutral domain types and a package-owned error type.

---

## `transport/native_server_adapter.rs` — `#[server]` Path

- Gate everything with `#[cfg(feature = "ssr")]` or the `#[server]` macro
- Call the service layer directly: `BlogService::list_posts(...).await`
- Return transport-neutral package types only, usually from `model.rs` when shared
- Not the target baseline as the only transport. A native-only adapter is allowed only for a
  documented internal operator/bootstrap exception with no GraphQL/REST contract yet.

```rust
#[cfg(feature = "ssr")]
pub async fn fetch_posts(req: PostListRequest) -> Result<PostListResponse, TransportError> {
    use rustok_blog::BlogService;
    // ...
}
```

---

## `transport/graphql_adapter.rs` — GraphQL Adapter

- Mandatory for packages that participate in dual-path/headless parity. Documented native-only
  operator/bootstrap exceptions do not need this file until a GraphQL/REST contract exists.
- Never remove an existing GraphQL adapter just because a native path was added.
- Active in CSR/Trunk debug profile and used by Next.js/mobile/headless hosts.
- `transport/graphql_adapter.rs` uses `rustok-graphql`, the framework-agnostic GraphQL HTTP client. Leptos hooks, if needed, belong in `rustok-graphql-leptos`; transport adapters should not depend on Leptos hooks.
- Never write raw HTTP calls — always use the platform-provided GraphQL client.
- May call `core::*` helpers (e.g. `core::optional_text`) for input normalisation before
  building the GraphQL variables. This is allowed because `core` is framework-agnostic and
  has no Leptos dependency.

```rust
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use crate::core;
use crate::model::PostList;

pub async fn fetch_posts(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<PostList, ApiError> {
    // calling core helpers for normalisation is fine
    let locale = locale.map(|l| core::optional_text(&l)).flatten();
    let response: PostsResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(POSTS_QUERY, Some(PostsVariables { locale })),
        token,
        tenant_slug,
        None,
    ).await?;
    Ok(response.posts)
}
```

---

## `ui/leptos.rs` — Thin Render Adapter

Only allowed content:
- `#[component]` definitions
- `view!` macros
- Leptos signals, resources, derived signals, effects
- i18n label lookups via `crate::i18n::t(locale, "key", "fallback")`
- Calls to `transport::*` facade functions

Never allowed in this file:
- Request construction or validation logic (belongs in `core`)
- CSS class policy or display label logic (belongs in `core`)
- Direct imports from `transport::graphql_adapter` or `transport::native_server_adapter`
- Business rules or domain decisions

```rust
use crate::{core, i18n::t, transport};

#[component]
pub fn BlogAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();

    let title = t(ui_locale.as_deref(), "blog.title", "Blog");
    let posts = LocalResource::new(move || {
        transport::fetch_posts(token(), tenant(), ui_locale.clone())
    });

    view! {
        <h1>{title}</h1>
        // ...
    }
}
```

---

## Internal Libraries — Use These, Never Reinvent

### Leptos UI crates in `crates/leptos-*`

| Crate | What it provides | When to use |
|---|---|---|
| `leptos-ui` | `Button`, `Input`, `Badge`, `Alert`, `Card`, `CardHeader`, `CardContent`, `CardFooter`, `Label`, `Separator`, `Spinner`, `Checkbox`, `Switch`, `Textarea`, `Select`, `LanguageToggle` | Always — check before writing any primitive component |
| `leptos-ui-routing` | Leptos query readers/writers and route query policy integration on top of `rustok-ui-core` | All Leptos route/query state binding, including `RouteQueryWriter::apply_query_intent`; never invent a local helper |
| `leptos-auth` | Auth hooks and session context | Auth-gated operations |
| `leptos-forms` | Form state management | Multi-field forms |
| `leptos-hook-form` | Hook-form validation pattern | Complex validation flows |
| `leptos-table` | Table component with pagination | List/data table views |
| `leptos-shadcn-pagination` | Pagination UI | List pagination |
| `leptos-zod` | Schema validation (Zod-style) | Client-side schema validation |
| `leptos-zustand` | Cross-component state (Zustand-style) | Shared state across subtrees |

### Platform crates

| Crate | What it provides |
|---|---|
| `rustok-api` | Host/API contracts, permissions, locale primitives, ports and server/runtime context. It does not own UI route/query helpers or UI i18n helpers. |
| `rustok-ui-core` | **Framework-agnostic UI contracts:** `UiRouteContext`, `UiRouteQueryUpdate`, `UiRouteQueryIntent`, `AdminQueryKey`, admin query sanitization, `normalize_ui_text`, `parse_ui_csv`, `ui_busy_key*` helpers (use in `core.rs` and host UI context wiring). |
| `rustok-graphql` | **GraphQL core client:** `GraphqlRequest`, `GraphqlHttpError`, `execute`, `persisted_query_extension` (use in `graphql_adapter.rs`) |
| `rustok-graphql-leptos` | **Leptos GraphQL hooks:** `use_query`, `use_mutation`, `use_lazy_query` for Leptos UI code that needs reactive GraphQL hooks |
| `rustok-ui-i18n` | **i18n core:** `UiMessageCatalog`, `UiTranslator`, catalog parsing and fallback resolution. Do not import it through `rustok-api`. |
| `rustok-ui-i18n-leptos` | **Leptos i18n adapter:** `LeptosUiMessages` for module-owned Leptos `i18n.rs` files. |
| `rustok-ui-transport` | **Framework-agnostic FFA transport evidence:** shared transport path, selected-path error/result types and build-profile transport selection helpers for native server + GraphQL facades. |
| `rustok-seo-admin-support` | `SeoEntityPanel`, `SeoEntityForm`, `SeoSnippetPreviewCard`, `SeoRecommendationsCard` — embed in owner module admin packages |

### Shared UI primitives (`UI/leptos/`)

Source primitives live in `UI/leptos/src/`. The compiled crate boundary is `crates/leptos-ui`.
Check [`docs/UI/rust-ui-component-catalog.md`](./rust-ui-component-catalog.md) before
writing any new component — it may already exist.

Cross-framework component API (props, variants, CSS variables):
[`UI/docs/api-contracts.md`](../../UI/docs/api-contracts.md)

---

## When to Extract Shared Libraries

**Rule:** If a pattern appears in **2+ modules** or **2+ hosts**, extract it into a shared library instead of duplicating.

### Decision matrix for extraction

| Reuse pattern | Where to extract | Example |
|---|---|---|
| UI primitives (buttons, inputs, cards) | `crates/leptos-ui/` | `Button`, `Input`, `Card` |
| Framework-agnostic UI route/query/input/busy contracts | `crates/rustok-ui-core/` | `UiRouteContext`, `UiRouteQueryUpdate`, `UiRouteQueryIntent`, `AdminQueryKey`, `normalize_ui_text`, `ui_busy_key_with_id` |
| Leptos routing/query adapter helpers | `crates/leptos-ui-routing/` | `use_route_query_value`, `use_route_query_writer` |
| Framework-agnostic FFA transport result evidence and build-profile transport selection | `crates/rustok-ui-transport/` | `UiTransportError`, `UiTransportPath`, `UiTransportResult`, `execute_selected_transport` |
| Framework-agnostic GraphQL transport client | `crates/rustok-graphql/` | GraphQL request/response/error types and HTTP execution |
| Leptos GraphQL hooks adapter | `crates/rustok-graphql-leptos/` | Reactive Leptos query/mutation hooks |
| Auth/session hooks | `crates/leptos-auth/` | Auth state, session context |
| Form state management | `crates/leptos-forms/` | Multi-field form state |
| Table/pagination UI | `crates/leptos-table/` | Reusable table component |
| Framework-agnostic UI i18n | `crates/rustok-ui-i18n/` | Message catalog and key resolution |
| Leptos UI i18n adapter | `crates/rustok-ui-i18n-leptos/` | Static bundle storage and `UiRouteContext.locale` adapter |
| Host/API/backend contracts | `crates/rustok-api/` | Locale, permissions, ports, server/runtime contracts |
| Domain-specific cross-module UI | `crates/rustok-<capability>-<surface>-support/` | `rustok-seo-admin-support` |

### Extraction checklist

Before duplicating code, check:

1. ✅ **Does this pattern exist in another module?** → Search codebase first
2. ✅ **Will this be needed by 2+ modules?** → Extract to shared library
3. ✅ **Is it framework-specific (Leptos)?** → Extract to `crates/leptos-*/`
4. ✅ **Is it framework-agnostic (FFA-ready)?** → Extract to `crates/rustok-*/`
5. ✅ **Is it domain-specific but cross-module?** → Extract to `crates/rustok-<capability>-<surface>-support/`

### Anti-patterns (do NOT extract)

❌ **Single-use helpers** — Keep in module if only one consumer exists
❌ **Over-abstraction** — Don't extract just for the sake of extraction
❌ **Wrong boundary** - UI route/query or framework-specific code in `rustok-api` breaks FFA
❌ **Premature extraction** — Wait until pattern is proven in 2+ places

### FFA considerations for new libraries

When creating a new shared library, decide upfront:

**Framework-specific (Leptos-only):**
- Name: `crates/leptos-<name>/`
- Can use `leptos::*` imports
- Example: `leptos-ui`, `leptos-auth`
- **Future:** Will need Dioxus equivalent or migration to framework-agnostic

**Framework-agnostic (FFA-compatible, permanent):**
- Name: `crates/rustok-<name>/` or `crates/<name>/` (if truly generic)
- **NO** `leptos::*` or `dioxus::*` imports
- Example: `rustok-api`, `rustok-ui-i18n`, `rustok-graphql`
- Works with both Leptos and Dioxus UI adapters

### Current extraction opportunities (examples)

If you see these patterns duplicated across modules, extract them:

- **Locale negotiation helpers** -> Already in `rustok-api::locale`
- **Route query parsing** -> Already in `rustok-ui-core` (`UiRouteQueryUpdate`)
- **i18n message resolution** -> Already in `rustok-ui-i18n` (`LeptosUiMessages`)
- **GraphQL error mapping** -> Already in `rustok-graphql`
- **Native/GraphQL transport evidence and build-profile transport selection** -> Already in `rustok-ui-transport`
- **Form validation patterns** -> Extract to `leptos-forms` or framework-agnostic `rustok-forms`
- **Table pagination logic** -> Already in `leptos-table`
- **Selection state URL sync** -> Already in `leptos-ui-routing`

---

## i18n Rules

Full contract: [`docs/architecture/i18n.md`](../architecture/i18n.md)

**The single rule: the package never selects the locale.** The host provides it via
`UiRouteContext.locale`.

### How i18n works in practice

**Why not `leptos_i18n`?** Module-owned UI follows FFA (Fluid Frontend Architecture), so message catalogs and translation lookup must be framework-agnostic and reusable by sibling framework adapters.

`leptos_i18n` is a Leptos-specific library with `t!(i18n, key)` macro that:
- Depends on Leptos reactive system
- Cannot be used in framework-agnostic `core/`
- Cannot be reused by a Dioxus UI adapter

**Current contract:**
- Module-owned Leptos UI packages use `rustok-ui-i18n-leptos`.
- Module-owned UI packages never use `leptos_i18n`, `t!(i18n, key)` macros, or `rustok-api` UI i18n helpers.
- Host shell/navigation i18n is host-owned and must not be copied into module-owned UI packages.

**Solution:** `rustok-ui-i18n` provides framework-agnostic catalog and fallback resolution, while `rustok-ui-i18n-leptos` provides the shared Leptos adapter.

This is **not a full-featured i18n library** (no pluralization, ICU MessageFormat, etc.), but:
- Framework-agnostic core works with the Leptos adapter now and with the Dioxus adapter when Dioxus enters the workspace.
- Adapter crates prevent repeating static catalog boilerplate in every UI package.
- The core crate has no Leptos, Dioxus, Next.js, GraphQL or host locale-selection dependency.

`rustok-api` does not own or re-export UI message resolution.

`i18n.rs` uses the shared Leptos adapter pattern:

```rust
// src/i18n.rs
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

Usage in `ui/leptos.rs` — pass the host-provided locale and always supply a fallback string:

```rust
use crate::i18n::t;

// ui_locale comes from UiRouteContext, provided by the host
let title = t(ui_locale.as_deref(), "blog.posts.title", "Posts");

// or inline inside view!
view! {
    <h1>{t(ui_locale.as_deref(), "blog.header", "Blog")}</h1>
}
```

```
// CORRECT — locale from host context
let ui_locale = route_context.locale.clone();
let label = t(ui_locale.as_deref(), "some.key", "Fallback text");

// WRONG — package-local locale negotiation
let locale = use_cookie("lang").unwrap_or("en");
```

- Locale files: `locales/en.json` and `locales/ru.json` — **nested JSON** format.
- The key structure must be **identical** to the matching host namespace
  (`apps/next-admin/messages/` for admin surfaces, `apps/next-frontend/messages/`
  for storefront surfaces) so stacks share the same keys.
- Locale files must be declared in `rustok-module.toml`:

```toml
[provides.admin_ui.i18n]
locales_path = "admin/locales"
default_locale = "en"
```

---

## URL-Owned Selection State

Selection state lives in the URL, not in local component memory.

- Use only typed `snake_case` query keys: `product_id`, `cart_id`, `order_id`, `tab`, `slug`
- Do not read legacy `id` or camelCase aliases
- Do not make auto-select-first the source of truth
- Clean up stale detail/form state when a `?key=` becomes invalid or missing
- Access route query through `leptos-ui-routing` — never a package-local helper:

```rust
// CORRECT
let product_id = UiRouteContext::use_context().query_value::<Uuid>("product_id");

// WRONG — local wrapper
fn get_id_from_query() -> Option<Uuid> { ... }
```

---

## Manifest Wiring

Declare UI surfaces in `rustok-module.toml`, not in host app config:

```toml
[provides.admin_ui]
leptos_crate  = "rustok-blog-admin"
route_segment = "blog"
nav_label     = "Blog"

[provides.admin_ui.i18n]
locales_path    = "admin/locales"
default_locale  = "en"
```

If the sub-crate is declared, `admin/Cargo.toml` must exist and its version must match the
parent module crate. Verify with `cargo xtask module validate <slug>`.

---

## Forbidden Patterns

| Pattern | Why forbidden |
|---|---|
| `use leptos::*` in `core.rs` | Blocks Dioxus migration; CI rejects |
| `transport::graphql_adapter::fetch_x()` called from `ui/leptos.rs` | Bypasses facade; transport leaks into UI |
| `#[server]` as the only transport path without a documented native-only exception | Breaks CSR/headless parity |
| Removing `graphql_adapter.rs` after adding native path without documenting an approved native-only exception | Breaks Next.js/mobile/debug |
| `use_cookie("lang")` or local `Accept-Language` parsing | Violates platform i18n contract |
| `auto_select_first` as selection source of truth | Causes stale state bugs |
| Raw HTTP client instead of platform GraphQL client | Unmanaged transport; bypasses platform patterns (use `rustok-graphql`) |
| Writing `leptos-table`/`leptos-ui` primitives locally | Duplicates internal libraries |
| Domain business UI placed inside `apps/admin/src/` | Host becomes domain owner |
| Duplicating code across 2+ modules | Violates DRY; extract to shared library instead (see extraction decision matrix above) |
| New locale files without `rustok-module.toml` declaration | Breaks i18n verification |
| `t!(i18n, key)` macro usage | Wrong i18n pattern — `leptos_i18n` is Leptos-specific, breaks FFA; use `i18n::t(locale, "key", "fallback")` with `rustok-ui-i18n` instead |
| Writing `i18n.rs` from scratch without `rustok-ui-i18n` | Wrong i18n pattern — follow the standard `LeptosUiMessages` boilerplate; this is framework-agnostic and Dioxus-compatible |
