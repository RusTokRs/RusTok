# Published pages are transported as their landing; the page body is editor state

- Date: 2026-11-10
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-pages
- Extends: [Page body working copies: draft revisions and body revision journal](./2026-10-09-page-body-draft-and-revision-history.md)
- Supersedes: None
- Superseded by: None

## Context

A page has two representations. The **body** (`page_bodies`, `page_body_drafts`) is the Page
Builder authoring document. The **published landing** (`page_static_landing_artifacts`, bound by
`page_published_landing_artifacts`) is the sanitized markup and stylesheet that the reviewed
publish compiled from the body.

Public reads returned the body. `pageBySlug.body` gave anonymous callers the authoring document,
which the publish sanitizer has not processed. Storefronts had to find the landing by another
route: the Leptos storefront wrapped it into a synthetic body format
(`fly_artifact_payload`, `fly_artifact_url`) and kept a render branch for the raw authoring
document; the Next storefront at one point rendered the authoring document itself, and later
fetched `GET /api/pages/{id}/artifact` and split the HTML document by the renderer's exact
envelope. Each consumer decided from the content which representation it held.

## Decision

1. **The page body is authoring state.** Every page read returns `body` only to a principal
   that may save or publish that document: the rule is the write rule of those commands
   (`pages:update` or `pages:publish` over the page, `enforce_owned_scope` with the page
   author). All other readers, including public readers, customers and read-only staff, get
   `body: null`. The same rule covers the draft of a published page: a publish folds the draft
   into the current body, so a principal that may publish must be able to review it. This
   amends the 2026-10-09 decision, which exposed the draft to `pages:update` only. The
   revision journal of the document (`pageBodyRevisions`,
   `GET /api/admin/pages/{id}/document/history`) is readable under the same rule; restoring a
   revision is a save and requires `pages:update`. The rule
   applies to every transport (service, GraphQL, REST). The role label of the principal does
   not decide it on its own: no label grants the body without the permission or withholds it
   from a holder. The label takes part only inside the shared write rule, where
   `permission_scope_for_set` narrows `pages:update` of a non-staff label to the principal's
   own pages; the body follows that scope so that reading and saving never disagree.
   Every page read also says whether the reader may read the document
   (`PageResponse.document_readable`, GraphQL `documentReadable`), so that an absent `body`
   is not ambiguous: with `true` the page has no document in the effective locale, with
   `false` the document is withheld. A write command answers with the same read, by the same
   rule. An editor opens its starter document only in the first case.
2. **Published content is the typed field `landing`.** `GqlPage.landing` returns
   `{ locale, html, css, artifactHash }` from the artifact bound to the page: `html` is the body
   markup, `css` the unscoped stylesheet. It is resolved by
   `PageBuilderArtifactService::load_public_bound_artifact_with_fallback` for the effective
   locale of the page and the request channel, and is `null` when the page is not published,
   not visible on that channel, or has no bound landing.
3. **Storefronts render the landing and nothing else.** The Leptos storefront
   (`PageDetail.landing`, `PublishedLandingBody`) and the Next storefront
   (`fetchPublishedPage`, `PageBuilderContent`) carry the same shape. Neither has a type, a
   query selection or a render branch for the body. A published page without a landing has
   nothing to render, so its storefront route does not exist: the Next route answers 404 and
   the Leptos host route decision (`resolve_storefront_page_route`) answers `not_found`, for
   the canonical route and for its aliases. The Leptos page card is a module view, not a
   route; embedded elsewhere it shows an empty state for such a page.
4. **Public metadata derived from page content uses the landing too.** The SEO target of a
   page (`seo_targets.rs`) derives its fallback description and its primary image from the
   body markup of the published landing: the description is its visible text, the image its
   first `<img>` with a shareable source. Nothing in SEO reads the body. Without a landing the
   record has no content-derived description and no image.
   The public search document of a page (`search_projection.rs`) follows the same rule: its
   body text is the visible text of the landing that the landing binding of the page locale
   points at. The projection does not read the page body, and it does not pick an artifact
   from the artifact table, which also keeps superseded landings.
   A published page without a published landing has no storefront route (decision 3), so it
   is not advertised either: the search projection emits no document for a page without any
   bound landing, and the sitemap omits a page whose default-locale read resolves no landing.
