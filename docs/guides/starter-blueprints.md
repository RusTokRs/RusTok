---
id: doc://docs/guides/starter-blueprints.md
kind: guide
language: markdown
status: verified
owner: platform-devops
---
# Starter Blueprints and Demo Data Guide

This document is the canonical reference for developers and AI agents working with RusToK **Starter Blueprints** (demo data and initial tenant content packs).

It explains how the blueprint engine works, how data is structured and imported, the 4 execution entry points, domain invariants, idempotency guarantees, and operational recipes for AI agents.

---

## 1. Overview and Core Philosophy

In RusToK, demo and starter content is **declarative, typed, and domain-governed**:
- Demo data is defined as structured JSON blueprints (`StarterBlueprint`).
- Content is imported strictly through **public domain module services** (`PageService`, `PostService`, `CategoryService`, `TopicService`, `TaxonomyService`, `NavigationService`).
- **No direct raw SQL inserts**: all business rules, slug allocations, timestamps, revisions, outbox events, and database constraints are respected.
- **Idempotency**: Running an import multiple times on the same tenant will not corrupt data or produce duplicate entities.
- **Multilingual Support**: Supports multi-language demo data, localized pages, menus, and blog posts. See the [Multilingual Demo Guide](./multilingual-demo.md).

---

## 2. Canonical Ownership

