# Deployment Profiles and UI Stack Selection

- Date: 2026-03-07
- Status: Partially superseded by [Leptos server functions as the internal data layer](./2026-03-29-leptos-server-functions-as-internal-data-layer.md)
  (composable deployment profiles remain in effect; transport between Leptos UI and server is revised)

> Clarification after `#[server]` implementation: for Leptos UI, native server functions and GraphQL live in parallel. `#[server]` became the preferred internal path, but `/api/graphql` is not removed.

## Context

RusToK supports two UI stacks:

- **Leptos** (Rust) — admin (`apps/admin`) + storefront (`apps/storefront`)
- **Next.js** (TypeScript) — admin (`apps/next-admin`) + storefront (`apps/next-frontend`)

The first iteration of the ADR proposed 3 rigid profiles (`monolith | headless-leptos |
headless-next`), but real-world scenarios are more flexible:

> "We were on monolith, but wanted to move the storefront to Next.js for two
> sites in different regions, while keeping the backend with admin together."

This cannot be expressed through 3 presets — a **composable model** is needed.

## Decision

### 1. Composable layers instead of rigid profiles

Each layer (server, admin, storefront) is configured **independently**:

```toml
# modules.toml

[build]
target = "x86_64-unknown-linux-gnu"
profile = "release"

# ───────────────────────────────────────────────
# Server: always Axum (Rust). The question is what to embed.
# ───────────────────────────────────────────────
[build.server]
embed_admin = true          # Embed Leptos admin into the binary?
embed_storefront = false    # Embed Leptos storefront into the binary?

# ───────────────────────────────────────────────
# Admin: if embed_admin = false, a separate process is needed
# ───────────────────────────────────────────────
[build.admin]
stack = "leptos"            # "leptos" | "next"
# deploy = "embedded" inferred from embed_admin = true

# ───────────────────────────────────────────────
# Storefronts: one or several (multi-site)
# ───────────────────────────────────────────────
[[build.storefront]]
id = "default"
stack = "next"              # "leptos" | "next"
# deploy = "standalone" inferred from embed_storefront = false
```

### 2. Typical configurations

#### WordPress monolith (everything in one)

```toml
[build.server]
embed_admin = true
embed_storefront = true

[build.admin]
stack = "leptos"

[[build.storefront]]
id = "default"
stack = "leptos"
```

**Result**: 1 Rust binary. Admin at `/admin`, storefront at `/`.

#### Headless Next.js (Strapi-style)

```toml
[build.server]
embed_admin = false
embed_storefront = false

[build.admin]
stack = "next"

[[build.storefront]]
id = "default"
stack = "next"
```

**Result**: 1 Rust API + 1 Node.js admin + 1 Node.js storefront.

#### Hybrid: monolith admin + Next.js multi-site

Scenario: backend + admin together (one binary), with 2 Next.js storefronts
in different regions.

```toml
[build.server]
embed_admin = true           # Admin embedded in the server
embed_storefront = false     # Storefront — separate

[build.admin]
stack = "leptos"             # Leptos embedded in Axum

[[build.storefront]]
id = "site-eu"
stack = "next"

[[build.storefront]]
id = "site-us"
stack = "next"
```

**Result**: 1 Rust binary (API + admin) + 2 Node.js storefronts.

```
                   ┌──────────────────────────────┐
                   │  rustok-server (Rust binary)  │
                   │  ┌────────────────────────┐  │
                   │  │ Axum API (GraphQL)      │  │
                   │  ├────────────────────────┤  │
                   │  │ Leptos Admin (WASM)     │  │  ← /admin
                   │  └────────────────────────┘  │
                   └──────────────┬───────────────┘
                                  │ GraphQL
                      ┌───────────┴───────────┐
                      │                       │
               ┌──────┴──────┐         ┌──────┴──────┐
               │ Next.js     │         │ Next.js     │
               │ site-eu     │         │ site-us     │
               │ EU region   │         │ US region   │
               └─────────────┘         └─────────────┘
```

#### Full headless Leptos (for max performance)

```toml
[build.server]
embed_admin = false
embed_storefront = false

[build.admin]
stack = "leptos"

[[build.storefront]]
id = "default"
stack = "leptos"
```

**Result**: 3 Rust binaries, deployed independently.

#### Leptos admin + Leptos storefront EU + Next.js storefront US

```toml
[build.server]
embed_admin = true
embed_storefront = false

[build.admin]
stack = "leptos"

[[build.storefront]]
id = "main-site"
stack = "leptos"

[[build.storefront]]
id = "us-site"
stack = "next"
```

**Result**: 1 Rust binary (API + admin) + 1 Rust SSR + 1 Node.js.
You can even mix storefront stacks.

### 3. Implementation via Cargo features