5. **An unpublished page exists only for page authors.** The record of a page that is not
   published is readable by a principal that holds a page authoring permission (`pages:create`,
   `pages:update`, `pages:delete`, `pages:publish`; `pages:manage` implies them).
   `pages:read` and `pages:list` do not qualify, because public readers and customers hold
   both. On GraphQL the same test separates authors from visitors: a signed-in principal
   without an authoring permission is a visitor and gets what an anonymous reader of the
   request channel gets (published pages of that channel; `page(id)` of an unpublished page is
   `null`, not a permission error). REST `GET /api/pages` applies the same test: a token
   without an authoring permission gets `404 page_not_found` for a page that is not visible
   on the request channel or when the Pages module is not bound to that channel; a page
   author resolves the page on any channel. The role label is not consulted here either.
6. **The published landing is cached once, on the server, under one key rule.**
   `GqlPage.landing` and the document of `GET /api/pages/{id}/artifact` are read through the
   generation-aware Pages cache (`PagesCacheReadRuntime`, 60 s TTL). Both entries are keyed by
   `page_cache_key` in the artifact scope with the variant built by
   `published_landing_cache_variant`: the representation (`landing` or `document`), the
   route and page generations, the locale, the fallback locale and the normalized channel.
   Together with the artifact generation of the key, every page event rotates both entries.
   An entry carries no principal, and only a resolved landing is stored. Storefront
   applications keep no cache of their own: the Next storefront reads with `no-store` and has
   no revalidation endpoint, no revalidation secret and no Draft Mode.

## Sources of truth and ownership

- Canonical owner: `rustok-pages`.
- Authoritative state: the page body (editor state) and the bound landing artifact (published
  content).
- Derived representations: `GqlPublishedLanding`, the Leptos `PublishedLanding`, the Next
  `PublishedLanding`, and the cached native storefront snapshot (`StorefrontPagesData`).
- Dependency direction: storefront packages consume `rustok-pages`; `rustok-pages` does not
  know its storefronts.
- `GET /api/pages/{id}/artifact` is the other public representation of the same artifact: the
  complete standalone HTML document with its own Content-Security-Policy, `ETag`/`304` and
  `Cache-Control: public, max-age=60, stale-while-revalidate=300`. It serves consumers that
  need a document rather than markup to embed (an iframe, a CDN or an edge cache, a client
  outside this repository). The storefronts of this repository do not read it, by design:
  they embed `landing` in their own document. Its server cache entry follows the key rule of
  decision 6, so the server never answers from an entry older than the last page event, also
  for a conditional request. What a downstream cache holds stays valid for the `max-age` the
  response declares.

## Invariants

### Allowed states

- `body` present for a reader that may save or publish the document; `Draft` state only on
  by-id reads.
- `landing` present for a published page that is visible on the request channel and has a
  bound landing for the effective, the tenant default or the platform fallback locale.
- `landing: null` together with a resolved page (for example an author reading a page on a
  channel where it is not published, or reading an unpublished page).

### Forbidden states

- A body in a response to a public reader, a customer, or any principal with neither
  `pages:update` nor `pages:publish` over the page.
- Storefront markup derived from the body, from a persisted body mutation that was not
  published, or from the full artifact document.
- A storefront fallback content source when the landing is absent.
- A source-access or visibility decision made from the role label alone, or one that differs
  from the write rule of the same principal over the same page.
- The record of an unpublished page in a response to a principal without a page authoring
  permission.
- A cached landing or landing document that outlives a page event on the server, or a cache
  entry for "no landing".
- A storefront route that resolves for a published page without a landing.
- A public search document or a sitemap entry for a published page without a landing, or
  search text taken from the page body or from a landing that is no longer bound.
