---
id: doc://docs/architecture/distribution-profiles.md
doc_type: current_contract
status: current
owner: platform-architecture
canonical_for:
  - distribution-profiles
language: markdown
---

# RusToK Distribution Profiles Architecture

## 1. Context and Problem Statement

RusToK is an enterprise-grade modular monolith comprising 80+ workspace packages and modules covering diverse business domains: core identity/tenancy, high-integrity e-commerce, content management, social community, marketplace economics, AI automation, and highload enterprise collaboration (SuperApp).

### The Monolithic Build Bottleneck
Compiling all 80+ modules on every developer machine, agent run, or CI gate causes severe operational friction:
- **Build Times:** Full workspace compilation can consume 80%+ of developer and agent turnaround time (often 10–15+ minutes on Windows workstations).
- **Resource Contention:** Multiple concurrent Cargo processes lock the `target/` directory and saturate CPU/disk I/O.
- **Binary Footprint:** Deploying a full monolith for a specialized workload (e.g. headless auth gateway or a focused e-commerce shop) unnecessarily bloats binary size and memory consumption.
- **Blast Radius:** Minor refactorings in decoupled modules (e.g. `rustok-blog`) trigger recompilations of unrelated subsystems if everything is bundled into a single monolithic target.

RusToK addresses this through **First-Class Distribution Profiles (Сборки платформы)**.

---

## 2. Canonical Distribution Profiles

RusToK defines 6 standard platform distribution profiles:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           PROFILE MATRIX                                    │
├──────────────┬─────────────────────────────┬────────────────────────────────┤
│ Profile      │ Primary Target Workload     │ Domain Scope                   │
├──────────────┼─────────────────────────────┼────────────────────────────────┤
│ core         │ Minimal Headless Kernel     │ Auth, Tenant, RBAC, Events     │
│ ecommerce    │ Retail, B2B Shop, D2C       │ Core + Products, Orders, Pay   │
│ content      │ Editorial CMS, Media, Blogs │ Core + Pages, Blog, SEO, Media │
│ social       │ Forums & Community          │ Core + Profiles, Groups, Forum │
│ superapp     │ Enterprise Workplace        │ Core + Messenger, Docs, Calls  │
│ global       │ Enterprise Monolith         │ Complete Catalog (All Modules) │
└──────────────┴─────────────────────────────┴────────────────────────────────┘
```

### 2.1 Profile: `core` (Чистое ядро / Headless Kernel)
- **Target:** Microservice auth provider, headless API gateway, tenant management node.
- **Included Modules:**
  - `rustok-auth`
  - `rustok-tenant`
  - `rustok-rbac`
  - `rustok-channel`
  - `rustok-cache`
  - `rustok-outbox`
  - `rustok-events-module`
  - `rustok-email`
  - `rustok-index`
  - `rustok-search`
  - `rustok-modules`
- **Excluded:** All domain commerce, content, social, and collaboration modules.
- **Characteristics:** Ultra-fast build (< 30s), minimal memory usage, zero external domain dependencies.

### 2.2 Profile: `ecommerce` (E-Commerce & Digital Retail)
- **Target:** Modern online stores, multi-region storefronts, B2B procurement portals.
- **Included Modules:**
  - *All `core` modules*
  - **Catalog & Merchandising:** `rustok-product`, `rustok-brand`, `rustok-product-relations`, `rustok-product-bundles`, `rustok-taxonomy`
  - **Pricing & Availability:** `rustok-pricing`, `rustok-inventory`, `rustok-region`
  - **Checkout & Transactions:** `rustok-cart`, `rustok-customer`, `rustok-order`, `rustok-payment`, `rustok-fulfillment`, `rustok-commerce`
  - **Assets & Comms:** `rustok-media`, `rustok-storage`, `rustok-notifications`
- **Excluded:** Messenger, collaborative docs, video calls, blog, forum, marketplace allocation/commission.

### 2.3 Profile: `content` (CMS & Editorial Platform)
- **Target:** Corporate portals, content publishing, knowledge bases, marketing websites.
- **Included Modules:**
  - *All `core` modules*
  - **Publishing:** `rustok-content`, `rustok-pages`, `rustok-page-builder`, `rustok-blog`
  - **Rich Media & Assets:** `rustok-media`, `rustok-storage`, `rustok-taxonomy`
  - **Interactivity:** `rustok-comments`, `rustok-forms`, `rustok-seo`
  - **Localization:** `rustok-translation`
- **Excluded:** Cart, checkout, payments, inventory, messenger, video calls.

### 2.4 Profile: `social` (Community & Social Hub)
- **Target:** Public or private community forums, member portals, discussion clubs.
- **Included Modules:**
  - *All `core` modules*
  - **Social Graph & Identity:** `rustok-profiles`, `rustok-social-graph`, `rustok-groups`, `rustok-reactions`
  - **Discussions:** `rustok-forum`, `rustok-comments`, `rustok-moderation`
  - **Notifications:** `rustok-notifications`, `rustok-media`
- **Excluded:** E-commerce carts/orders, Notion-grade docs, video calls.

### 2.5 Profile: `superapp` (Workplace & Collaboration SuperApp)
- **Target:** Enterprise communication, internal operations, team collaboration (governed by [Workplace Collaboration SuperApp Plan](./workplace-collaboration-superapp-plan.md)).
- **Included Modules:**
  - *All `core` modules*
  - **Highload Messaging:** `rustok-messenger` (Iggy stream fabric, monotonic cursors)
  - **Collaborative Docs:** `rustok-docs` (Loro CRDT, live blocks)
  - **Conferencing & Audio:** `rustok-calls` (LiveKit SFU integration)
  - **Scheduling:** `rustok-calendar` (CalDAV, booking slots)
  - **Issue Tracking:** `rustok-tasks` (Linear-like issue workflows)
  - **Identity & AI:** `rustok-profiles`, `rustok-social-graph`, `rustok-reactions`, `rustok-notifications`, `rustok-ai`, `rustok-mcp`, `rustok-media`
- **Excluded:** Full retail shopping carts, external public checkout.

### 2.6 Profile: `global` (Full Enterprise Monolith)
- **Target:** Comprehensive enterprise deployments, unified local development sandbox, full-suite CI verification.
- **Included Modules:** Union of all supported platform modules:
  - `core` + `ecommerce` + `content` + `social` + `superapp` + `marketplace` (multi-vendor seller, listing, commissions, payouts) + `workflow` + `accounting` + `alloy`.

---

## 3. Module Composition Matrix

| Module | `core` | `ecommerce` | `content` | `social` | `superapp` | `global` |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `auth` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `tenant` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `rbac` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `channel` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `cache` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `outbox` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `events` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `search` / `index` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `email` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `product` / `pricing` / `inventory` | ❌ | ✅ | ❌ | ❌ | ❌ | ✅ |
| `cart` / `order` / `payment` / `fulfillment` | ❌ | ✅ | ❌ | ❌ | ❌ | ✅ |
| `content` / `pages` / `blog` | ❌ | ❌ | ✅ | ❌ | ❌ | ✅ |
| `page-builder` / `seo` / `forms` | ❌ | ❌ | ✅ | ❌ | ❌ | ✅ |
| `profiles` / `social-graph` / `groups` | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ |
| `forum` / `moderation` | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ |
| `messenger` | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
| `docs` / `calls` / `calendar` / `tasks` | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
| `ai` / `mcp` | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
| `marketplace-*` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| `workflow` / `accounting` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |

---

## 4. Technical Implementation Mechanisms

RusToK implements profile builds via a **two-tier architecture**:

### 4.1 Tier 1: Cargo Feature Bundles in `apps/server`

The server host provides top-level meta-features representing each profile:

```toml
# apps/server/Cargo.toml

