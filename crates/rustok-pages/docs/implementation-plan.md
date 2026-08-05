# Implementation Plan for `rustok-pages`

Date: 2026-08-06  
Status: `in_progress / host-route-response-source-ready / execution-pending`

## Policy: current code only

Pages is under active development. It keeps **no legacy** compatibility editor,
component mirror, block table, shadow document authority or migration shim.

Forbidden:

- a JSON/CRUD editor beside Fly;
- the deleted Next/GrapesJS page-builder route;
- `frames[0].component` as a component-tree mirror;
- `PageBlock`, `BlockService`, `page_blocks` or block mutations;
- storefront block fallback rendering;
- UI access to raw transport adapters;
- host-owned Pages persistence, route-claim policy, cache-key policy or document policy.

The visual document authority is `pages[].component` stored in the Pages body.

## Mission and ownership

`rustok-pages` owns page identity, localized metadata and bodies, localized slugs,
immutable public route history, channels, draft/published lifecycle, immutable
landing artifacts, publish/rollback receipts, route/page/artifact cache namespaces,
public reads, redirects, deletion policy and audit.

Fly/Page Builder owns visual primitives, review, sanitizer, runtime materialization,
renderer and artifact-producer contracts. It does not own Pages persistence, public
route identity, cache scope or tenant policy.

Navigation owns menu identity and public menu composition. Hosts own request
routing, locale/auth/tenant context and HTTP response composition, but not Pages
route ownership.

Optional external event infrastructure is outside the active Pages cursor.

## Current implementation

### Domain and persistence

- [x] Independent entities exist for pages, translations, bodies, channel
  visibility, scenario baselines, immutable artifacts, publish/rollback receipts
  and exact publish manifests.
- [x] `PageBlock`, block DTOs/services/transports and storefront block fallback are
  deleted.
- [x] Current documents use only `pages[].component`; unknown current Fly fields are
  preserved.
- [x] Metadata and document writes use independent optimistic versions/revisions.
- [x] Reviewed publication persists authoritative sanitization, runtime
  materialization evidence, immutable artifacts, bindings, events, receipt and
  manifest in one transaction.
- [x] Rollback restores a verified prior immutable manifest without compiling the
  current draft.
- [x] `PageService::create` always creates a draft; Page Builder publication must
  cross the reviewed command.
- [x] Pages owns bounded route/page/artifact cache scopes and generation-aware keys.
- [x] `page_route_aliases` is an append-only localized public route ledger with a
  unique `(tenant_id, locale, slug)` claim and no foreign key to the current page.

### Admin FFA

- [x] Pages owns the Page Builder consumer facade and transport selection.
- [x] Fly saves reload current metadata and reject stale body revisions.
- [x] Admin publication gathers all body revisions, explicit promoted scenario and
  reviewed runtime, then consumes the durable receipt.
- [x] Rollback uses an independent deterministic idempotency namespace.
- [x] The registered `rustok.pages.metadata` contribution is reused in draft and
  published-only surfaces.
- [x] The bespoke `PageMetadataEditor` and direct workspace metadata transport are
  removed.
- [x] A published metadata slug rename appends immutable redirects before replacing
  translations in the same owner transaction.
- [x] A draft-only slug rename does not create public route history.

### Storefront FFA

- [x] Published Fly/GrapesJS documents render through Page Builder storefront.
- [x] Bound artifacts are integrity-checked before public HTML is returned.
- [x] The composite storefront response binds route, page and artifact generations,
  slug, requested/fallback locale and channel.
- [x] Channel/module admission precedes cache lookup; cache failures fail open only
  to validated owner reads.
- [x] The production generation gate runs the Pages invalidation handler before
  downstream acceptance with process-bounded event UUID dedupe.
- [x] A production gate PostgreSQL publish/rollback restart harness retains durable receipts/events and a post-invalidation downstream failure; process-bounded dedupe prevents a second rotation when a new relay instance retries the same event UUID.
- [x] A factory-selected Memory and OutboxLocal profile harness retains the real
  topology. Memory rotates synchronously without a durable row; OutboxLocal writes
  a pending row first and rotates inside the relay target before acknowledgement.
- [x] A selected immutable published artifact regression retains exact/fallback
  reads across a persisted draft body mutation. The current body content is not
  public render authority.
- [x] Native and unauthenticated GraphQL public detail and list reads use the same
  tenant fallback chain: requested locale, tenant default locale, then platform
  fallback.
- [x] An anonymous storefront dependency graph verifier covers Pages and host
  default/hydrate/SSR profiles while excluding admin and Fly authoring packages.
- [x] The current public Pages host is SSR-only and has no executable client
  bootstrap source.
- [x] Localized canonical Pages routes and hreflang alternates use
  `/{locale}/modules/pages?slug={slug}`.
- [x] Pages resolves current routes and immutable aliases as canonical, redirect or
  gone and fails closed on current/history ownership overlap.
- [x] Old published slug claims cannot be reused by another page.
- [x] A registered Pages host route decision server function performs channel-module
  admission before the Pages route owner and rechecks target publication/channel
  visibility.
