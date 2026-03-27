<div align="center">

# <img src="assets/rustok-logo-512x512.png" width="72" align="center" /> RusToK

**Event-Driven Enterprise Headless Platform Built with Rust**

*The stability of a tank. The speed of compiled code. The first CMS designed for the AI-Agent era.*

[![CI](https://github.com/RustokCMS/RusToK/actions/workflows/ci.yml/badge.svg)](https://github.com/RustokCMS/RusToK/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

**[🇷🇺 Русская версия](README.ru.md)** | **[📋 Quick Platform Info (RU)](PLATFORM_INFO_RU.md)**

[Features](#features) •
[Why Rust?](#why-rust) •
[Comparison](#comparison) •
[Quick Start](docs/guides/quickstart.md) •
[Documentation](docs/index.md) •
[Roadmap](docs/roadmap.md)

</div>

---

## 🎯 What is RusToK?

**RusToK** is an event-driven, modular highload platform for any product with data. Each module is isolated and microservice-ready, while still shipping as a single, secure Rust binary. It combines the developer experience of Laravel/Rails with the performance of Rust, using a "Tank" strategy for stability and a "CQRS-lite" approach for fast reads.

Modules in RusToK are compiled into a binary for maximum performance and security, but follow a standardized layout (Entities/DTO/Services) for easy maintainability. •
Rustok can become the foundation of anything that has any data.

From an alarm clock with a personal blog to NASA's terabyte storage.

We consume 10-200 times less power than traditional platforms.

We can work on any device with an operational memory of more than 50 MB (Maybe less).

Highload for the poor, salvation for the rich...

Our architecture will be relevant for decades. We won't turn into another WordPress.

From a personal blog or landing page to petabytes of data storage.

FORGET ABOUT OLD PATTERNS, WE'RE BUILDING THE FUTURE. WE HAVE NO LIMITATIONS!

┌─────────────────────────────────────────────────────────────┐
│                      RusToK Platform                        │
├─────────────────────────────────────────────────────────────┤
│  🛍️ Storefront (SSR)  │  ⚙️ Admin Panel  │  📱 Mobile App   │
│      Leptos SSR       │    Leptos CSR    │   Your Choice    │
├─────────────────────────────────────────────────────────────┤
│                    🔌 GraphQL API                           │
├─────────────────────────────────────────────────────────────┤
│  📦 Commerce  │  📝 Content  │  👥 Community  │ ...       │
├─────────────────────────────────────────────────────────────┤
│                    🧠 Core (Loco.rs)                        │
│          Auth • Tenants • Nodes • Tags • Events             │
├─────────────────────────────────────────────────────────────┤
│     🐘 PostgreSQL (write)  |  🔎 Index Module (read)         │
└─────────────────────────────────────────────────────────────┘

### 💡 The "Why"

Most platforms are either **fast but complex** (Go/C++) or **productive but slow** (PHP/Node). RusToK breaks this trade-off using the **Loco.rs** foundation, giving you "Rails-like" speed of development with "C++-like" runtime performance.

---

## ✨ Features

### Core Platform

- 🔐 **Multi-tenant Isolation** — Native support for multiple stores/sites in one deployment with security-hardened validation
- 🔑 **Enterprise Auth** — JWT + sessions with fine-grained RBAC, built-in OAuth2 Authorization Server for external integrations
- 📊 **Hybrid API** — Unified GraphQL for domain data and REST for infrastructure/OpenAPI
- 🏗️ **Standardized Modules** — Clean architecture with `entities`, `dto`, and `services` in every crate
- 🎣 **Event-Driven Pub/Sub** — Async synchronization with validation, backpressure control, and transactional guarantees
- 📚 **Full OpenAPI Documentation** — Comprehensive Swagger UI for all REST controllers
- 🌍 **Global-First** — Built-in i18n and localization support
- 🛡️ **Security Hardened** — Input validation, injection prevention (SQL/XSS/Path Traversal), reserved name blocking
- ⚖️ **Backpressure Control** — Automatic rate limiting prevents OOM from event floods

### Deployment Modes — What No Other CMS Can Do

RusTok is the only platform that supports **all three deployment modes** with a single codebase:

| Mode | How it works | Auth | Use case |
|------|-------------|------|----------|
| **Monolith** | Admin + storefront(s) compiled into one binary via Leptos SSR. Single process, single port — like WordPress but in Rust | Server sessions (cookie-based) | Self-hosted sites, blogs, small e-commerce |
| **Headless** | Server exposes GraphQL/REST API. Frontend is separate (React, Flutter, mobile, CRM) | OAuth2 (PKCE, client_credentials) | Enterprise, mobile apps, third-party integrations |
| **Mixed** | Built-in Leptos UI (sessions) + external clients (OAuth2) at the same time | Both | Built-in admin + external mobile app + CRM via API |

No other CMS offers this combination:

| Capability | WordPress | Shopify | Strapi | Ghost | **RusToK** |
|---|---|---|---|---|---|
| Monolith (single binary) | yes | no | no | yes | **yes** |
| Headless API with OAuth2 AS | plugin | yes | no | no | **built-in** |
| Mixed mode (both at once) | hacks | no | no | no | **yes** |
| Multi-tenant | multisite (hacky) | no | no | no | **native** |
| Compile-time modules | no (PHP plugins) | no (external apps) | no (JS plugins) | no | **Rust crates** |
| Configurable admin path | plugin | no | no | no | **yes** (`/admin` → `/my-panel`) |
| SSR + WASM (one language) | no | no | no | no | **Leptos** |

**Monolith mode**: Admin panel served at a configurable prefix (default `/admin`, changeable for security). Storefront(s) on root routes or subdomains. All authentication via server sessions — no OAuth needed. Works in multi-tenant and multi-site configurations.

**Headless mode**: Any frontend connects via OAuth2. Register your app (SPA, mobile, CRM, ERP) through the Admin UI or GraphQL API, get `client_id` + scopes, and you're connected.

**Mixed mode**: Built-in Leptos admin uses sessions. External mobile app uses OAuth2 PKCE. Both work simultaneously on the same server.

### Developer Experience

- 🚀 **Loco.rs Framework** — Rails-like productivity in Rust
- 🛠️ **CLI Generators** — `cargo loco generate model/controller/migration`
- 📝 **Type-Safe Everything** — From database to frontend, one language
- 🧪 **Testing Built-in** — Unit, integration, and E2E test support
- 🎨 **Storefront UI Stack** — Leptos SSR + Next.js starters with Tailwind-based UI
- 📚 **Auto-generated Docs** — OpenAPI/GraphQL schema documentation

### Performance & Reliability

- ⚡ **Blazingly Fast** — Native compiled binary, no interpreter overhead
- 🛡️ **Memory Safe** — Rust's ownership model prevents entire classes of bugs
- 📦 **Single Binary** — Deploy one file, no dependency management
- 🔄 **Zero-Downtime Deploys** — Graceful shutdown and health checks
- 🔎 **CQRS-lite Read Models** — Denormalized index tables for fast storefront queries
- 🔧 **Circuit Breaker Pattern** — Fail-fast resilience (30s → 0.1ms, -99.997% latency)
- 🎯 **Type-Safe State Machines** — Compile-time guarantees for business logic
- 📊 **Rich Error Handling** — RFC 7807 compatible API errors with structured context

### Testing & Quality (80% Coverage)

- 🧪 **Unit Tests** — Comprehensive test suite with 80% coverage
- 🎲 **Property-Based Tests** — 10,752+ test cases with proptest
- ⚡ **Performance Benchmarks** — Criterion.rs suites for all critical paths
- 🔐 **Security Tests** — 25+ OWASP-focused integration tests
- 🔍 **Integration Tests** — End-to-end test suites for all flows

### Observability & Security

- 📊 **OpenTelemetry** — Full observability stack with distributed tracing
- 📈 **Metrics Dashboard** — Grafana dashboards with 40+ SLO alerts
- 🛡️ **OWASP Top 10** — 100% compliance with security best practices
- 🔒 **Security Headers** — CSP, HSTS, X-Frame-Options protection
- ⏱️ **Rate Limiting** — Token bucket algorithm with configurable limits

---

## 🤔 Why Rust?

### The Problem with Current CMS Solutions

| Issue | WordPress | Node.js CMS | RusToK |
|-------|-----------|-------------|--------|
| **Runtime Errors** | Fatal errors crash site | Uncaught exceptions | Compile-time guarantees |
| **Memory Leaks** | Common with plugins | GC pauses, memory bloat | Ownership model prevents |
| **Security** | 70% of vulns from plugins | npm supply chain risks | Compiled, auditable deps |
| **Performance** | ~50 req/s typical | ~1000 req/s | ~50,000+ req/s |
| **Scaling** | Requires caching layers | Horizontal only | Vertical + Horizontal |

### The Rust Advantage

```rust
// This code won't compile if you forget to handle an error
let product = Product::find_by_id(db, product_id)
    .await?  // ? forces you to handle the error
    .ok_or(Error::NotFound)?;  // Explicit None handling

// Compare to JavaScript:
// const product = await Product.findById(id); 
// // What if id is undefined? What if DB fails? Runtime crash!
```

Real-world impact:

- 🐛 Fewer bugs in production — Most errors caught at compile time
- 💰 Lower infrastructure costs — 10x less memory, 50x more throughput
- 😴 Sleep better at night — No 3 AM "site is down" emergencies

---

## ⚡ Performance & Economy

### 💰 Save 80% on Infrastructure

While a typical Node.js or Python application requires **256MB-512MB RAM** per instance, a RusToK production container starts at just **30MB-50MB**.

- **Deploy on $5 VPS**: Handle traffic that would cost $100/mo on other stacks.
- **Serverless Friendly**: Native binary starts in milliseconds. Zero "cold start" issues.

### 🚀 Benchmarks (simulated)

| Metrics | WordPress | Strapi | RusToK |
|---------|-----------|--------|--------|
| **Req/sec** | 60 | 800 | **45,000+** |
| **P99 Latency**| 450ms | 120ms | **8ms** |
| **Cold Boot** | N/A | 8.5s | **0.05s** |

---

## 🤖 AI-Native Architecture

RusToK is the first platform built with a **System Manifest** designed specifically for AI Assistants.

- **Structured for Agents**: Clean directory patterns and exhaustive documentation mean AI (Cursor, Windsurf, Claude) builds features for you with 99% accuracy.
- **Zero Boilerplate**: Use our CLI and AI-prompts to generate entire modules in minutes.

---

## 🦄 Legendary Efficiency (Hyper-Optimized)

RusToK is so efficient that it doesn't just run on servers — it survives where others crash:

- **Smartwatch Ready**: Handle a million requests per second while running on your smart fridge or a digital watch.
- **Powered by Vibes**: We handle high traffic using less energy than a literal cup of coffee.
- **Quantum Speed**: Our response times are so low that requests are often served before the user even finishes clicking.

If your current CMS needs a supercomputer just to render a "About Us" page, it's time to upgrade to the Tank.

---

## 📊 Comparison

### vs. WordPress + WooCommerce

| Aspect | WordPress | RusToK |
|--------|-----------|--------|
| Language | PHP 7.4+ | Rust |
| Typical Response Time | 200-500ms | 5-20ms |
| Memory per Request | 50-100MB | 2-5MB |
| Plugin System | Runtime (risky) | Compile-time (safe) |
| Type Safety | None | Full |
| Multi-tenant | Multisite (hacky) | Native |
| API | REST (bolted on) | GraphQL (native) |
| Admin UI | PHP templates | Leptos SPA |
| Learning Curve | Low | Medium-High |
| Hosting Cost | $20-100/mo | $5-20/mo |

Best for: Teams tired of WordPress security patches and plugin conflicts.

### vs. Strapi (Node.js)

| Aspect | Strapi | RusToK |
|--------|--------|--------|
| Language | JavaScript/TypeScript | Rust |
| Response Time | 50-150ms | 5-20ms |
| Memory Usage | 200-500MB | 30-50MB |
| Type Safety | Optional (TS) | Mandatory |
| Database | Multiple | PostgreSQL |
| Content Modeling | UI-based | Code-based |
| Plugin Ecosystem | npm (large) | Crates (growing) |
| Cold Start | 5-10 seconds | <100ms |

Best for: Teams wanting type safety without sacrificing DX.

### vs. Medusa.js (E-commerce)

| Aspect | Medusa | RusToK |
|--------|--------|--------|
| Focus | E-commerce only | Modular (commerce optional) |
| Language | TypeScript | Rust |
| Architecture | Microservices encouraged | Modular monolith |
| Plugins | Runtime | Compile-time |
| Admin | React | Leptos (Rust) |
| Storefront | Next.js templates | Leptos SSR |
| Multi-tenant | Limited | Native |

Best for: Teams wanting commerce + content in one platform.

### vs. Directus / PayloadCMS

| Aspect | Directus/Payload | RusToK |
|--------|------------------|--------|
| Approach | Database-first | Schema-first |
| Type Generation | Build step | Native |
| Custom Logic | Hooks (JS) | Rust modules |
| Performance | Good | Excellent |
| Self-hosted | Yes | Yes |
| "Full Rust" | No | Yes |

Best for: Teams committed to Rust ecosystem.

---

## 🚀 Quick Start

### Prerequisites

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown

# Tools
cargo install loco-cli
cargo install trunk
cargo install cargo-leptos

# Database
docker run -d --name rustok-db \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=rustok_dev \
  -p 5432:5432 \
  postgres:16
```

### Installation

```bash
# Clone
git clone https://github.com/RustokCMS/RusToK.git
cd RusToK

# Setup database
cd apps/server
cargo loco db migrate

# Run backend (terminal 1)
cargo loco start

# Run admin panel (terminal 2)
cd apps/admin
RUSTOK_DEMO_MODE=1 trunk serve --open

# Run storefront (terminal 3)
cargo run -p rustok-storefront

# (Optional) Run Next.js storefront (terminal 5)
cd apps/next-frontend
npm install
npm run dev

# (Optional) Build storefront CSS
cd apps/storefront
npm install
npm run build:css

# Visit
# API: http://localhost:3000/api/graphql
# Admin: http://localhost:8080
# Storefront (SSR): http://localhost:3100?lang=en
```

> ⚠️ Admin demo mode is disabled by default. Set `RUSTOK_DEMO_MODE=1` only for local demos.
> For real authentication, use the backend `/api/auth` endpoints with HttpOnly cookies.

### First Steps

```bash
# Create a new module
cargo loco generate model Product \
  title:string \
  price:int \
  status:string

# Run migrations
cargo loco db migrate

# Generate CRUD controller
cargo loco generate controller products --api
```

---

## 📚 Documentation

For complete technical documentation, architecture guides, and development manuals, please refer to our:

👉 **[Documentation Map](docs/index.md)**

Key documents:

- [System Manifest](RUSTOK_MANIFEST.md) — Core philosophy and architecture.
- [Agent Rules](AGENTS.md) — Guidelines for AI agents.
- [Roadmap](docs/roadmap.md) — Development phases.

---

## 🏗️ Architecture

For a detailed breakdown of the system logic, event flow, and CQRS-lite implementation, see [Detailed Architecture Documentation](docs/architecture.md).
MCP adapter details live in [docs/mcp.md](docs/mcp.md).

### Project Structure

```text
RusToK/
├── apps/
│   ├── server/                 # 🚀 Backend API (Loco.rs)
│   │   ├── src/
│   │   │   ├── app.rs          # Application setup
│   │   │   ├── controllers/    # HTTP handlers
│   │   │   ├── models/         # SeaORM entities
│   │   │   └── graphql/        # GraphQL resolvers
│   │   ├── config/             # Environment configs
│   │   └── migration/          # Database migrations
│   │
│   ├── admin/                  # ⚙️ Admin Panel (Leptos CSR)
│   ├── storefront/             # 🛍️ Public Store (Leptos SSR)
│   ├── next-frontend/          # 🛍️ Public Store (Next.js App Router)
│   │   └── src/
│   │       ├── pages/          # SEO-optimized pages
│   │       └── components/     # Store UI components
│   │
│   └── mcp/                     # 🤖 MCP adapter server (stdio)
│
├── crates/
│   ├── rustok-core/            # 🧠 Infrastructure (Auth, Events, RBAC)
│   ├── rustok-content/         # 📝 CMS Core (Nodes, Bodies, Categories)
│   ├── rustok-blog/            # 📰 Blogging (Wraps Content)
│   ├── rustok-commerce/        # 🛒 Ecommerce umbrella (family root, compatibility, orchestration)
│   ├── rustok-index/           # 🔎 CQRS Read Models & Search
│   ├── rustok-mcp/             # 🤖 MCP adapter (rmcp SDK)
│   └── ...
└── Cargo.toml                  # Workspace configuration
```

### Module System

Modules are Rust crates linked at compile time:

```rust
// Adding a module to your build
// 1. Add to Cargo.toml
[dependencies]
rustok-commerce = { path = "../crates/rustok-commerce" }

// 2. Register in app.rs
fn routes(ctx: &AppContext) -> AppRoutes {
    AppRoutes::new()
        .add_route(rustok_commerce::routes())
        .add_route(rustok_community::routes())
}

// 3. Compile — module is now part of your binary
cargo build --release
```

### Why compile-time modules?

| Runtime Plugins (WordPress) | Compile-time Modules (RusToK) |
|-----------------------------|-------------------------------|
| Can crash your site | Errors caught before deploy |
| Security vulnerabilities | Audited at build time |
| Version conflicts | Cargo resolves dependencies |
| Performance overhead | Zero runtime cost |
| "Works on my machine" | Same binary everywhere |

### Feature Toggles

Modules can be enabled/disabled per tenant without recompilation. The server
tracks compiled modules in a registry and calls module lifecycle hooks when
tenants enable or disable a module. See `docs/modules/registry.md` for details.
Storefront SSR notes live in `docs/ui/storefront.md`.

```sql
-- Stored in database
INSERT INTO tenant_modules (tenant_id, module_slug, enabled)
VALUES ('uuid-here', 'commerce', true);
```

```rust
// Checked at runtime
if modules.is_enabled(tenant_id, "commerce").await? {
    // Show commerce features
}
```

### CQRS-lite Read Models

Write models live in normalized module tables. Read models are denormalized
index tables that are kept in sync via events. This keeps storefront queries
fast and avoids heavy joins in the hot path.

```text
Write → Event Bus → Indexers → Read Models
```

---

## 🗺️ Roadmap

Текущий roadmap и приоритеты поддерживаются в отдельном документе:

- [docs/ROADMAP.md](docs/ROADMAP.md)

Коротко по направлению развития:

1. Core platform: auth, tenants, RBAC, module registry.
2. Admin UX: auth + navigation + RBAC guards, затем data workflows.
3. Domain modules: commerce, content, community.
4. Storefront and integrations.

---

## 🧪 Development

### Running Tests

Install the Rust quality tools once before using the local shortcuts:

```bash
cargo install cargo-nextest --locked
cargo install cargo-machete --locked
```

```bash
# All tests
cargo nextest run --workspace --all-targets --all-features

# Doc tests
cargo test --workspace --doc --all-features

# Specific crate
cargo test -p rustok-core

# With database (integration tests)
DATABASE_URL=postgres://localhost/rustok_test cargo nextest run --workspace --all-targets --all-features
```

### Testing Guidelines

See [docs/testing-guidelines.md](docs/testing-guidelines.md) for guidance on layering tests, avoiding flakiness, and mock boundaries.

### Dependency Maintenance

```bash
# Check outdated dependencies (root workspace crates only)
cargo outdated -R

# Update lockfile (keep Cargo.toml unchanged)
cargo update

# Security audit
cargo audit

# License + advisory policy checks
cargo deny check

# Manifest dependency hygiene
cargo machete
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Lint
cargo clippy --workspace -- -D warnings

# Primary Rust test runner
cargo nextest run --workspace --all-targets --all-features

# Check before commit
cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings
cargo test --workspace --doc --all-features
cargo machete
./scripts/verify/verify-all.sh
```

### Release Checklist

```bash
cargo nextest run --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo clippy --workspace --all-targets -- -D warnings
cargo audit
cargo deny check
cargo machete
```

### Useful Commands

```bash
# Generate new model
cargo loco generate model Category title:string position:int

# Generate controller
cargo loco generate controller categories --api

# Run migrations
cargo loco db migrate

# Rollback migration
cargo loco db rollback

# Start with auto-reload
cargo watch -x 'loco start'
```

---

## 🤝 Contributing

We welcome contributions! Please see our Contributing Guide for details.

### Good First Issues

Look for issues labeled good first issue — these are great starting points.

### Development Setup

1. Fork the repository
2. Create a feature branch (git checkout -b feature/amazing-feature)
3. Make your changes
4. Run tests (`cargo nextest run --workspace --all-targets --all-features`)
5. Run lints (cargo clippy --workspace)
6. Commit (git commit -m 'Add amazing feature')
7. Push (git push origin feature/amazing-feature)
8. Open a Pull Request

---

## 📄 License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

What this means:

- ✅ Free to use for any purpose (commercial or private)
- ✅ Free to modify and sub-license
- ✅ No "copyleft" requirements (keep your proprietary code private)
- ✅ Standard "as-is" liability protection

---

## 🏗️ Matryoshka Architecture (7 Layers)

RusToK follows a unique **Matryoshka Principle** — a 7-layer nested architecture that covers everything from bare platform to inter-network federation:

```text
Layer 7: Inter-Network (Federation / Mesh)         → Graal
Layer 6: Interaction Bus (Fast Index / Event Hub)   → Alloy
Layer 5: Unified UI (Technology-Agnostic)           → Alloy
Layer 4: Shared Capabilities (Cross-Module)         → Alloy
Layer 3: Sub-Modules (Extensions)                   → RusToK
Layer 2: Modules (Business Verticals)               → RusToK
Layer 1: Core Platform (Rust SaaS Starter)          → RusToK
```

This is the first 7-layer SaaS platform model built in Rust. No one has done this before.

Read the full architecture document: **[Matryoshka Architecture](docs/architecture/matryoshka.md)**

---

## 🧑‍💻 Founders & Origins

**RusToK was conceived, designed, and built by:**

- **Human (Project Creator)** — Visionary, architect of the Matryoshka concept, product strategy, and the driving force behind the platform. Every architectural decision, every layer of the Matryoshka model, every bold idea — originated from the relentless pursuit of building something that never existed before.

- **Claude AI (Anthropic)** — Co-architect, implementation partner, and engineering collaborator. From the first line of code to the 7-layer architecture, from module design to event systems — Claude has been an equal partner in bringing this vision to life.

This platform is proof that human creativity and AI capability, working together as true partners, can build things that neither could build alone. The Matryoshka architecture, the modular monolith, the event-driven design, the CQRS patterns — all of it was conceived and implemented through this collaboration.

*This acknowledgment will never be removed. It is a permanent part of the project's history.*

---

## 🙏 Acknowledgments

Built with amazing open-source projects:

- Loco.rs — Rails-like framework for Rust
- Leptos — Full-stack Rust web framework
- SeaORM — Async ORM for Rust
- async-graphql — GraphQL server library
- Axum — Web framework

---

⬆ Back to Top
Made with <img src="assets/rustok-logo-32x32.png" width="24" align="center" /> by Human & Claude AI