[features]
default = ["profile-core"] # Safe minimal default

# Profile Meta-Features
profile-core = [
    "redis-cache",
    "mod-auth", "mod-tenant", "mod-rbac", "mod-channel",
    "mod-cache", "mod-outbox", "mod-events", "mod-email",
    "mod-index", "mod-search",
]

profile-ecommerce = [
    "profile-core",
    "mod-product", "mod-brand", "mod-product_relations", "mod-product_bundles",
    "mod-pricing", "mod-inventory", "mod-region",
    "mod-cart", "mod-customer", "mod-order", "mod-payment", "mod-fulfillment",
    "mod-commerce", "mod-media", "mod-notifications",
]

profile-content = [
    "profile-core",
    "mod-content", "mod-pages", "mod-page_builder", "mod-blog",
    "mod-comments", "mod-forms", "mod-seo", "mod-media", "mod-translation",
]

profile-social = [
    "profile-core",
    "mod-profiles", "mod-social_graph", "mod-groups", "mod-reactions",
    "mod-forum", "mod-comments", "mod-moderation", "mod-notifications",
]

profile-superapp = [
    "profile-core",
    "mod-messenger", "mod-docs", "mod-calls", "mod-calendar", "mod-tasks",
    "mod-profiles", "mod-social_graph", "mod-reactions", "mod-notifications",
    "mod-ai", "mod-mcp", "mod-media",
]

profile-global = [
    "profile-ecommerce",
    "profile-content",
    "profile-social",
    "profile-superapp",
    "mod-marketplace", "mod-accounting", "mod-workflow", "mod-alloy",
]
```

### 4.2 Tier 2: Manifest Presets (`modules.<profile>.toml`)

Runtime registry and UI embedding contracts are configured through manifest presets:
- `modules.core.toml`
- `modules.ecommerce.toml`
- `modules.content.toml`
- `modules.superapp.toml`
- `modules.global.toml` (canonical `modules.toml`)

Developers and operators select a profile using `xtask`:
```powershell
cargo xtask profile apply ecommerce
# Automatically updates active modules.toml and validates dependency graphs
```

---

## 5. Developer & AI Agent Workflows

### 5.1 Scoped Verification First
When working on an individual module, developers and agents **MUST NOT** compile the full profile.
Instead, verify strictly at the crate boundary:
```powershell
# Fast local check: ~2-5 seconds
cargo check -p rustok-messenger
cargo test -p rustok-messenger --lib
```

### 5.2 Compiling a Specific Server Profile
When verifying server composition for a specific workload:
```powershell
# Check Ecommerce server only (does not pull blog, forum, messenger, etc.)
cargo check -p rustok-server --no-default-features --features profile-ecommerce

# Check SuperApp server only (does not pull ecommerce checkout, order, catalog)
cargo check -p rustok-server --no-default-features --features profile-superapp
```

---

## 6. Evolution and Maintenance

1. **Adding a New Module:**
   When introducing a new module via `docs/modules/module-authoring.md`, the author MUST update this document's **Module Composition Matrix** (§3) and assign the module to its owning profile.
2. **Profile Invariants:**
   Modules inside a specialized profile (e.g. `ecommerce`) MUST NOT introduce hidden dependencies on modules in disjoint profiles (e.g. `messenger` or `forum`). Cross-domain communication must strictly use events or typed extension seams.
