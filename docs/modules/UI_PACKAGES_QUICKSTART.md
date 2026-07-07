---
id: doc://docs/modules/UI_PACKAGES_QUICKSTART.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# UI Packages Quick Start

This quick start is for creating or finalizing a module-owned UI surface without
the old noise around installation and deployment. The canonical path starts with local module docs
and manifest wiring, not with host-specific hacks.

## What Should Be the Result

A module with UI at the end of this pass should have:

- Root `README.md` in English;
- `docs/README.md` in English;
- `docs/implementation-plan.md` in English;
- `rustok-module.toml` with correct `[provides.admin_ui]` and/or
  `[provides.storefront_ui]`;
- `admin/` and/or `storefront/` sub-crate if UI is actually provided;
- Passing `cargo xtask module validate <slug>`.

## Step 1. Set Up the Documentation Contract

Before UI wiring, the module must obtain the minimum docs standard:

- Root `README.md` with `Purpose`, `Responsibilities`, `Entry points`,
  `Interactions` and a link to `docs/README.md`;
- Local `docs/README.md` with sections `Purpose`, `Responsibility Zone`,
  `Integration`, `Verification`, `Related Documents`;
- Local `docs/implementation-plan.md` with at least `Focus` and `Improvements`.

If the module already exists, first update local documentation, then
add or change UI wiring.

## Step 2. Define UI Ownership

Before creating a UI package, capture:

- Whether this is a module-owned admin surface, storefront surface or both;
- Which host will mount the package: `apps/admin`, `apps/storefront`,
  `apps/next-admin` or `apps/next-frontend`;
- Whether only Leptos UI is needed, or also Next.js host integration;
- Whether there are package-owned locale bundles that need to be declared through the manifest.

The host application must not become the owner of this UI functionality.

## Step 3. Add Manifest Wiring

In `rustok-module.toml`, specify only actually existing UI surfaces.

Example for admin UI:

```toml
[provides.admin_ui]
leptos_crate = "rustok-blog-admin"
route_segment = "blog"
nav_label = "Blog"
```

Example for storefront UI:

```toml
[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
route_segment = "blog"
page_title = "Blog"
slot = "home_after_catalog"
```

If a UI sub-crate is declared in the manifest, `admin/Cargo.toml` or
`storefront/Cargo.toml` must actually exist and match the version of the main module.

## Step 4. Implement the UI Surface

For Leptos module-owned UI, the following baseline applies:

- Production runtime for Leptos hosts is considered SSR-first: the internal data layer is built on `#[server]` functions in `ssr`/`hydrate` profiles by default;
- GraphQL is not removed and remains the target parallel transport contract;
- CSR/WASM standalone remains a mandatory debug/compatibility profile for public/headless-capable UI packages, so those packages must have a GraphQL/REST fallback and must not require `/api/fn/*` in `csr`;
- Locale comes from the host/runtime contract, not from local cookie/header/query
  fallback chains;
- UI message resolution uses `rustok-ui-i18n-leptos` for Leptos packages and `rustok-ui-i18n` as the framework-agnostic core, not framework-specific i18n macros;
- The UI package does not pull in ownership of domain logic that should live in
  the module itself.
- For admin packages, selection state is considered URL-owned: use only typed
  `snake_case` query keys like `product_id`, `cart_id`, `order_id`; do not read legacy `id`/camelCase aliases; do not make
  auto-select-first the source of truth and clean up stale detail/form state on failed open.
- For Leptos storefront packages, query/state plumbing should also go through a shared reusable layer:
  read route query through `leptos-ui-routing`, do not invent a package-local helper over
  `UiRouteContext.query_value(...)` and do not diverge the storefront contract from host-level route semantics.

Why this split: a module-owned UI package normally lives in two modes simultaneously. In a product monolith, the host mounts it through SSR/hydrate and prefers `#[server]`, while for standalone debug/headless parity the same package must have a GraphQL/REST fallback and not depend on `/api/fn/*` in CSR. Native-only internal operator/bootstrap surfaces are allowed only as explicit module-local exceptions with no current GraphQL/REST contract.

For Next.js host integration:

- The module publishes a package-owned UI surface or host-specific integration layer;
- The domain contract itself remains with the module crate and server/API layer;
- The host only mounts, routes and composes.

## Step 5. Update Local Docs

After UI wiring appears, synchronously update:

- Module `README.md`;
- Module `docs/README.md`;
- `docs/implementation-plan.md` if the UI layer changes the roadmap;
- If needed, `admin/README.md` or `storefront/README.md`.

Central docs in `docs/modules/*` are updated only after local module docs are current.

## Step 6. Verify the Module Pointwise

Minimum local run:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

If the host/UI layer is affected, the following are additionally usually needed:

```powershell
npm run verify:i18n:ui
npm run verify:i18n:contract
npm.cmd run verify:storefront:routes
```

On Windows, the architecture guard runs via:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1
```

## What Not to Do

- Do not describe a UI package only in `apps/*`;
- Do not leave `admin/` or `storefront/` without manifest wiring;
- Do not introduce a separate i18n contract at the UI package level;
- Do not invent a package-local route-selection contract over the host schema;
- Do not describe standalone CSR/Trunk as a production default for Leptos hosts;
- Do not consider old installation and deployment instructions as canonical source of truth;
- Do not remove GraphQL in favor of `#[server]` or remove `#[server]` in favor of GraphQL where
  a parallel transport contract is needed.

## Where to Go Next

- [Module UI Packages Index](./UI_PACKAGES_INDEX.md)
- [`rustok-module.toml` Contract](./manifest.md)
- [Module Documentation Template](../templates/module_contract.md)
- [GraphQL and Leptos Server Functions](../UI/graphql-architecture.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [UI README](../UI/README.md)
- **Module UI Package Guides** (read when implementing Step 4 above):
  - [Architecture Guide](../UI/module-package-architecture.md) — FFA, `core/transport/ui` split, dual-path model, Dioxus-readiness
  - [Implementation Guide](../UI/module-package-implementation.md) — file structure, internal crates, i18n, URL-selection, manifest wiring, forbidden patterns
  - [Verification Guide](../UI/module-package-verification.md) — all verification commands, what each checks, common errors