```toml
# apps/server/Cargo.toml
[features]
default = []

# Embeds Leptos admin WASM assets into the server
embed-admin = ["dep:admin-assets"]

# Embeds Leptos storefront SSR into the server
embed-storefront = ["dep:leptos-storefront"]
```

Build pipeline reads `[build.server]` and assembles features:

```bash
# embed_admin=true, embed_storefront=true → monolith
cargo build -p rustok-server --release \
  --features "embed-admin,embed-storefront"

# embed_admin=true, embed_storefront=false → admin embedded, storefront separate
cargo build -p rustok-server --release \
  --features "embed-admin"

# embed_admin=false, embed_storefront=false → pure API
cargo build -p rustok-server --release
```

For separate storefronts:

```bash
# Leptos storefront → separate Rust SSR binary
cargo build -p rustok-storefront --release

# Next.js storefront → npm build
cd apps/next-frontend && npm run build
```

### 4. Migration between configurations

Transitioning from monolith to hybrid is a **change to `modules.toml` + rebuild**.
Data (DB, tenant_modules, users) is not affected.

```bash
# Before: monolith
# Want: backend+admin together, storefront on Next.js

# 1. Update modules.toml
[build.server]
embed_admin = true
embed_storefront = false    # ← was true

[[build.storefront]]
id = "default"
stack = "next"              # ← was leptos

# 2. Rebuild
rustok rebuild
# → Builds server (with admin, without storefront)
# → Builds Next.js storefront
# → Deploys both

# 3. Data — unchanged
# GraphQL API the same, tenant_modules the same,
# storefront just fetches data from a different stack
```

### 5. DeploymentProfile in DB

The enum in the builds table remains for backward compatibility, but is extended:

```rust
pub enum DeploymentProfile {
    /// Everything in one: server + admin + storefront
    Monolith,
    /// Server + embedded admin, storefronts separate
    ServerWithAdmin,
    /// Server + embedded storefront, admin separate
    ServerWithStorefront,
    /// Pure API, everything else separate
    HeadlessApi,
}
```

Computed automatically from `[build.server]`:

| `embed_admin` | `embed_storefront` | Profile |
|---|---|---|
| `true` | `true` | `Monolith` |
| `true` | `false` | `ServerWithAdmin` |
| `false` | `true` | `ServerWithStorefront` |
| `false` | `false` | `HeadlessApi` |

### 6. Multi-site — storefront per tenant

`[[build.storefront]]` supports an array, which allows:

- **Different regions**: site-eu, site-us — same code, different instances.
- **Different stacks**: site-main on Leptos (performance), site-promo on Next.js (team knows React).
- **Different tenants**: each storefront can serve a subset of tenants.

```toml
[[build.storefront]]
id = "main"
stack = "leptos"
tenants = ["*"]              # All tenants by default

[[build.storefront]]
id = "promo-us"
stack = "next"
tenants = ["acme-us", "beta-us"]  # Only specific tenants
```

Routing (which storefront serves which tenant) — via:
- DNS (tenant.rustok.dev → specific storefront)
- Reverse proxy (nginx/traefik routing rules)
- Or via config in DB (`tenant.storefront_url`).

### 7. Marketplace — profile-agnostic

The module marketplace **does not depend on the configuration**. A module works in any
variant because:

- Backend part — **always the same** (RusToKModule trait).
- UI part — **both stacks in one crate**, build pipeline picks the right one.

If a module has no UI for a specific stack — that's OK.
Backend functionality (GraphQL, migrations, events) still works.
The UI just doesn't show in that storefront.

## Consequences

### Positive

- **Any combination** — monolith, hybrid, full headless, multi-site.
- **Example from monolith to Next.js storefront** — just change two fields in TOML.
- **Multi-site out of the box** — `[[build.storefront]]` array.
- **Stack mixing** — one storefront on Leptos, another on Next.js.
- **Data does not change** — switching stacks = only a rebuild.

### Negative

- **More complex for newcomers** — more fields than a single `deployment_profile`.
  Mitigation: presets (templates) via CLI: `rustok init --preset monolith`.
- **Build pipeline is more complex** — different artifacts need to be built for different storefronts.
- **Testing** — more combinations in CI.

### Follow-up

1. Update `modules.toml` to the new `[build.server]` / `[[build.storefront]]` format.
2. Update `DeploymentProfile` enum: `Monolith | ServerWithAdmin | ServerWithStorefront | HeadlessApi`.
3. Add Cargo features: `embed-admin`, `embed-storefront`.
4. CLI presets: `rustok init --preset monolith`, `--preset headless-next`, `--preset hybrid`.
5. Build pipeline: generate build commands based on TOML configuration.
6. Document typical configurations in README.
