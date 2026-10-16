# Page and Template Builder Unification: Unified Canvas, Layout Slots, and Preset Architecture

- Date: 2026-10-16
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-page-builder` (canvas capabilities and authoring workspace), `fly` (AST model, slot validation, and deterministic composition), `rustok-pages` (persistence, lifecycle, publish closure, and starter catalog)
- Extends: [Page layouts and owner-resolved menu page links](./2026-10-09-page-layouts-and-menu-page-links.md), [Site symbols: shared component definitions with instance re-issue](./2026-10-09-site-symbols-shared-definitions.md), [Fly page-builder architecture](./2026-07-13-fly-page-builder-architecture.md)
- Supersedes: None
- Superseded by: None

## Context

RusToK previously split page authoring, site symbols, and page layouts across sequential architectural steps:
1. `fly` and `rustok-page-builder` established the framework-neutral visual document engine, component AST, design tokens, and browser canvas protocol.
2. `DECISIONS/2026-10-09-site-symbols-shared-definitions.md` established cross-page reusable component definitions (`site_symbols`) re-issued on publish.
3. `DECISIONS/2026-10-09-page-layouts-and-menu-page-links.md` introduced the `page_templates` table (`tenant_id, locale, template_key`) linking ordered header and footer site-symbol IDs.

Despite this foundation, four critical functional and architectural gaps remained:
- **Headless layouts:** Layouts existed solely as relational rows of symbol IDs without a visual editor. Administrators had no visual canvas to design or adjust layouts.
- **Disconnected publish pipeline:** `PageService::publish_reviewed` did not invoke `compose_layout_document`. A page with `pages.template = "sidebar"` was compiled into an artifact containing only the page body, silently omitting the configured layout during publication.
- **Blind page authoring:** When editing a page, the canvas showed the page body in isolation. Editors had no WYSIWYG context to observe how the content aligns within the header and footer of the selected layout.
- **Lack of page templates / starters:** When creating a new page, authors were presented only with an empty canvas. There was no catalog of pre-built page templates (Hero + Grid + CTA + Testimonials + FAQ) to kickstart creation.

Creating an independent, disconnected "Templates" subsystem would duplicate component registries, design tokens, style managers, and rendering pipelines.

## Decision

Integrate page layouts, site symbols, starter templates, and visual pages into a single **Page and Template Builder** powered by the canonical `fly` engine and `rustok-page-builder`.

### 1. Unified Document Kinds (`DocumentKind`)

In `fly` and `rustok-page-builder`, every authored document explicitly declares its intent via `project.extensions["flyDocumentKind"]`:
- `Page`: An independent route document owning its unique content body.
- `LayoutTemplate`: A site-wide page frame containing headers, footers, sidebars, and exactly one `<PageContentSlot />`.
- `SectionSymbol`: A standalone reusable section (e.g. promo bar, subscription form, global navigation).
- `StarterPreset`: A pre-built page blueprint cloned deeply into a new page draft at creation.

### 2. Built-in `<PageContentSlot />` Component

Introduce a canonical AST primitive `page_content_slot` in `fly::builtin_blocks`:
- **Visual presentation:** Renders in the editor as a distinct outline boundary with a localized placeholder: *"Page Content Slot"*.
- **Structural invariant:** A `LayoutTemplate` MUST contain exactly one `page_content_slot`. A `Page` or `SectionSymbol` MUST NOT contain a `page_content_slot`.
- **Node protection:** Inside the Layout editor, the slot component cannot be deleted or duplicated. It does not accept nested children in its persisted state.

### 3. In-Context Visual Authoring (WYSIWYG Page Editing)

When an editor opens a page that specifies a non-default `template_key`:
- The Page Builder host fetches the composed projection via `previewPageTemplateDocument` (or the contextual preview port).
- The canvas mounts the composite document:
  - Header and footer sections from the layout are rendered in-place, visually enclosed with subtle boundary indicators and marked as **read-only / locked**.
  - A contextual toolbar button allows authorized operators to *"Edit Layout"* directly.
  - The page body inside the slot remains fully interactive (drag & drop, style editing, text editing).
- **Save boundary:** When the operator saves the page, `rustok-page-builder` serializes and saves **strictly the page body components**. The layout nodes and symbol wrappers are never written into `page_bodies`.

### 4. Reviewed Publish Closure

Close the open implementation gap in `PageService::publish_reviewed`:
1. During the reviewed publish transaction, load the page's effective `template_key` for the target locale.
2. If `template_key != "default"`, load the exact-locale `page_templates` row and dependent `site_symbols`.
3. Wrap the reviewed page body using `compose_layout_document` into a single unified Fly page.
4. Expand symbol references (`fly::resolve_symbol_instances`).
5. Execute `sanitize_static_landing_project` and `compile_materialized_static_landing`.
6. Enforce that the revision token of the layout and symbols matches the review snapshot (optimistic CAS guard against unreviewed drift).
7. Store the final compiled HTML and scoped CSS in `page_static_landing_artifacts`.

### 5. Starter Templates Catalog (Presets)

1. `rustok-pages` owns a catalog of starter presets (`page_starters`), categorized into `landing`, `ecommerce`, `about`, `contact`, `blog`.
2. The New Page dialog presents a template picker:
   - *Start Blank*
   - *Choose from Template Library* (visual preview cards with pre-assigned layouts).
3. Selecting a starter template performs a deep AST clone with identifier remapping (`UniqueIdGenerator`) to ensure zero shared-reference coupling with the starter definition.
4. Operators with `pages:manage` permissions can save any existing page as an organization-local starter template.

## Sources of truth and ownership

- **`fly`**: Owns `DocumentKind`, AST validation rules, `page_content_slot` component definition, symbol expansion, and pure document composition.
- **`rustok-page-builder`**: Owns neutral capability ports (`preview`, `tree`, `properties`, `publish`), canvas iframe protocol, and authoring workspace state.
- **`rustok-pages`**: Owns `pages`, `page_templates`, `site_symbols`, `page_starters`, routes, review hashes, revision CAS, and immutable published landing artifacts.
- **`apps/next-frontend` & Rust storefront**: Render compiled static landing artifacts directly via SSR without runtime layout resolution overhead.

## Invariants

### Allowed states
- A `LayoutTemplate` containing header symbols, footer symbols, and exactly one `page_content_slot`.
- A `Page` referencing a valid `template_key` or `default`.
- A page document containing zero layout nodes in persistence, but rendering inside layout nodes in canvas preview.
- An immutable published artifact containing fully pre-compiled and sanitized layout + page content.

### Forbidden states
- A `LayoutTemplate` with zero or more than one `page_content_slot` (fails validation).
- A `Page` persisting `page_content_slot` nodes (fails validation).
- Writing layout nodes or site symbol source trees into `page_bodies` during page save.
- Publishing an artifact with unreviewed layout or symbol revision drift (fails closed with review error).
- Client-side runtime assembly of layouts on the public storefront (forbidden; all composition happens at publish time).

## Non-goals

- Dynamic headless theme templates for external headless frameworks (Next.js components remain React Server Components; Fly templates govern Fly-managed document routes).
- Runtime client-side template swapping without page re-publication.

## Data, transaction, and concurrency boundary

- **Transaction boundary:** Page publish, template save, and symbol save run in tenant-scoped database transactions.
- **Revision CAS:** `page_templates` and `site_symbols` use monotonically increasing `revision: i64`. Publishing a page locks the page row, checks expected body revisions, and verifies the layout snapshot revision.
- **Destructive operations:** Deleting a `page_template` fails closed if active published pages reference it, unless an explicit re-assignment key is provided.

## Context dimensions

- **Tenant:** Strictly isolated by `tenant_id` across all tables (`page_templates`, `site_symbols`, `page_starters`, `pages`).
- **Locale:** Layouts and symbols are exact-locale keyed (`(tenant_id, locale, template_key)`). No implicit fallback to default locale during publication.
- **RBAC:**
  - View templates / starters: `pages:read`.
  - Edit pages: `pages:update`.
  - Author / edit templates and symbols: `pages:manage`.
  - Publish pages: `pages:publish`.

## Events and projections

- `PageTemplateSaved` / `PageTemplateDeleted` emitted through the transactional outbox on template mutations.
- Cache invalidation: Template mutations trigger invalidation events for dependent page ISR tags (`pages`, `page-${slug}`) in Next.js storefront.

## Failure semantics

- Missing template at publish time: Fails closed with actionable error (`Page template '{key}' not found for locale '{locale}'`).
- Symbol recursion / cycles in layout: Fails closed at validation time before any database write.
- Revision drift during publish: Fails closed, prompting the editor to reload, review the updated layout, and re-publish.

## Verification

- Scoped backend test suite: `cargo test -p rustok-pages --test page_template_catalog_sqlite` and `cargo test -p rustok-pages --test reviewed_publish_layout`.
- Fly core test suite: `cargo test -p fly` verifying `page_content_slot` registration and single-slot validation.
- End-to-end publish verification: Verifying published artifact HTML contains layout header, page content, and layout footer in correct order with verified hash integrity.