- A second cache of page content in a storefront application.

## Non-goals

- Draft preview on a storefront. A storefront holds no author credential and renders no
  unsanitized document; the working copy is previewed in the Page Builder.
- A cache for the page record (status, translation, channel list). Only the landing is
  cached; the record is read per request and decides which principal sees what.
- A read-only permission for the authoring document or for unpublished pages.
- The role label itself. It stays an inferred heuristic owned by `rustok-core`; this decision
  only stops Pages reads from depending on it.
- Search text for a page locale that has no landing of its own. Such a locale is served the
  landing of a fallback locale; its search document carries the titles of the page and no
  landing text, because the projection has no tenant default locale to resolve the fallback.

## Data, transaction, and concurrency boundary

No schema change. `landing` is loaded in the artifact owner's transaction, which reads the page
row, channel visibility, the binding and the artifact, and verifies artifact integrity. The page
and its landing are two reads inside one request: a publish between them can pair the new
landing with the previous translation fields for that single response.

## Context dimensions

- Tenant: the request tenant; a tenant argument cannot select another tenant.
- Channel: `landing` follows the request channel for every caller. The page record follows
  it for visitors, on GraphQL and on REST; a page author resolves the record on any channel.
- Locale: the effective locale of the page, then the tenant default, then the platform
  fallback; `landing.locale` reports the locale that was used.
- Principal: decides `body` and the visibility of unpublished records. `landing` is the same
  for every principal on a channel, which is why its cache entry carries none.
- Trace, timezone, currency: not applicable.

## Events and projections

No new events. The search projection is refreshed by the existing page events; a page that
loses its last landing binding drops out of the index at the next refresh of that page. The native storefront snapshot cache keeps its generation-based invalidation.
`PageDetail` rejects unknown fields, so a snapshot of another shape is a decode error and is
reloaded from the source.

The `landing` cache entry lives in the artifact scope under a `landing` variant and rotates
with the route, page and artifact generations: a hit skips the owner's status and
channel-visibility decision, so a page update (which rotates only the route and page
generations) must evict it as well. `GqlPublishedLanding` rejects unknown fields, so an entry
of another shape is a decode error and is treated as a miss.

## Failure semantics

- An artifact integrity failure fails the read (GraphQL error, server function error). It is
  never reported as a page without a landing.
- The Next reader fails on any GraphQL error, also when partial data came with it.
- An absent landing is a normal state, not an error: `landing` is `null`, and a storefront
  route answers "not found".
- Every cache failure (generation read, key, read, decode, fill) degrades to the owner read
  and is logged; it never fails the page read.

## Migration and cutover

One cutover, no compatibility layer:

- The synthetic body formats `fly_artifact_payload` and `fly_artifact_url`, `PageBody`,
  `StaticLandingBodyPayload`, `PageBuilderPageBody` and the format constants are removed from
  `rustok-pages-storefront`. `PagesRichTextRenderer` goes with them: it was used only by the
  removed branch that rendered the authoring document (see Consequences).
- `fetchPageArtifact`, `parseArtifactHtml`, `ARTIFACT_ENVELOPE`, `ArtifactEnvelopeError` and
  `pageCacheTags` are removed from `@rustok/pages-frontend`.
- External GraphQL or REST consumers that read `body` with neither `pages:update` nor
  `pages:publish` now receive `null` and must select `landing`. This includes staff tokens that
  hold `pages:read` only: there is no read-only permission for the document.
- A release that predates this change, reading a cached storefront snapshot written by this
  one, shows the page without content until the entry expires (60 seconds).
- The Next storefront routes `/api/draft`, `/api/disable-draft` and `/api/revalidate`,
  `DraftPreviewBanner` and the `REVALIDATE_SECRET` / `PREVIEW_SECRET` settings are removed.
  Nothing called the revalidation route and no read was tagged; a deployment that calls it
  gets 404 and can drop the call. The Next admin no longer renders a "Preview draft" link
  (it carried the secret in its `href`); `storefrontUrlForPage` returns `null` for a page that
  is not published and takes no secret.