- [x] Exact localized canonical routes continue SSR; legacy, noncanonical and alias
  routes return `308`; gone returns `410`; missing returns `404`; ambiguous ownership
  returns `409`; operational route failures return `503`.
- [x] Terminal Pages host route responses use `private, no-store`, and canonical
  redirect locations percent-encode the slug query value.
- [ ] Delete tombstones and historical backfill remain open.
- [ ] Authenticated real-DOM inline editing is not implemented.
- [ ] Compiled SSR/CSR/hydrate bundle artifact evidence remains open; client bundle
  proof becomes mandatory when a Pages client bootstrap is introduced.

### Page Builder/FBA

- [x] Capability registry, permissions, typed errors and fallback profiles exist.
- [x] Deterministic rendering and SHA-256 artifact identity exist.
- [x] Pages persists immutable landing artifacts, bindings and materialization
  identity/snapshots without raw runtime context.
- [x] Page Builder exposes authoritative static sanitization and reviewed runtime
  contracts.
- [x] Publish replay is idempotent; key reuse with different input fails closed.
- [x] Rollback verifies exact manifests and remains independent of current provider
  health.
- [x] GraphQL, HTTP and admin transports use typed publish/rollback receipts.
- [x] Non-builder publication rejects every Fly/GrapesJS body.
- [x] Page Builder ownership is unchanged by Pages route aliases and host responses;
  route identity is derived from Pages translations and publication state only.
- [ ] Accepted execution evidence must correlate receipts, events, generation
  changes, cache misses/refills and public route behavior.
- [ ] Observed Wave 0/Wave 1 tenant evidence remains open.

## Core invariants

1. `pages[].component` is the sole visual component-tree authority.
2. Metadata and document writes never overwrite one another implicitly.
3. Draft saves do not mutate the selected immutable artifact; current body content
   is not public render authority.
4. Publish rejects stale metadata or any stale localized body revision.
5. Publish state, bindings, exact manifest, outbox events and receipt commit or roll
   back together.
6. Rollback state, replacement bindings, events and receipt commit or roll back
   together.
7. Create never publishes; Page Builder publication always crosses reviewed
   sanitization and runtime materialization.
8. Channel/module authorization runs before every cache lookup and before public
   Pages route resolution.
9. Cache fill follows validated owner reads; cache failures do not authorize data.
10. Current and historical route claims are unique by tenant, locale and slug.
11. Immutable redirects store target page and locale, not target slug; resolution
    recomputes the current canonical descriptor and avoids redirect chains.
12. Draft-only slugs are not permanent public claims before publication.
13. Ambiguous current/alias ownership or alias payload drift fails closed with
    `PAGE_ROUTE_RESOLUTION_CONFLICT`; missing routes use `PAGE_ROUTE_NOT_FOUND`.
14. Localized canonical output is `/{locale}/modules/pages?slug={slug}`; the legacy
    unprefixed module route remains parseable but is not emitted as canonical.
15. Only an exact localized canonical Pages decision reaches SEO and SSR rendering;
    every terminal route decision is `private, no-store`.
16. Feature-resolved anonymous storefront graphs exclude admin and Fly authoring
    packages through non-dev dependencies.
17. No block or shadow-editor fallback exists.

## Current pipelines

```text
reviewed publish
  → exact page/body locks
  → feature + promoted-scenario gates
  → authoritative sanitizer
  → runtime materialization
  → deterministic renderer
  → immutable artifacts + bindings
  → published page + transactional events
  → durable receipt + exact manifest
  → production generation gate
  → registered generation-aware public miss/refill
```

```text
published metadata slug rename
  → page lock + version check + publish permission
  → current translation claim check
  → immutable alias claim check
  → append redirect for old localized slug
  → replace current translation
  → advance metadata version
  → transactional NodeUpdated
  → route generation rotation through existing event path
```

```text
public Pages host request
  → trusted tenant/request context
  → Pages channel-module admission
  → requested → tenant default → platform locale candidates
  → PageRouteService::resolve
  → target publication/channel visibility recheck
  → exact canonical: SEO + SSR render
  → alias/noncanonical: 308
  → gone: 410
  → missing: 404
  → conflict: 409
  → operational failure: 503
```

## FFA/FBA status

- **FFA:** `in_progress` — metadata/document separation, reviewed publication,
  rollback, immutable public reads, tenant locale fallback, localized route identity,
  published-slug redirects and explicit host route responses are source-connected.
  Browser execution, inline edit and built artifact evidence remain open.
- **FBA:** `in_progress` — sanitizer/materialization/artifact/receipt boundaries and
  production-gated cache invalidation are source-connected. Route persistence and
  response decisions belong to Pages and do not change Page Builder. Execution and
  rollout remain open.
- **Structural shape:** `core_transport_ui` with one current document authority.

## Completed source slices

### 2026-07-21

- Deleted block storage, services, transports and compatibility editor paths.
- Added immutable artifacts, reviewed runtime, authoritative sanitizer and atomic
  reviewed publication.
