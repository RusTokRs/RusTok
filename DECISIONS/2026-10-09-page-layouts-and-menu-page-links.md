# Page layouts and owner-resolved menu page links

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-pages` (templates, published routes and page visibility), `rustok-navigation` (menu references and trees), host adapters (composition)
- Extends: [Site symbols](./2026-10-09-site-symbols-shared-definitions.md), [Multilingual content contract](./2026-03-28-multilingual-content-contract.md)
- Supersedes: None
- Superseded by: None

## Context

The site-structure wave in the Pages audit calls for section-based page layouts and menu links that follow a page when its slug changes. Today `pages.template` is a free-form label, not a layout. Navigation stores only `menu_items.url`; neither Navigation nor its GraphQL/HTTP/storefront readers know the identity of a Page. Pages already owns published-route resolution and slug redirects. A persisted URL copy in Navigation would be a second route authority, and a Navigation dependency on Pages would invert the documented module boundary. Site symbols provide reusable section definitions but not the ordered layout (header/body/footer) or its lifecycle.

## Decision

1. **Layouts.** Pages owns versioned, localized `page_templates` keyed by `(tenant_id, template_key, locale)`, containing an ordered header and footer list of site-symbol IDs. The page's existing `template` string selects a key; `default` is the explicit empty layout for legacy pages. A non-default key must exist for **every** published locale. Layout slots refer to Pages-owned site symbols, not copied section JSON. Editor preview merges the selected layout and document body without writing the layout nodes into the body. At publish, resolve the template and definitions in the Pages transaction, insert header and footer references around the one Fly page's content, expand symbol references, then run the existing validation, sanitization, immutable artifact and integrity pipeline. The exact template and symbol revision snapshot used for review must match the publish input; drift requires reload, save and review, never a silent build of unseen content. Published artifacts remain immutable until a separately reviewed re-issue (including after layout changes).
2. **Menu targets.** Navigation owns an optional `page_id` on a menu item, mutually exclusive with a static URL. The old non-null `url` column uses `''` only for page-target rows; it is not a fallback or rendered link. Legacy URL rows remain unchanged. Input/DTOs distinguish `Page { page_id }` from `Url { url }`; do not infer identity by parsing URL or slug. Reject both-or-neither on writes and require a Pages-owned published, tenant-local, channel-visible target for the menu's read context. Navigation stores no page slug, no derived URL and no foreign key into Pages tables.
3. **Resolution port.** Navigation declares a narrow `PageMenuRouteReader` read port taking `(tenant_id, page_id, exact_locale, channel_id)` and yielding `Option<canonical_path>`. A host adapter implements it with the Pages **published** `PageRouteService::canonical_descriptor`, checking page/channel visibility with Pages APIs first; it does not reconstruct `/{locale}/modules/pages?slug=...` in Navigation. Host GraphQL schema data supplies this adapter to both `menu` and `activeMenu`; Navigation's Axum router receives the same adapter through its host state; the Navigation native storefront server function obtains it from host runtime context. The GraphQL storefront adapter reads the resolved Navigation URL from the GraphQL menu result, rather than supplying a second route algorithm. All three readers use the same Navigation projection and fail closed when an adapter is missing for a page-target row. Static-URL-only menus require no Pages adapter. If the page is unpublished, deleted, disabled for that channel or lacks an exact published locale, omit the link/item from **public** menus (and its children unless individually presentable); never serve the stored empty URL, a stale route or a draft URL. An administrative read may expose the target ID and unresolved state, but must not call it a public URL.
4. **Slug changes.** Pages remains the sole route authority. Menu reads resolve `page_id` afresh against the current published descriptor, so a published slug change changes the public link without rewriting Navigation data or relying on an alias redirect. Pages route-generation changes invalidate menu/storefront cache identity; the storefront's navigation snapshot remains an input to its composition ETag. A menu cache may only cache a result together with the corresponding Pages route/visibility generation (or bypass caching for page-target menus).

## Sources of truth and ownership

Pages owns template rows, symbol rows, page metadata/body, publication, route and channel visibility. Navigation owns menu rows, item target discriminants, localization and channel bindings. Host adapters compose public read models; neither owner reads or mutates the other's tables. Storefront and search are projections, never layout or route authorities. Fly owns component AST and safe expansion; the pinned Page Builder capability request/response envelope does not change.

## Invariants

### Allowed states

- Existing static URL menu items and `default` template pages work without a migration rewrite.
- A page item stores exactly a tenant-local page identity, and a public path is materialized only during an authorized exact-locale, channel-scoped read.
- Reviewed layout/symbol snapshots can be promoted into immutable per-locale artifacts.

### Forbidden states

- A menu item storing both a page target and a usable URL, a cross-tenant target, an unpublished/draft link in a public menu, or a URL derived from a guessed slug.
- A template definition silently modifying an already published artifact; layout content bypassing Pages publish sanitization, symbol cycle checks or review checks.

## Non-goals

Automatic re-issue of dependent published pages on symbol/template edits is a separately tracked part of the site-symbol decision. Menu page references do not replace Pages redirects for old inbound URLs. External URLs remain Navigation-owned. No generic site-routing registry or new Page Builder FFA request variants are introduced.

## Data, transaction, and concurrency boundary

Use additive migrations for `page_templates` and nullable `menu_items.page_id`, with a database check requiring exactly one of nonblank URL or page ID. Backfill: existing rows have `page_id = NULL`, preserve their URL bytes; existing `pages.template = 'default'` uses the virtual empty layout, while other pre-existing labels remain legacy labels until explicitly migrated or a matching template is created. Introduce non-default publish enforcement only after catalog population and an operator audit of existing labels; do not silently break old publications. Template writes use a tenant-scoped revision CAS; deletion must reject live dependents or require explicit reassignment. Menu creation validates target identity with the route port before committing; read-time validation is still required because publication and channel visibility can change independently. Neither menu creation nor template writes may mutate already published artifacts. Retries of reviewed publish retain the existing idempotency key and saved snapshot semantics.

## Context dimensions

Tenant and exact locale are mandatory for both layout resolution and page links. Page-link public resolution requires the current channel and checks Pages module/page visibility; absence of a channel is not permission to expose an otherwise hidden page. Navigation and Pages retain their own auth/RBAC on writes and read visibility. Trace context propagates through the host adapter. Timezone and currency are not applicable.

## Events and projections

Pages owns route, publication and template/symbol change evidence, and notifies cache dependents on new published route generations. Navigation does not consume a page event to rewrite item URLs. A resolved-menu cache must track those generations or stay uncached; rebuilding it does not change canonical rows. Search projection is independent of this menu contract.

## Failure semantics

Template or symbol missing/stale at publish: reject with actionable review error. Reader unavailable or route/visibility lookup failing: fail closed, not a stale URL. Unpublished or wrong-locale page: public item omitted. Tenant mismatch is never treated as a fallback. A Pages module disabled for the channel yields no public page link.

## Migration and cutover

Stage the additive columns/tables and dual static/page input first, then wire **all** GraphQL, HTTP and native storefront read paths to the same adapter before accepting page-target writes. Audit and explicitly migrate pre-existing non-default free-form template labels before enabling strict publish enforcement. Deployment must not expose stored empty URLs during a rolling upgrade; gate writes until all readers use the target discriminator. No automatic conversion of existing hard-coded page URLs; editors can opt into references. Roll back by disabling new page-target writes and retaining the additive columns, not by fabricating static URLs.

## Alternatives considered

- Copy Pages slug into Navigation and update it on slug-change events: rejected as second route truth, with lag/races on unpublish.
- Make Navigation depend on Pages: rejected because Navigation is the menu owner and Pages must remain an optional module.
- Treat `pages.template` as a presentation label and copy sections into every body: rejected because edits cannot be reviewed or consistently re-issued from one layout owner.
- Resolve by URL alias: rejected because alias history is inbound compatibility, not an identity reference.

## Verification

Test additive/backfill migration on populated menu and pages schemas; uniqueness/CAS and cross-tenant template writes; exact-locale/channel unpublished/deleted and missing-adapter behavior in GraphQL, HTTP and native storefront; published slug change without Navigation row mutation; cache identity after route change; reviewed layout/symbol drift, one-page publish and immutable artifact integrity; legacy `default` and static-link parity. Run scoped Rust tests and repository migration/ADR gates. The additive Pages-owned template catalog, CAS authoring service and GraphQL authoring surface are implemented. Publish/preview composition, existing-label cutover, navigation page targets and host route adapters are **not** implemented; implementation status remains **In progress** until the full verification matrix is satisfied.

## Consequences

This introduces a host composition port and a review-aware layout lifecycle, not a new FFA capability. Site-symbol re-issue and usage UI remain outstanding; writing this decision alone does not close the wave-3 audit gaps.