| Component | Repository Path | Responsibility |
|---|---|---|
| **Engine & Schema** | [`crates/utils/rustok-starter`](file:///d:/RusTok/crates/utils/rustok-starter) | Defines `StarterBlueprint`, embedded default JSON, and the topological `StarterEngine`. |
| **CLI Commands** | [`crates/utils/rustok-installer-cli`](file:///d:/RusTok/crates/utils/rustok-installer-cli) | Exposes `rustok-cli starter import` and `--starter` flag on `seed apply`. |
| **Server GraphQL API** | [`apps/server/src/graphql/starter.rs`](file:///d:/RusTok/apps/server/src/graphql/starter.rs) | Provides the `importStarter` mutation for web installer and admin console. |
| **Default Content** | [`crates/utils/rustok-starter/src/embedded/default_starter.json`](file:///d:/RusTok/crates/utils/rustok-starter/src/embedded/default_starter.json) | The official demo blueprint containing landing page, blog, forum, and navigation. |
| **Integration Test** | [`crates/utils/rustok-starter/tests/starter_import_integration.rs`](file:///d:/RusTok/crates/utils/rustok-starter/tests/starter_import_integration.rs) | Complete end-to-end import verification against real database migrations. |

---

## 3. Four Execution Entry Points

Developers and agents can trigger starter data imports through 4 distinct entry points:

### 3.1 CLI Standalone Import (`rustok-cli starter import`)

Import a blueprint into an existing tenant:

```bash
# Import the embedded default demo blueprint:
rustok-cli starter import --tenant-slug demo --starter default

# Import a custom JSON blueprint file:
rustok-cli starter import --tenant-slug demo --file ./my-blueprint.json

# Preflight dry-run (validates schema and prerequisites without database writes):
rustok-cli starter import --tenant-slug demo --starter default --dry-run
```

### 3.2 CLI Tenant Provisioning (`rustok-cli seed apply`)

Populate demo data automatically during tenant bootstrap and database seeding:

```bash
rustok-cli seed apply --tenant-slug demo --profile dev --starter default
```

### 3.3 Web Installer & Admin GraphQL Mutation (`importStarter`)

In web installer wizards or the admin backoffice, execute the `importStarter` GraphQL mutation:

```graphql
mutation ImportStarterDemo {
  importStarter(name: "default") {
    tenantId
    blueprintId
    pagesCreated
    blogPostsCreated
    forumTopicsCreated
    durationMs
  }
}
```

*Authorization requirement:* The request must carry an authenticated user with `system:manage`, `tenants:manage`, or `modules:manage` permission. The user's ID is automatically mapped as the initial content author.

### 3.4 Programmatic Rust Engine (Tests, Background Workers)

To import directly from Rust code:

```rust
use rustok_starter::{StarterEngine, embedded};
use rustok_core::security::EffectiveSecurityContext;
use uuid::Uuid;

let engine = StarterEngine::new(db_pool.clone(), event_bus.clone());
let blueprint = embedded::default_starter();
let tenant_id = Uuid::new_v4();
let security = EffectiveSecurityContext::system();

let report = engine.import_blueprint(&tenant_id, blueprint, &security).await?;
tracing::info!(
    "Imported {} pages and {} blog posts in {}ms",
    report.pages_created,
    report.blog_posts_created,
    report.duration_ms
);
```

---

## 4. Topological Import Sequence & Domain Invariants

The `StarterEngine` executes domain drivers in strict topological order to guarantee referential integrity and immediate visibility:

```
[1. Taxonomy Driver]
       │
       ▼
  [2. Pages Driver]  ── (PageBuilderReviewedPublishRuntime -> published)
       │
       ▼
   [3. Blog Driver]  ── (Synthesizes deterministic author if unauthenticated)
       │
       ▼
  [4. Forum Driver]  ── (Categories, topics, initial discussion threads)
       │
       ▼
[5. Navigation Driver] ── (MenuBindingService::bind -> default channel slot)
```

### 4.1 Taxonomy Driver
- Creates vocabularies and root terms.
- Ensures required categories exist before blog posts or products are attached.

### 4.2 Pages Driver & Storefront Routing
- Imports landing pages and informational pages with GrapesJS/Fly builder blocks (HTML, CSS, JSON components).
- **Domain Invariant:** In RusToK, pages created via `PageService::create_page` start in `ContentStatus::Draft`. For demo landing pages to render on the storefront (e.g. `/` route), the driver immediately transitions the page through `service.publish_reviewed` using the canonical `PageBuilderReviewedPublishRuntime` contract (`starter-blueprint-landing` scenario).
- Result: Landing page has `ContentStatus::Published` and is immediately visible to public storefront visitors.

### 4.3 Blog Driver & Author Identity
- Imports categories, tags, and articles.
- Supports rich-text bodies (`markdown`, `html`, or `json_blocks`).
- **Domain Invariant:** `rustok-blog` enforces `BlogError::AuthorRequired` when creating articles. If the starter import runs in CLI or automated background mode where `security.user_id` is `None`, the driver synthesizes a deterministic fallback author UUID (`Uuid::from_u128(0x01)`) to preserve database integrity.

### 4.4 Forum Driver
- Imports discussion categories, initial topics, and initial replies.
- Creates active community threads so the forum UI is populated out-of-the-box.

### 4.5 Navigation Driver & Channel Slot Auto-Binding
- Imports header and footer menus and their hierarchical navigation items.
- **Domain Invariant:** Navigation menus are tenant-scoped, but storefront layouts resolve menus via **Channel Slots** (e.g. slot `header` on channel `default`).
- The driver creates the menu and immediately calls `MenuBindingService::bind(tenant_id, channel_id, location, menu_id)` so the storefront header displays the menu items instantly.

---

## 5. Blueprint JSON Specification

Starter blueprints conform to the following schema:

```json
{
  "schema_version": "1.0",
  "metadata": {
    "id": "default-starter",
    "name": "RusToK Default Starter Pack",
    "version": "1.0.0",
    "author": "RusToK Core Team",
    "description": "Initial demo content: landing page, blog articles, forum topics, and navigation"
  },
  "taxonomy": {
    "vocabularies": [
      {
        "slug": "blog_categories",
        "name": "Blog Categories",
        "description": "Categories for blog posts",
        "terms": [
          { "slug": "news", "name": "News & Announcements" },
          { "slug": "guides", "name": "Guides & Tutorials" }
        ]
      }
    ]
  },
  "pages": [
    {
      "slug": "home",
      "title": "Welcome to RusToK",
      "builder_type": "fly",
      "meta_description": "Modern E-Commerce and Content Platform",
      "content": {
        "html": "<section class=\"hero\"><h1>Welcome to RusToK</h1></section>",
        "css": ".hero { padding: 4rem 2rem; text-align: center; }",
        "components": [],
        "styles": []
      }
    }
  ],
  "blog": {
    "categories": [
      { "slug": "general", "name": "General", "description": "Platform updates" }
    ],
    "posts": [
      {
        "slug": "welcome-to-rustok",
        "title": "Introducing RusToK E-Commerce",
        "excerpt": "A deep dive into our modular architecture.",
        "content_format": "markdown",
        "content": "# Welcome to RusToK\n\nRusToK is a next-generation modular platform...",
        "tags": ["release", "architecture"],
        "category_slug": "general"
      }
    ]
  },
  "forum": {
    "categories": [
      { "slug": "general-discussion", "title": "General Discussion", "description": "Community conversations" }
    ],
    "topics": [
      {
        "category_slug": "general-discussion",
        "title": "Introduce Yourself!",
        "content": "Welcome to the RusToK community! Say hello here."
      }
    ]
  },
  "navigation": {
    "menus": [
      {
        "slug": "header_main",
        "title": "Main Navigation",
        "location": "header",
        "items": [
          { "title": "Home", "url": "/", "sort_order": 1 },
          { "title": "Blog", "url": "/blog", "sort_order": 2 },
          { "title": "Community", "url": "/forum", "sort_order": 3 }
        ]
      }
    ]
  }
}
```

---

## 6. Idempotency Guarantees

Every driver in `rustok-starter` follows an **upsert / skip-if-exists** pattern:
- **Pages**: Queries for existing page by `(tenant_id, slug)`. If present, skips creation and ensures published status.
- **Blog Posts**: Checks for post by `(tenant_id, slug)`. If present, skips re-insertion.
- **Forum Topics**: Checks for existing topic by `(category_id, title)`.
- **Navigation**: Finds menu by `(tenant_id, slug)`. If existing, updates items and binding without creating duplicate menus.

Running the import repeatedly is 100% safe and will not cause unique constraint errors or duplicate records.

---

## 7. Instructions for AI Agents: Working with Demo Data

When implementing features, fixing bugs, or writing tests that touch demo data or starter blueprints, agents **MUST** follow these rules:

### 7.1 When Modifying or Adding Demo Content
1. **Edit the embedded blueprint**:
   - Location: [`crates/utils/rustok-starter/src/embedded/default_starter.json`](file:///d:/RusTok/crates/utils/rustok-starter/src/embedded/default_starter.json).
   - Ensure the JSON is valid and matches the `StarterBlueprint` struct in [`crates/utils/rustok-starter/src/schema.rs`](file:///d:/RusTok/crates/utils/rustok-starter/src/schema.rs).
2. **Never bypass domain modules**:
   - If a new content type (e.g. products, reviews, FAQ) is added to the blueprint, create a dedicated driver under `crates/utils/rustok-starter/src/drivers/<entity>.rs`.
   - Call the module's public application service, never execute raw SQL against private tables.
3. **Always preserve reviewed publication invariants**:
   - When importing content that storefronts need to show, ensure the driver advances it to `Published` state via the official publication runtime.
4. **Channel auto-binding**:
   - Any navigation menu intended for storefront header/footer must be explicitly bound to the default channel slot via `MenuBindingService`.

### 7.2 Verification Protocol for Agents
Before concluding any task involving starter blueprints or demo data:
1. Run static checks on `rustok-starter`:
   ```bash
   cargo check -p rustok-starter --tests
   cargo clippy -p rustok-starter -- -D warnings
   ```
2. Verify architecture boundary compliance:
   ```bash
   python scripts/architecture_dependency_guard.py
   ```
3. Run the dedicated integration test:
   ```bash
   cargo test -p rustok-starter --test starter_import_integration
   ```
   *(Note: Remember rule 2.1 in `AGENTS.md` — maintainer runs full platform suites, but targeted integration tests for crate verification can be run when debugging or validating new drivers).*

---

## 8. Troubleshooting & Common Pitfalls

| Issue / Symptom | Root Cause | Canonical Resolution |
|---|---|---|
| **Landing page 404 on Storefront `/`** | Page was created in `Draft` status without publication. | Use `service.publish_reviewed` with `PageBuilderReviewedPublishRuntime` scenario `starter-blueprint-landing`. |
| **Header navigation empty on Storefront** | Menu was created in database but not bound to channel location slot. | Call `MenuBindingService::bind(tenant_id, channel_id, location, menu_id)`. |
| **`BlogError::AuthorRequired` during CLI import** | CLI run has `security.user_id = None`. | Synthesize deterministic fallback author UUID (`Uuid::from_u128(0x01)`) in driver. |
| **Foreign key error on category** | Topic or post created before category. | Respect topological order: categories and taxonomy terms are always created first. |