- Removed create-time/default-runtime Page Builder publication.

### 2026-07-22

- Added bounded route/page/artifact cache generations and event-driven
  invalidation.
- Added exact immutable publish manifests and rollback receipts.
- Connected GraphQL, HTTP and admin rollback surfaces.

### 2026-08-03

- Registered canonical metadata contributions for draft and published-only hosts.
- Added stale metadata and dirty-Fly isolation source regressions.
- Removed the bespoke metadata editor.

### 2026-08-05

- Retained native registered route, channel admission and immutable artifact source
  packets.
- Added production gate, PostgreSQL restart and factory-selected Memory/OutboxLocal
  source harnesses. OutboxLocal writes a pending row first.
- Added selected immutable artifact, anonymous dependency graph and SSR-only host
  source packets.
- Added public detail/list tenant fallback parity.
- Added immutable redirects for published slug renames, route-claim reservation,
  transport-neutral canonical/redirect/gone resolution and localized SEO canonical
  routes.

### 2026-08-06

- Added the registered Pages host route decision server function.
- Composed route decisions before SEO and SSR rendering.
- Added source responses for exact canonical, permanent redirect, gone, missing,
  conflict and operational failure states.
- Added percent-encoded canonical locations and private no-store terminal policy.
- Added a registered SQLite/Axum source harness and fail-closed source contract.
- Tests, verifiers, formatters, Cargo commands, databases, server functions, hosts,
  browsers, workflows and CI were not executed by the implementation agent.

## Next implementation order

### P0 — execution evidence

- [ ] Run the host route response verifier and registered SQLite/Axum harness.
- [ ] Run metadata conflict and dirty-Fly isolation packets.
- [ ] Run selected immutable artifact, native route/cache/admission and generation
  gate packets.
- [ ] Run the production gate PostgreSQL restart and Memory/OutboxLocal profile
  packets.
- [ ] Build and inspect the anonymous SSR artifact.
- [ ] Run the published slug alias SQLite/PostgreSQL owner evidence.

### P1 — storefront and routing

- [x] Serve only the selected immutable published artifact.
- [x] Keep public detail and list reads on the same tenant fallback chain.
- [x] Add localized canonical URLs and immutable redirects for published slug
  renames.
- [x] Reserve historical published route claims and fail closed on collisions.
- [x] Mount `PageRouteService::resolve` in the public host after channel/module
  admission and return canonical, redirect or gone responses.
- [ ] Add deletion tombstones while preserving existing redirect history.
- [ ] Define historical route backfill/import policy.
- [ ] Compose Navigation-owned menus, SEO and channel visibility with deterministic
  generation-aware cache keys.
- [ ] Implement authenticated real-DOM inline editing.
- [ ] Prove admin preview, published output and inline edit parity.

### P1 — Page Builder authoring

- [ ] Add Media contributions without transferring Media ownership.
- [ ] Integrate rich text only through the opaque payload/editor seam.
- [ ] Generate contribution registries from module metadata.
- [ ] Filter contributions by tenant, permission, capability, provider health and
  surface.
- [ ] Complete accessibility, keyboard and degraded-state coverage.

### P2 — operations and rollout

- [ ] Correlate metadata save, document save, publish, rollback, route alias,
  host response, invalidation and public read in telemetry.
- [ ] Add artifact/manifest integrity audit and repair/rebuild commands.
- [ ] Audit delete/unpublish/rollback and route tombstone behavior.
- [ ] Run observed internal-tenant Wave 0, then Wave 1 after all gates pass.

## Verification

Suggested commands; execution is intentionally maintainer-owned:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-host-route-response.mjs
cargo test -p rustok-pages-storefront --features ssr \
  --test host_route_decision_sqlite -- --nocapture
cargo test -p rustok-storefront --features ssr --lib -- --nocapture
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-storefront --features ssr --all-targets

node crates/rustok-pages/scripts/verify/verify-pages-published-slug-route-alias.mjs
cargo test -p rustok-pages --test page_published_slug_route_alias_sqlite -- --nocapture

node crates/rustok-pages/scripts/verify/verify-pages-public-list-locale-fallback.mjs
cargo test -p rustok-pages --test page_locale_fallback -- --nocapture

node crates/rustok-pages/scripts/verify/verify-pages-selected-immutable-artifact.mjs
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-gate-postgres-restart.mjs
node crates/rustok-pages/scripts/verify/verify-pages-event-delivery-profile-parity.mjs
node scripts/verify/verify-pages-current-only.mjs
node scripts/verify/verify-pages-ui-boundary.mjs
```

Compiled SSR/CSR/hydrate bundle artifact evidence remains open.

## Update rules

- Update this plan in every Pages implementation slice.
- Checkboxes require merged source; execution gates require reproducible evidence.
- Contract changes require matching source guards and tests.
- New dependencies require dependency records.
- Never reintroduce block storage, shadow editors, frame mirrors or duplicate
  document authorities.