- Tokens that hold `pages:read` / `pages:list` without any page authoring permission no
  longer see unpublished pages and are treated as visitors on GraphQL and on REST
  `GET /api/pages`: channel visibility and the per-channel module binding apply to them.
- The Leptos host answers 404 for the route of a published page that has no landing. Before,
  it rendered the page title with an empty state.

## Alternatives considered

1. **Keep `body` public and add `landing` beside it.** Rejected: the unsanitized authoring
   document stays readable by anyone, and every consumer keeps the choice between two
   representations.
2. **Keep the single-document artifact endpoint as the storefront transport.** Rejected: the
   consumer must split a document by an envelope defined in another crate, in a second request
   that can observe a different page state than the first.
3. **Gate the body by role (`!Customer`).** Rejected: the role label is inferred from the
   permission snapshot; the default customer permission set satisfies the staff heuristic, and
   a token holding only `pages:*` is labelled customer and would lose the document it may save.
4. **Keep the draft with `pages:update` only.** Rejected: a publisher without update authority
   would publish a draft it cannot read.
5. **Cache page reads in the Next data cache and revalidate on publish.** Rejected: it needs a
   server-to-storefront webhook with its own secret, retries and failure mode, and until the
   call arrives the storefront serves a landing the server has already replaced. The server
   cache is invalidated in-process by the same events and serves every storefront.
6. **Remove `GET /api/pages/{id}/artifact`.** Rejected: it is the only cacheable `GET`
   representation of a published page and the only one that is a complete document; `landing`
   is markup for a host document and travels over GraphQL `POST`.
7. **Read-only visibility of unpublished pages through `pages:read`.** Rejected: customers
   hold it. It would need a new permission; none is introduced here.

## Verification

- `crates/modules/rustok-pages/src/services/rbac.rs` unit tests: the source follows the
  save and publish write rules; unpublished records need an authoring permission whatever the
  role label; default customer permissions grant neither.
- `tests/page_body_draft_and_history_sqlite.rs`: source visibility on by-id and by-slug reads,
  including the draft for a publisher; the revision journal is readable under the same rule.
- `tests/search_projection_sqlite.rs`: the search document carries the text of the bound
  landing, never of a superseded landing or of the page body; a page without a bound landing
  is not indexed; a locale without a landing of its own is indexed without landing text.
- `tests/public_page_landing_graphql_sqlite.rs`: `pageBySlug`/`page` return the landing and no
  body to public readers; an unpublished body mutation reaches neither; `landing` follows the
  request channel and a signed-in customer is a visitor of it; an unpublished page is `null`
  for visitors; the source needs save or publish authority; a `client_credentials` token
  whose subject id equals the page author does not own the page (own-pages `pages:update`
  yields no body, `pages:manage` does); the landing is read through the
  cache (miss, fill, hit without the owner, channel separation, degraded read, eviction by a
  page update, no entry for "no landing").
- `storefront/tests/native_storefront_*_sqlite.rs` and
  `apps/server/tests/pages_production_relay_native_route_sqlite.rs`: the native storefront
  response carries the landing and no authoring document.
- `storefront/tests/host_route_decision_sqlite.rs`: a published page without a landing
  resolves neither by its canonical route nor by an alias; with a landing it does.
- `tests/artifact_http_cache_sqlite.rs`: the cached landing document and its `304` do not
  outlive a page update. `src/cache_invalidation.rs` and `src/controllers/mod.rs` unit tests:
  the variant separates representation, route and page generation, locale, fallback locale
  and channel.
- `tests/page_http_visitor_channel_sqlite.rs`: REST `GET /api/pages` hides a page outside the
  request channel from a token without authoring authority and resolves it for an author.
- `tests/page_body_draft_and_history_sqlite.rs` also checks `document_readable` for a reader,
  a publisher, a foreign owner-scoped editor and a public reader.
  `crates/modules/rustok-pages/admin/src/browser_problem.rs` unit test: a withheld document is
  `403 DOCUMENT_UNAVAILABLE`. The admin workspace branch and the project store refusal have
  no test.
