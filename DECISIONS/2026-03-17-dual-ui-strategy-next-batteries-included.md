# UI Strategy: Leptos (Primary) + Next.js (Modular Packages) + Deployment Modes

- Date: 2026-03-17
- Amended: 2026-03-18
- Status: Accepted

## Context

RusToK supports two UI stacks:

- **Leptos** — primary, compiles to WASM, auto-deploy on module install/uninstall
- **Next.js** — secondary, for JS developers familiar with the React ecosystem

History of changes to the UI package structure approach:
1. *(before 2026-03-17)* Next.js UI was stored as npm packages inside crates (`crates/rustok-blog/ui/admin/`)
2. *(2026-03-17)* Next.js moved to "batteries included" — all UI directly in `apps/next-admin/src/features/`; Leptos UI via feature flags in `src/admin/` of a single crate
3. *(2026-03-18)* **Current solution**: both stacks get separate publishable packages, but structured differently. Details below.

## Decision

### 1. Leptos UI — lives inside the module crate via feature flags

Leptos UI is located inside the module crate's directory in subdirectories `admin/` and `storefront/` (which themselves are publishable crates). Activation of the UI in the `apps/admin` application happens by including the corresponding crates.

In the main module code (backend), feature flags may exist for logical connection with the UI, but physically they are separate crates.

```text
crates/rustok-blog/
  Cargo.toml           # rustok-blog (backend)
  src/
  admin/               # rustok-blog-admin → crates.io
    Cargo.toml
    src/
  storefront/         # rustok-blog-storefront → crates.io
    Cargo.toml
    src/
```

`apps/admin/Cargo.toml` depends on `rustok-blog-admin`, `rustok-commerce-admin`, etc.
The architecture allows publishing UI independently, but in the module code they are logically connected via feature flags.

### 2. Deployment modes

Each binary is built with the required set of crates:

```bash
# pure API
cargo build -p rustok-server --release

# API + Leptos Admin WASM
cargo build -p rustok-admin --release
# (rustok-admin depends on rustok-blog-admin, rustok-commerce-admin, ...)

# API + Leptos Storefront SSR
cargo build -p rustok-storefront --release

# everything together (monolith)
cargo build --workspace --release
```

The specific deployment topology is the operator's decision: monolith, headless,
separate servers for API/admin/storefront, multi-tenant, edge.

### 3. Next.js UI — modular packages inside the application

Each module's Next.js UI lives as a **separate npm package** inside the `packages/` folder of the application itself:

```text
apps/next-admin/
  packages/
    blog/              # @rustok/blog-admin
      package.json
      src/
    commerce/          # @rustok/commerce-admin
      package.json
      src/
  src/                 # the application itself — imports from packages/*
  package.json         # depends on all packages/*

apps/next-frontend/
  packages/
    blog/              # @rustok/blog-frontend
      package.json
      src/
    commerce/          # @rustok/commerce-frontend
      package.json
      src/
  src/
  package.json
```

`apps/next-admin/package.json` depends on all `packages/*` by default.

To remove a module from Next.js:

1. Delete `apps/next-admin/packages/<module>/`
2. Remove the dependency from `apps/next-admin/package.json`
3. `npm install && npm run build`

> [!IMPORTANT]
> Auto-install via marketplace **is not supported** for Next.js.
> Rebuild is done manually. BuildExecutor only manages the Leptos stack.

### 4. rustok-module.toml — declaring UI crates/packages

```toml
# crates/rustok-blog/rustok-module.toml
[provides.admin_ui]
leptos_crate = "rustok-blog-admin"   # Cargo crate name
next_package = "@rustok/blog-admin"  # npm package name

[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
next_package = "@rustok/blog-frontend"
```

## Consequences

### Positive

- **Publishable**: both stacks can be published to registries (crates.io / npm)
- **Headless-friendly**: UI deploys independently from backend
- **Granular**: operator selects only the needed modules
- **Co-located**: UI package next to the module/application — convenient for development
- **Auto-deploy** for Leptos via BuildExecutor

### Negative

- **More crates/packages** — 3 entities per Leptos module (backend + admin + storefront)
- **Next.js — manual management** — no auto-deploy
- **API client duplication** between stacks
  Mitigation: OpenAPI-generated types from `packages/rustok-api-client`

## ADR change history

### 2026-03-18 (current revision)

- Leptos UI: moved from `src/admin/` (feature flags) to separate sub-crates `admin/` and `storefront/`
- Next.js UI: moved from `apps/next-admin/src/features/` to `apps/next-admin/packages/<module>/`
- Both stacks are now publishable (crates.io and npm respectively)
- Added `rustok-module.toml` field for declaring UI crates/packages

### 2026-03-17 (first revision)

- Next.js moved to batteries included (`apps/next-admin/src/features/`)
- Leptos: feature flags inside a single crate (`src/admin/`)
- Auto-install only for Leptos

### before 2026-03-17

- Next.js UI as npm packages inside `crates/rustok-<m>/ui/admin/`
- Leptos UI in separate crates `crates/leptos-blog-admin/`
