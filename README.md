<div align="center">

# <img src="assets/rustok-logo-512x512.png" width="72" align="center" /> RusTok

**The platform that builds anything with data. Built to last.**

*Content · Commerce · Community · Workflow · One runtime, zero compromises.*

[![CI](https://github.com/RustokCMS/RusToK/actions/workflows/ci.yml/badge.svg)](https://github.com/RustokCMS/RusToK/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

**[Русская версия](README.ru.md)** | **[Quick Platform Info (RU)](PLATFORM_INFO_RU.md)**

</div>

---

## Why "RusTok"?

**Rust + Tokio** — the name is right there in the product.

**Rust** is the language that eliminates entire categories of bugs before the program ever runs. No null pointer crashes, no memory leaks, no silent data corruption, no "works on my machine." The compiler is the first line of defense, and it does not negotiate.

**Tokio** is the async runtime that sits beneath everything and acts as the engine. While most platforms struggle to handle a few hundred simultaneous requests before reaching for extra servers, Tokio-backed services routinely handle tens of thousands of concurrent connections on a single machine — without thread pools, without GC pauses, without drama.

Together they produce something that feels almost unfair: a platform that starts in 50 milliseconds, handles 45,000+ requests per second, and catches type errors, missing fields, and domain contract violations at compile time rather than at 2 AM in production.

---

## What is RusTok?

RusTok is a modular platform for building any product that has data. Not just a CMS. Not just an online store. A platform where you pick the modules you need — content, commerce, community, workflow, integrations — and they assemble into one coherent runtime.

Think of it like Lego for backend systems: each module is a self-contained brick with its own data model, API surface, and UI. Modules know how to talk to each other through typed events. The entire structure is verified at compile time — not at runtime, not with plugins that might break on the next update. If it compiles, the contracts are sound.

RusTok is designed for teams that are tired of duct-taping multiple platforms together, paying for SaaS services that charge per seat, or inheriting a codebase where "just add a plugin" has become an act of courage.

---

## What can you build?

If it has data, you can build it on RusTok. Here are some examples — and this list barely scratches the surface:

**Stores & Commerce**
- An online store with product catalog, variant pricing, inventory tracking, multi-currency checkout, and fulfillment workflows — all in one platform, not five integrations stitched together.
- A marketplace where multiple vendors sell under one roof, each with their own products, pricing zones, and order flows.
- A B2B platform with customer-specific pricing, regional rules, and approval workflows before orders go through.

**Content & Media**
- A blog or editorial publication with authored content, rich media, categories, tags, comment threads, and a full-text search that actually works.
- A news portal or magazine with scheduled publishing, editorial workflows, and localized content for different regions.
- A documentation hub or knowledge base where pages, navigation, and search are first-class citizens.
- A media asset library where images, videos, and files are stored, tagged, versioned, and served through a unified API.

**Community & Social**
- A forum or discussion platform with categories, moderated topics, threaded replies, and user profiles.
- A community around a product — where customers can ask questions, share reviews, post in forums, and earn reputation — all integrated with the same store they buy from.
- A membership site where access to content, forums, and features is gated by subscription tier.

**SaaS & Multi-tenant Products**
- A multi-tenant SaaS where each client gets their own isolated workspace with independent module configuration, their own users, roles, and data — without separate deployments.
- A white-label platform where different tenants run different feature sets: one has commerce enabled, another runs content-only, a third has both plus workflow automation.
- An internal platform where multiple teams share infrastructure but operate in isolated namespaces.

**Workflow & Automation**
- A business process platform where actions trigger events, events trigger workflows, and workflows can call webhooks, send emails, update records, or notify other systems.
- An operations tool where order status changes, inventory alerts, and fulfillment events flow through automated pipelines instead of manual processes.
- An integration hub where external systems push data in via webhooks and pull results out via REST or GraphQL.

**APIs & Headless Backends**
- A headless backend for a mobile app, where the platform handles auth, data, search, and file storage while the app team owns the UI completely.
- A GraphQL API server for a React/Vue/Svelte frontend, with full RBAC, multi-tenancy, and event-driven writes baked in.
- A backend for a desktop application, IoT dashboard, or any system that needs structured data, roles, and real-time updates.

**Enterprise & Highload Platforms**
- A full **CRM** — leads, deals, pipelines, contacts, companies, activity logs, forecasts, and team collaboration — where every customer interaction feeds into automated follow-up workflows and the entire sales funnel is queryable in real time across tens of millions of records.
- An **ERP core** — finance, HR, procurement, warehouse, and production management — built on the same module system, where adding a new business domain means adding a module, not spinning up a new service with a new team.
- A **pharma or life-sciences platform** — batch records, clinical trial data, regulatory submission tracking, full audit trails, and controlled-access data rooms — where the type-safe, tenant-isolated architecture maps directly to compliance requirements.
- A **financial services backend** — transaction ledgers, compliance reporting, multi-currency accounts, risk scoring, and customer KYC flows — where single-digit millisecond latency and tamper-evident event logs are non-negotiable.
- A **logistics and supply chain platform** — shipment tracking, carrier integrations, warehouse operations, route feeds, and SLA monitoring — where thousands of status events per second need to be processed without queuing nightmares.
- A **healthcare data platform** — patient records, appointment scheduling, billing, referral workflows, and access control — where multi-tenancy maps naturally to clinics or hospital networks, and data isolation is a regulatory requirement, not a feature request.
- A **real estate management system** — listings, leads, transaction pipelines, document workflows, agent performance, and portal feeds — running across multiple brands or regions in a single deployment.
- A **media and publishing platform at scale** — ingesting thousands of articles per day, running editorial workflows, serving content to millions of readers, with per-region localization and search that keeps pace with write velocity.
- A **developer infrastructure platform** — feature flags, A/B experiments, deployment metadata, usage analytics, and API rate limiting — the kind of internal tooling that large technology companies build and rebuild, available as a configurable module set.

The common thread: if your product has users, data, and business rules — RusTok gives you the foundation instead of forcing you to build it from scratch or stitch together cloud services.

---

## Alloy — Logic Without Deployments

Every platform eventually runs into the same wall: the business wants to change how something works, but making that change requires a developer, a pull request, a code review, a deployment, and a maintenance window. For a pricing rule. For an input validation. For a notification message.

**Alloy** is how RusTok breaks that wall — without sacrificing type safety, auditing, or platform stability.

Alloy is a scripting runtime embedded directly in the platform. Business logic lives in scripts stored in the database, activated instantly, and executed inside a strict sandbox where they can read and modify records but cannot harm the system. No deployment. No downtime. No risk of a pricing-rule change breaking unrelated code.

### When scripts run

Scripts attach to the lifecycle of any entity in the platform and fire at precisely defined moments:

| Trigger | When | What you can do |
|---------|------|-----------------|
| **Before create / update / delete** | Before the record hits the database | Validate fields, normalize data, calculate values, reject the operation |
| **After create / update / delete** | After the record is saved | Send notifications, create follow-up records, trigger side effects |
| **On commit** | After the transaction is confirmed | Call external APIs, sync to third-party systems, push to event queues |
| **Cron schedule** | On a timer | Generate reports, clean up stale data, renew subscriptions, send digests |
| **Manual trigger** | On demand | Recalculate a batch, re-send a failed sync, migrate a dataset |

### What a script looks like

Scripts are written in **Rhai** — a sandboxed scripting language that reads like simplified Rust and is safe for non-Rust developers to write and deploy:

```rhai
// Before creating an order: validate, enrich, and protect
if entity["total"] < 0 {
    abort("Order total cannot be negative");
}

if entity["customer_tier"] == "vip" {
    entity["discount"] = 15;
    entity["priority_fulfillment"] = true;
}

validate_email(entity["contact_email"]);
log("Order pre-processed: " + entity["customer_id"]);
```

The sandbox enforces hard limits on execution time, operation count, and memory. A runaway script cannot take down the platform. If a script calls `abort()`, the operation is rejected cleanly with the reason returned to the caller.

### Integration superpowers

Scripts in the `OnCommit` phase have outbound HTTP access. This is how RusTok connects to the outside world without hard-coding integrations into the platform:

- **Payment processors** — confirm charges, handle refunds, record receipts
- **CRM systems** — push deal updates to Salesforce, HubSpot, or any REST-based CRM
- **Accounting software** — export transactions to 1С, QuickBooks, or any API-accessible ledger
- **Warehouse and logistics** — confirm shipments, update inventory in partner systems
- **Communication channels** — send Slack messages, trigger SMS via Twilio, post to any webhook receiver
- **Data pipelines** — stream events to ClickHouse, BigQuery, or any analytics warehouse endpoint

An integration that would take a sprint to build as a native module takes minutes as an Alloy script. When it stabilizes, it can graduate.

### From script to native module

When a script has proven its value and runs thousands of times a day, it can be promoted: the same logic rewritten as a native Rust module, compiled into the platform binary, with zero scripting overhead. Alloy is the prototyping and automation layer; native modules are the production layer. The path between them is intentional and explicit.

### Full observability

Every script execution is recorded: when it ran, how long it took, what entity it processed, what it changed, and whether it succeeded, was rejected by `abort()`, or failed with an error. Execution logs are queryable via API. Scripts have explicit statuses — `Draft`, `Active`, `Paused`, `Archived` — making the standard workflow: write in Draft, test, activate when confident.

### AI-generated logic

Alloy scripts are short, structured, and purpose-driven — exactly what AI tools generate well. Describe what you want, get a script, validate it in the sandbox, activate it. The feedback loop from idea to running business logic shrinks from days to minutes.

---

## Why RusTok over other platforms?

Most platforms make a trade-off: they are easy to start with, but painful to scale, extend, or maintain as requirements grow. RusTok makes a different trade-off: the initial investment is in Rust and a compiled architecture, and the payoff is a platform that stays fast, stays correct, and stays under control.

### The speed gap is real

| Metric | Interpreted platforms | RusTok |
|--------|----------------------|--------|
| **Req/sec** | 60 – 800 | **45,000+** |
| **P99 Latency** | 120 – 450ms | **8ms** |
| **Cold Boot** | 1 – 8.5 seconds | **0.05 seconds** |

This is not about benchmarks for their own sake. It means smaller servers, lower cloud bills, and a product that stays responsive under real traffic spikes — without a CDN layer doing the heavy lifting.

### Safety that does not require discipline

Other platforms rely on developer discipline: remember to validate input, remember to handle null, remember to check permissions. In RusTok, the type system enforces these at compile time. Permission-aware contracts, tenant isolation, and domain boundaries are part of the code structure, not a convention in a wiki.

### Multi-tenancy as a first-class citizen

Most platforms add multi-tenancy as an afterthought — a `tenant_id` column bolted onto every table, access checks sprinkled in manually. RusTok is built around `rustok-tenant` from day one: tenant context flows through every request, module enablement is per-tenant, and isolation is a platform guarantee rather than a dev practice.

### Modular, not all-or-nothing

Platforms that bundle everything together make you pay for what you do not use: memory, startup time, attack surface, complexity. RusTok's modules are explicit compile-time dependencies declared in `modules.toml`. Want just content and search? Done. Want to add commerce six months later? Enable the module — the contracts are already there.

### One platform, many frontends

RusTok does not pick sides in the frontend war. The integrated path uses **Leptos** — a Rust/WASM framework that runs in the same type system as the backend. The headless path exposes the same data through GraphQL and REST for any frontend: Next.js, mobile apps, desktop clients, third-party tools. Or both at once — integrated admin panel, external customer app, same runtime.

### Comparison at a glance

| Capability | Typical CMS | Typical e-commerce platform | Headless CMS | RusTok |
|---|---|---|---|---|
| Integrated deployment | yes | partial | no | **yes** |
| Headless API surface | partial | limited | yes | **yes** |
| Integrated + headless simultaneously | rarely | no | no | **yes** |
| Native multi-tenancy | no | limited | no | **yes** |
| Compile-time module composition | no | no | no | **yes** |
| Content + Commerce + Community in one runtime | no | no | no | **yes** |
| Rust performance baseline | no | no | no | **yes** |

---

## Platform architecture

### Three ways to deploy

| Mode | What it means |
|------|---------------|
| **Integrated** | Server plus Leptos admin and storefront — everything under one roof, shared sessions, shared runtime |
| **Headless** | API-only server, frontend lives anywhere — mobile app, external site, third-party tool |
| **Mixed** | Integrated Leptos hosts for your team, external clients for your customers — same runtime, different surfaces |

### Applications

| Path | Role |
|---|---|
| `apps/server` | Composition root — HTTP, GraphQL, auth, RBAC, events, manifest validation |
| `apps/admin` | Leptos admin panel (integrated path) |
| `apps/storefront` | Leptos customer storefront (integrated path) |
| `apps/next-admin` | Next.js admin (headless path) |
| `apps/next-frontend` | Next.js storefront (headless path) |

### Module taxonomy

`modules.toml` is the source of truth for what is in the platform.

**Core modules** — always present, the foundation everything else builds on:

`auth` · `cache` · `channel` · `email` · `index` · `search` · `outbox` · `tenant` · `rbac`

**Optional modules** — enabled per-tenant, composed at build time:

*Content & Community:* `content` · `blog` · `comments` · `forum` · `pages` · `media` · `workflow`

*Commerce family:* `cart` · `customer` · `product` · `profiles` · `region` · `pricing` · `inventory` · `order` · `payment` · `fulfillment` · `commerce`

**Capability & support crates** — shared infrastructure across all modules:

*Shared:* `rustok-core` · `rustok-api` · `rustok-events` · `rustok-storage` · `rustok-commerce-foundation` · `rustok-telemetry`

*Runtime capabilities:* `rustok-mcp` · `alloy` · `alloy-scripting` · `flex` · `rustok-iggy` · `rustok-iggy-connector`

---

## How the module system works

Every module in `modules.toml` flows through the same pipeline:

```text
modules.toml
  → build.rs generates host wiring
  → apps/server validates the manifest at startup
  → ModuleRegistry bootstraps the runtime
  → per-tenant enablement activates optional modules
```

This means:
- **Build composition** decides what code is compiled into the binary. Unused modules are not there — no dead code, no extra attack surface.
- **Tenant enablement** decides which optional modules are active for a given customer at runtime. One binary, many configurations.

---

## AI-ready by design

RusTok ships with a built-in **Model Context Protocol (MCP)** server via `rustok-mcp`. This means AI agents and LLM tools can interact with the platform directly — query data, trigger workflows, inspect module state — through a typed protocol rather than raw API calls.

Beyond MCP, the platform is structured for agent-assisted development: explicit module contracts, a documentation map at `docs/index.md`, typed event schemas, and `AGENTS.md` rules that make the codebase readable and navigable for automated tools.

---

## Built on solid foundations

RusTok is assembled from well-maintained open-source crates:

- **[Loco.rs](https://loco.rs)** + **[Axum](https://github.com/tokio-rs/axum)** — web framework and HTTP routing
- **[Leptos](https://leptos.dev)** — Rust/WASM frontend framework
- **[SeaORM](https://www.sea-ql.org/SeaORM/)** — async database ORM for PostgreSQL
- **[async-graphql](https://async-graphql.github.io/async-graphql/)** — type-safe GraphQL server
- **[Tokio](https://tokio.rs)** — async runtime (the second half of the name)
- **[Casbin](https://casbin.org)** — flexible RBAC authorization
- **[Iggy](https://iggy.rs)** — event streaming infrastructure

---

## Quick Start

The full local-dev guide lives in [docs/guides/quickstart.md](docs/guides/quickstart.md).

```bash
./scripts/dev-start.sh
```

This starts the full local stack:

| Service | URL |
|---------|-----|
| Backend API | `http://localhost:5150` |
| Leptos Admin | `http://localhost:3001` |
| Leptos Storefront | `http://localhost:3101` |
| Next.js Admin | `http://localhost:3000` |
| Next.js Storefront | `http://localhost:3100` |

---

## Development

Prerequisites:

- Rust toolchain (version from repository `rust-toolchain.toml`)
- PostgreSQL for local runtime
- Node.js or Bun for Next.js hosts
- `trunk` for Leptos hosts

```bash
# run all Rust tests
cargo nextest run --workspace --all-targets --all-features

# doc tests
cargo test --workspace --doc --all-features

# format and lint
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings

# dependency and license checks
cargo deny check
cargo machete
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md) for contributor and agent rules.

---

## Documentation

| Resource | Link |
|----------|------|
| Documentation map | [docs/index.md](docs/index.md) |
| Architecture overview | [docs/architecture/overview.md](docs/architecture/overview.md) |
| Module registry | [docs/modules/registry.md](docs/modules/registry.md) |
| Module docs index | [docs/modules/_index.md](docs/modules/_index.md) |
| System manifest | [RUSTOK_MANIFEST.md](RUSTOK_MANIFEST.md) |
| Module system plan | [docs/modules/module-system-plan.md](docs/modules/module-system-plan.md) |
| Platform verification plan | [docs/verification/PLATFORM_VERIFICATION_PLAN.md](docs/verification/PLATFORM_VERIFICATION_PLAN.md) |
| Testing guide | [docs/guides/testing.md](docs/guides/testing.md) |
| MCP reference | [docs/references/mcp/README.md](docs/references/mcp/README.md) |
| Agent rules | [AGENTS.md](AGENTS.md) |

---

## License

RusTok is released under the [MIT License](LICENSE).