- `apps/next-frontend/packages/rustok-pages/src/pages-frontend.test.mjs`: the query selects
  the landing fields of the server type and never the body.
- `crates/modules/rustok-pages/src/seo_targets.rs` unit tests: image lookup in landing markup,
  the description text, and the channel on which each scope reads the landing.
- `crates/modules/rustok-pages/src/seo_target_tests.rs`: the SEO record read through the
  registered provider after a reviewed publish. The description and the image come from the
  landing; a stored document edit that is not published changes neither, in the public and in
  the authoring scope; an authoring view without a channel, or on a channel that does not
  admit the page, reads the same landing values as the public route; a hand-written meta
  description wins; a visitor of another channel gets no record; a page without a landing
  binding has no image and falls back to its title; the sitemap lists a page with its landing
  image and omits a page without a landing.

## Consequences

- Every Next page view is one GraphQL request. The landing part of it is answered from the
  server cache; the page record is read from the database.
- A page published without a reviewed landing has no storefront route on either storefront.
  The production publish command always binds a landing; only the non-builder service
  lifecycle (`publish_non_builder`) produces such a page.
- The Leptos host route decision reads the landing binding on every page view, in addition
  to the route and the page record.
- Staff with `pages:read` only can no longer open the document of a page or see unpublished
  pages. Read authority cannot carry either, because the default customer permission set
  holds it too; a read-only view of authoring state needs its own permission.
- The Page Builder capability preflight still maps `Preview` and `Tree` to `pages:read`
  (`graphql/builder_rollout.rs`). It is a rollout and capability check, not the document
  gate, and its mapping is a contract with `rustok-page-builder`. A principal that passes it
  with read authority only gets no document and `documentReadable: false`. The Leptos admin
  then shows "Document not available" instead of the editor, its browser intent endpoint
  answers `403 DOCUMENT_UNAVAILABLE`, and its project store refuses to load a project.
- REST `GET /api/pages` requires a token; it returns the page record without a landing and
  is not a storefront transport.
- Rich text: no implementation of the `fly::RichTextHtmlRenderer` port remains in the
  workspace. `PagesRichTextRenderer`, the binding to the platform rich-text capability
  (`rustok-content`, Article profile) that decision 1 of
  [the rich-text ADR](./2026-10-14-fly-rich-text-embed-action-runtime-and-adapters.md) names,
  was called only where the Leptos storefront rendered the authoring document, and that branch
  is removed. The reviewed publish compiles a landing through `fly::render_page`, without a
  renderer, as it did before this decision, so `flyRichText` content in a published landing is
  the escaped fallback projection of `fly`, not the output of the platform renderer. The native
  storefront read already served that landing. The GraphQL fallback read of an `ssr` build
  served the authoring document and rendered its rich text through the platform renderer; on
  that read the rich text a visitor sees changes to the fallback projection. Binding the
  platform renderer in the publish compilation changes the markup, and so the hash, of every
  landing that carries rich text; it is recorded in the Page Builder backlog and is not part
  of this decision.
- SEO: the primary image is found only as an `<img>` in the landing body markup. An image that
  exists only as a CSS background in the landing stylesheet is no longer picked up; the
  previous lookup read image keys of the authoring document. In the authoring scope, a page
  that is not published has no content-derived description; a published page restricted to
  channels is read on the request channel when that channel admits it and on its first
  channel otherwise, because the SEO editor passes no channel.
- The role label of a request is still inferred from its permission snapshot
  (`SecurityContext::from_permission_snapshot`), and the default customer permission set still
  satisfies the staff heuristic. Pages reads no longer depend on it. `permission_scope_for_set`
  in `rustok-core` does: a principal labelled non-staff gets the `Own` scope for
  `pages:update` and `pages:delete`, so a staff token whose permissions do not satisfy the
  heuristic can save only its own pages. `rustok-blog` has the same label test in its
  non-public read rule.
