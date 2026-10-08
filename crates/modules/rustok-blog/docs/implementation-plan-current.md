# rustok-blog canonical implementation cursor

Status: `canonical_reference_v1_source_certified_execution_evidence_pending`.

This document is the canonical **current** source cursor for `rustok-blog`.
`crates/modules/rustok-blog/docs/implementation-plan.md` and the standalone
`implementation-plan-slice-*.md` files are historical implementation records.
They remain useful for provenance, but statements in them about a live Blog
Category Translation provider, Blog Category translation donor tables, or a
pending slice-98 PostgreSQL execution gate are superseded by this file. The
owner-scoped documentation cleanup that followed the source cutover is complete
through TAXONOMY-CAT-17.

## Canonical reference v1 hardening

The first post-layout certification audit found semantic defects that are unsafe
to propagate into other modules. The active reference therefore includes both
physical and semantic gates.

The hardened contract now requires:

- truthful locale write provenance: read fallback never supplies a write locale,
  and a new locale never copies localized text from another locale;
- `rustok_api::Patch<T>` for nullable edit semantics;
- mandatory predecessor `version` and compare-and-swap post updates;
- production lifecycle commands using the domain transition table, including an
  explicit Archived -> Draft restore command;
- derived Comments counters that preserve Blog business `version` and
  `updated_at` and publish locale-neutral reindex requests;
- Comments lifecycle projections serialize on the tenant-scoped Blog post row lock, order delivery state per comment by envelope id, derive counter transitions from lifecycle state, record update/status-change events without changing `comment_count`, and never mutate Blog business `version` or `updated_at`;
- public-comment snapshot keys include the latest committed monotonic Blog comment projection revision per tenant/post; invalidation survives cross-comment out-of-order delivery and requires no cache-key enumeration or second invalidation index;
- current-tenant authority on GraphQL reads and writes;
- one redacted Blog public-error mapping boundary;
- private persistence entities and integrations that consume owner service state;
- deterministic typed list ordering and fail-closed missing-translation reads;
- stable Comments write `command_id` across retries while FBA remains honestly
  `boundary_ready`;
- module-owned UI separation into core commands/presentation/tests, transport,
  and render components;
- one owner permission namespace for Blog post reads/writes (`BlogPosts`, never the
  generic `Posts` resource as a privileged-read fallback);
- storage/canonical-projection corruption classified as internal invariant failures,
  including persisted status decoding and missing/duplicate Taxonomy Category rows;
- typed Taxonomy dependency failures preserving 404/409/internal semantics across
  the Blog boundary;
- native admin/storefront server functions using fallible host-context lookup and
  redacted owner-owned internal errors rather than raw runtime/dependency details;
- runtime SEO/Reaction extension registration failures are composition-time internal
  errors, never user-input validation failures; the core `Error::Internal` category is preserved
  through `ModuleRegistry::build_runtime_extensions()`.

The machine gates are `npm run verify:module-source-layout` and
`npm run verify:module-reference-contract`. The semantic contract is documented
in `DECISIONS/2026-09-18-canonical-native-module-reference-contract.md`.

The final source audit after #4072 is complete at the architecture/source level.
Maintainer-owned compile/test/runtime evidence remains separate; this status does not
promote Comments FBA beyond `boundary_ready`.

Blog FBA registry schema v16 and Comments projection evidence schema v7 encode
the corrected derived-state contract. Runtime/remote evidence is still pending,
so this hardening does not claim `transport_verified`.

## Current Category ownership

The Blog Category migration to canonical Taxonomy is source-complete through
TAXONOMY-CAT-12. CAT-13..CAT-17 actualize the owner-scoped planning, Translation,
registry, database-map, and long-form documentation around that completed source
boundary; they do not move the production cutover past CAT-12.

Canonical interpretation:

`blog_category_taxonomy_cutover = source_complete_through_cat12`

`blog_category_documentation_cursor = owner_scoped_actualized_through_cat17`

The retained ownership boundary is:

- `rustok-taxonomy` owns canonical Blog Category localized copy, route history,
  and the Taxonomy Category projection used by Blog public/owner reads;
- Blog Category create/update commands synchronize canonical Taxonomy state in
  the owner transaction;
- Blog public `get`/`list`, post `category_name` projection, and mutation
  responses read canonical Taxonomy state rather than the retired Blog
  translation mirror;
- Category hierarchy mutations synchronize the Taxonomy hierarchy in the same
  Blog owner transaction;
- Category delete delegates canonical lifecycle cleanup to Taxonomy;
- `blog_categories` remains Blog-owned for module membership, settings, owner
  revision and local command invariants. CAT-12 does **not** transfer or drop
  that table or the typed Taxonomy binding.

### Completed Category continuation

The continuation after the historical Blog cursor is:

- CAT-1..CAT-4: establish typed Taxonomy ownership/binding and canonical read
  seams;
- CAT-5..CAT-6: synchronize Category hierarchy/structure to Taxonomy;
- CAT-7: return Category update responses from canonical Taxonomy;
- CAT-8: retire `BlogCategoryTranslationTargetProvider` and host registration;
- CAT-9: retire writes to the Blog Translation change journal;
- CAT-10: retire live `blog_category_translation` mirror reads/writes and the
  compatibility bridge from Category commands;
- CAT-11: append irreversible migration
  `m20260828_000021_retire_blog_category_legacy_storage` after the historical
  Taxonomy backfill, fail closed unless same-ID Taxonomy ownership is present,
  then drop `blog_category_translations` and `blog_translation_changes`;
- CAT-12: remove the inert Translation bridge module and unregistered change
  entity, while retaining only the crate-private donor translation entity
  needed by the historical `000020` upgrade backfill;
- CAT-13: actualize the canonical Blog planning cursor and active Blog README
  surfaces, retire orphaned provider-era PostgreSQL evidence/verifier sources,
  and guard the post-cutover source boundary;
- CAT-14: actualize cross-owner Taxonomy/Flex planning and the central database
  map while preserving the accepted no-duplicate-provider ownership ADR;
- CAT-15: actualize central/module Translation plans and the machine-readable
  Translation surface registry so `blog_categories` is `excluded` /
  `not_registered` and `taxonomy_terms` remains the canonical registered owner;
- CAT-16: actualize the central module registry and remove the obsolete
  Blog-specific Category Translation recovery/readiness gate;
- CAT-17: align the long-form Blog plan's live ownership summary and former
  Translation-pilot section with canonical Taxonomy ownership and add a focused
  exact-head guard against provider-era drift.

Focused exact-head contracts for the completed continuation cover canonical
commands, mutation responses, reads, post category-name projection, hierarchy,
delete lifecycle, donor-storage retirement, `rustok-blog --lib` compilation with
warnings denied, and the CAT-13..CAT-17 owner/documentation boundaries.

## Superseded Category Translation pilot

Slice 98 is a historical source record for the former `blog/category`
Translation-target pilot. Its proposed provider, PostgreSQL harness, Blog change
journal and execution evidence are **not** a live readiness gate anymore.

Canonical interpretation:

`blog_category_translation_provider = retired`

`blog_category_translation_postgres_evidence = superseded_by_taxonomy_cutover`

Do not recreate or execute the retired Blog provider/harness merely to satisfy
slice-98 language. The production provider source, provider tests, change
writer, change entity, donor journal and donor translation storage have been
retired in later bounded CAT slices. Historical migration files and historical
slice documents remain immutable upgrade/provenance records.

Any Translation-control-plane onboarding for Blog Categories must now target the
canonical Taxonomy owner contract. It must not restore direct Blog Category
localized storage or a second `blog/category` provider.

## Reference-v1 category validation boundary

The fresh Taxonomy boundary audit found and closed one contract mismatch: Blog previously advertised and locally accepted Category names up to 255 characters, while the canonical Taxonomy Category owner rejects names above 120 characters. Blog DTO/OpenAPI metadata and service validation now enforce the canonical 120-character bound before opening the owner mutation path.

## Reference-v1 category route normalization boundary

The fresh Taxonomy boundary audit found a second concrete contract mismatch: Blog had its own ASCII-only slug normalizer, while canonical Taxonomy uses the shared routable route-key normalizer with transliteration. Blog could therefore reject localized Category names such as Cyrillic names without an explicit ASCII slug even though the canonical owner could represent them. Blog now delegates route normalization to Taxonomy and keeps a focused regression test for localized route generation.

## Reference-v1 category route storage boundary

Taxonomy is the single canonical owner of localized Category route keys and
historical aliases. Blog stores only Category membership/settings/revision; it
does not persist Category slug, parent, position, or depth. Category command
writes synchronize canonical Taxonomy route/placement state in the same owner
transaction, while Blog reads project the canonical Taxonomy representation.

## Reference-v1 Tag input validation boundary

The fresh Taxonomy boundary audit found one concrete Blog/Taxonomy contract
mismatch: Blog Tag name validation used UTF-8 byte length while the public DTO
max_length contract and canonical Taxonomy name validation use Unicode
character count, and the Tag slug schema was not enforced by the owner service.
Blog now validates both name and slug limits at the service boundary. Post tag
mutations also revalidate resolved Blog-owned module terms, so the stricter
Blog Tag limit cannot be bypassed through post creation/update while shared
global Taxonomy terms remain unaffected. Focused regression coverage remains
for the Unicode name and slug limits.

A fresh Taxonomy migration audit found that canonical Category route keys are stored at 120 characters, while Blog previously exposed a 255-character slug schema and delegated oversized normalized keys to the persistence layer. Blog now validates the normalized route key at the command boundary and both Create/Update DTO schemas advertise the canonical 120-character limit.

## Other retained Blog source tracks

The Category migration does not reopen unrelated source-complete tracks from the
previous cursor. Their latest retained source states remain:

- `remote_comments_transport = source_implemented_maintainer_execution_pending`;
- `canonical_outbox_relay_postgres_evidence_source_ready_maintainer_execution_pending`;
- `cached_public_comments_snapshot = source_implemented_maintainer_execution_pending`;
- `comment_form_policy = source_verified_hide_on_provider_absence`; the active storefront writes only when Comments is available, while unavailable/timeout states preserve explicit read availability and may render the cached approved snapshot.
- `tag_list_pagination = source_complete_maintainer_execution_pending`;
- `tag_canonical_projection = source_complete_maintainer_execution_pending`;
- `tag_mutation_atomic_reindex = source_complete_maintainer_execution_pending`;
- `global_tag_search_invalidation = source_complete_maintainer_execution_pending`;
- `post_category_name_projection = source_complete_canonical_taxonomy_read`.
- `comment_target_lifecycle_guard = source_complete`; comment reads and mutations that begin from a Comments record revalidate canonical Blog post existence, so asynchronously stale Comments threads cannot remain operable after terminal post deletion.
- `comment_create_target_race_compensation = source_complete`; comment creation revalidates the canonical Blog post after the external Comments write and compensates a comment created after terminal post deletion, while preserving the durable `TargetDeleted` cleanup backstop.

For tags, Taxonomy remains the shared dictionary owner and Blog retains
`blog_post_tags` attachment ownership. Global Taxonomy tags may be attached and
read by Blog, but shared global terms are mutated only by the Taxonomy owner;
Blog tag mutations apply only to `module:blog` terms. The Blog-owned
`blog_tag_usage` projection is derived from canonical attachments and Taxonomy
`canonical_key`, is maintained in the same transactions as post/tag mutations,
and provides the database-bounded tag-list read path. Global Taxonomy Tag updates,
deletes, and exact-locale Translation-target applies also enqueue the existing
tenant-scoped Search reindex event inside the Taxonomy transaction, keeping Blog
Search projections fresh without a Blog-to-Taxonomy runtime dependency. Zero-use Blog-local terms
remain in the projection; zero-use global terms are removed. For Comments, the execution-owned
transport/restart/relay evidence remains separate from Category Taxonomy work.

## Accepted Comments and Reactions capability cutover

The static `blog -> comments` lifecycle edge has been removed. The canonical
source now requires:

1. Blog serving remains functional with Comments absent; only comment operations
   require the optional capability;
2. `CommentsThreadPort` and lifecycle projections remain the only cross-owner
   boundary, with no local or embedded provider fallback in Blog;
3. provider absence maps to `COMMENTS_PROVIDER_UNAVAILABLE` for writes and to
   explicit `UNAVAILABLE`/`TIMEOUT` read states for public comments;
4. the active storefront hides the comment write surface when the provider is
   unavailable or timed out and may preserve a valid approved snapshot for reads;
5. Reactions remains an independent optional capability owned by its own provider;
6. Profiles is optional presentation enrichment: provider absence yields no author
   profile and never blocks Blog publication reads or triggers direct Profiles storage
   access.

## 2026-09-24 Blog Category settings boundary

Category settings now have one owner-level persistence contract: they must be JSON objects and
must not exceed 64 KiB when encoded. Create/update commands enforce input validation, while
list/get revalidate persisted state as an internal invariant before exposing it through the
Blog Category API. Migration `m20260924_000029_enforce_blog_category_settings_contract` now
adds the database backstop on PostgreSQL and SQLite and fails the upgrade before constraints are
installed when pre-existing rows violate the contract. This prevents malformed persisted settings
from being written or returned as ordinary Category data.

## 2026-09-24 Shared global Tag Search invalidation

The fresh cross-owner Blog Search audit found that canonical Search documents resolve global Taxonomy Tag labels directly from Taxonomy tables. A Taxonomy-owned global Tag update, delete, or exact-locale Translation-target apply could therefore change source truth without invalidating Search. Slice 105 closes this gap by writing the existing tenant-scoped `index.reindex_requested` event from inside the Taxonomy transaction. Module-owned Tags are excluded from this global rebuild because their owning modules retain narrow invalidation ownership.

`global_tag_search_invalidation = source_complete_maintainer_execution_pending`

The Search module already consumes the generic reindex event, so no Blog↔Taxonomy runtime dependency and no new event type are introduced. Runtime/build/gatekeeper/tests remain maintainer-owned and unrun.

## Remaining execution-owned results

The retained maintainer/runtime evidence backlog is now limited to tracks whose
source still exists and whose result has not been superseded:

1. Execute the retained Comments transport/composition, restart/ambiguity,
   canonical relay and cached-snapshot evidence at an exact revision.
2. Execute the retained canonical tag projection and tag
   mutation/outbox rollback/delete-cascade evidence before runtime promotion.
3. Execute global Taxonomy Tag Search invalidation evidence, including direct
   update/delete, exact-locale Translation-target apply, module-owned negative
   coverage, and transactional outbox rollback.
4. Audit deployed data for metadata-only legacy tag rows before canonical tag
   projection rollout; backfill owner relations if such rows exist.
5. Execute category CRUD/Search refresh/canonical navigation/mounted rate-limit
   evidence that remains applicable to the current Taxonomy-backed Category
   implementation.
6. Execute the Blog article richtext cutover/backfill/browser evidence already
   retained by the historical plan.
7. Run Blog migration smoke against PostgreSQL and SQLite, including clean up-from-zero,
   incremental upgrade, direct invalid settings rejection, oversized settings rejection,
   and dirty-data preflight failure.

There is **no** remaining execution item for the retired Blog Category
Translation provider or its deleted PostgreSQL harness.

## Documentation follow-up

The owner-scoped Blog Category cleanup is complete through CAT-17. Active Blog,
Translation, Taxonomy/database-map, central module-registry, and long-form Blog
ownership surfaces now describe canonical Taxonomy ownership without treating a
second `blog/category` provider, donor tables, or provider PostgreSQL evidence as
live readiness contracts.

Historical migrations and standalone slice records remain provenance and may
name retired provider/storage concepts in historical context. Any future stale
live claim discovered outside these owner-scoped surfaces is a new independent
documentation gap and must be handled from a fresh `main` under the owning
module's boundary.

## 2026-09-24 GraphQL principal preservation boundary

A fresh Blog GraphQL audit found that authenticated Query reads reconstructed
the domain `SecurityContext` through `from_permission_snapshot`, which always represented
the principal as a user even though the GraphQL transport already validates and
passes `AuthPrincipalContext` and preserves the original grant type.

Blog GraphQL Query reads now use the same
`security_context_from_access_token` bridge as Blog mutations. Direct and
authorization-code user grants retain user ownership; `client_credentials`
service grants remain service principals with no user ownership identity.
Focused source coverage asserts this service-principal preservation.
## 2026-09-24 Blog dependency cleanup boundary

The live Blog module no longer depends directly on `rustok-translation-targets`.
That crate was left behind after the retired Blog Category Translation provider
and is not referenced by the current Blog production source or runtime registration.
Removing the orphan direct dependency keeps the pre-release zero-legacy boundary
explicit: Translation control-plane ownership for Blog Category copy is now entirely
the canonical Taxonomy owner contract.
## 2026-09-24 Tag route-key length boundary

Blog Tag input validation now checks both the user-facing raw slug limit (100
characters) and the canonical Taxonomy route-key limit (120 characters) after
Unicode normalization. This prevents a valid-length Unicode input from expanding
past the 120-character `taxonomy_terms.canonical_key` / localized slug storage
boundary and failing later as a database error.
## Canonical native-module layout baseline

The Blog backend is the first strict reference implementation of the canonical
native module physical layout. The initial move was architecture-only; the
subsequent reference-v1 certification deliberately hardens owner mutation,
multilingual, concurrency, lifecycle, event, transport-error and integration
contracts before the structure is propagated to other modules.

Current source placement is:

- `module.rs` for module/runtime registration;
- `domain/` for the state machine and article richtext policy;
- `services/` for use cases, with `PostService` decomposed into bounded
  command/query/repository/helper/test files;
- `entities/` plus `migrations/` for persistence;
- `integrations/` for SEO, reactions, and public-comment snapshot adapters;
- `graphql/` plus `controllers/` for transport adapters;
- thin `lib.rs` facade with stable compatibility re-exports.

The baseline is guarded by `npm run verify:module-source-layout`. Other existing
modules are not grandfathered as alternate standards; they should be migrated in
bounded follow-up PRs from fresh `main` and enrolled in the verifier after their
physical layout conforms.

## Next cursor

There is no predeclared Blog Category Translation cleanup slice after CAT-17.
Continue only from a fresh repository audit that identifies a new independent
source, registry, or live-documentation gap. Do not manufacture work by
reopening CAT-1..CAT-17, by recreating the retired Blog Category Translation
provider, or by treating historical migration/slice provenance as a live
contract.


## 2026-09-23 Profile presentation boundary

Completed the remaining source-level Blog/Profiles build-time coupling gap. Blog now depends on `rustok-profiles-api`, consumes the optional `ProfileSummaryReader` through manifest-attached GraphQL runtime data, and preserves the existing `ProfileSummary` GraphQL contract. `rustok-profiles` remains the sole implementation owner and the server host composes its audience-aware presentation provider through `HostRuntimeContext`. Provider absence/failure remains a presentation-only degradation. Maintainer runtime evidence, gatekeeper, build, and tests remain unrun by the agent.


## 2026-09-23 Comments package boundary

Completed the remaining source-level Blog/Comments build-time coupling gap. Blog now depends only on `rustok-comments-api`; Comments remains the sole owner of persistence and implements the neutral `CommentsThreadPort` through the host runtime capability graph. Persistence-facing `CommentStatus` and other SeaORM types remain inside Comments, with explicit contract conversions at the owner boundary. Blog publication serving continues to degrade safely when the optional provider is absent. Maintainer runtime evidence, build, gatekeeper, and automated tests remain unrun by the agent.


## 2026-09-24 Server composition contract synchronization

The active Blog dependency contract is `content + taxonomy + outbox + channel`. Profiles remains optional presentation enrichment through `rustok-profiles-api` and must not return as a runtime module dependency. The server module contract test now asserts the canonical dependency set used by the active `modules.toml` and Blog module contract, preventing a stale hard dependency from silently returning during future composition changes. Maintainer runtime evidence, gatekeeper, build, and automated tests remain unrun by the agent.


## 2026-09-24 Comments principal-bound idempotency

The Blog reference audit identified that durable Comments receipts were replayed before owner authorization and were keyed only by tenant/owner/operation/idempotency key/request. The Comments owner now binds every port write receipt to the authenticated PortContext.actor in addition to the existing durable request identity, preventing cross-principal replay inside a tenant without coupling Blog to Comments persistence. The owner contract and static matrix document this invariant. Maintainer runtime evidence, gatekeeper, build, and automated tests remain unrun by the agent.


## 2026-09-24 Comments idempotency source-gate hardening

The Comments principal-bound durable receipt fix is now protected by the Comments port source verifier and its self-test: every write operation must bind the authenticated PortContext.actor, the registry declares the binding tuple, and the evidence matrix carries the idempotency_principal_bound assertion. A unit regression proves same-principal retries retain identity while a different principal does not. Tests, build and CI remain unrun by the agent per maintainer instruction.


## 2026-09-24 Blog storefront authenticated tenant binding

The storefront SSR transport now treats the authenticated AuthContext.tenant_id as an authoritative tenant boundary whenever authentication is present. A caller-supplied tenant_slug is resolved only for unauthenticated/public selection; an authenticated request must match the resolved tenant or the server returns the same generic internal error used for tenant-context mismatches. The boundary verifier and its self-test enforce the tenant-binding markers. Tests/build/CI remain unrun by the agent per maintainer instruction.


## 2026-09-24 Blog GraphQL rate-limit principal identity

A fresh Blog GraphQL rate-limit audit found that authenticated rate-limit keys collapsed every AuthContext into `user:<user_id>`, despite the canonical authentication boundary distinguishing human-user and client-credentials service principals. Blog now derives the limiter actor component from `AuthContext::port_actor()`, preserving principal kind and stable principal id. The source verifier requires the canonical actor binding and the verifier self-test rejects user-only keying. Tests/build/CI remain unrun by the agent per maintainer instruction.


## 2026-09-24 Public comment visibility race compensation

A fresh source audit found a TOCTOU gap in `CommentService::create_public_comment`: the public Blog post/channel boundary was validated before the external Comments create, but the post-create revalidation checked only target existence. A concurrent unpublish, channel-visibility change, inactive channel, or Blog channel disable could therefore leave a comment successfully created after the target stopped being publicly commentable.

The write path now revalidates the full `ensure_public_post_visible(tenant_id, post_id, public_channel_slug)` boundary after the external Comments write. Every post-create visibility-revalidation failure triggers a fresh idempotent Comments delete compensation before the original failure is returned. If compensation itself fails, Blog returns a redacted invariant failure rather than reporting success or silently leaving the foreign-side effect behind. The source verifier/evidence now require generic post-create failure compensation, while runtime/browser evidence remains maintainer-owned and unclaimed.

## 2026-09-24 Public comment compensation on infrastructure failure

The fresh saga audit found that the first TOCTOU fix only compensated `PostNotFound` after the external Comments write. A database, channel-service, or other infrastructure error during the final public-visibility revalidation could still return failure while the Comments record remained committed. The Blog write path now compensates on every revalidation failure, preserves the original error when cleanup succeeds, and fails closed with an invariant error when cleanup itself cannot be completed.

## 2026-09-24 Public comment compensation authority

A second saga audit found that the compensation delete inherited the caller's `SecurityContext`. Ordinary storefront customers have Comments create permission but delete is owner-scoped, so a compensation could itself be rejected by authorization. Blog compensation now uses a trusted `SecurityContext::system()` operation through the existing owner-managed Comments port, with a fresh idempotency key. The original user remains represented by the create-side event/audit context; compensation is explicitly a system-owned cleanup action. Source verification and a focused Rust unit regression enforce this boundary.

## 2026-10-08 Engineering audit remediation (Waves 1 and part of 2)

This section records the remediation of the 2026-10-08 Blog engineering audit.
The governing contract is
[`DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md`](../../../../DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md).

### Fixed in this slice

- **H-1 public locale.** The public GraphQL post list resolves the read locale
  from the filter or the request context (`rustok_api::resolve_graphql_locale`)
  instead of the tenant default. The Next RSS feed, the list page, the home
  section, and the related-articles block pass the route locale explicitly.
  The fallback chain is unchanged: requested, explicit fallback, `en`, first
  available.
- **H-2 slugs for non-ASCII titles.** Post slugs use
  `rustok_taxonomy::normalize_term_route_key`, the same primitive as Taxonomy
  route keys. A Cyrillic title without an explicit slug now produces a
  transliterated slug. Blog keeps no local transliteration copy.
- **H-3 slug redirects on the canonical registry.** Blog keeps no slug-history
  table. Post routes live in the `rustok-content` canonical URL registry and are
  written only through `CanonicalUrlWriter`. Renames keep the old route as an
  alias and emit `CanonicalUrlChanged`, which drives SEO redirects. Creates claim
  a retired route through `release_alias_route_in_tx`. Deletes purge the post's
  routes in the same transaction. `get_post_by_slug_with_locale_fallback` resolves
  the current slug first and then the registry route with the request locale. The
  Next post route issues a permanent redirect to the current slug.
- **H-4 publication timestamp.** `published_at` is the first publication time.
  Publish keeps an existing value; unpublish, archive, and restore preserve it.
  The DTO field is documented accordingly.
- **H-5 dead counter.** `view_count` is removed from the entity, DTOs, GraphQL
  mapping, search projection, the content-orchestration promotion path, and the
  test fixtures. Migration `m20261008_000031` drops the column. Real view
  analytics are not implemented; they belong to a future analytics owner.
- **H-10 featured image.** `featured_image_url` is validated on create and on
  update: absolute `http`/`https` or a root-relative path, at most 2048
  characters, no control characters, no whitespace padding, no
  protocol-relative values.
- **M-1 RSS feed.** CDATA content escapes a literal `]]>`, and the feed is
  locale-scoped. The feed already declares `atom:link rel="self"`. The
  `https://rustok.dev` fallback for `NEXT_PUBLIC_SITE_URL` is still present and
  is tracked below.
- **M-2 SEO fallbacks.** The storefront post query requests `seoTitle`,
  `seoDescription`, and `updatedAt`. Page metadata uses SEO fields first, then
  the excerpt, then a localized default description. JSON-LD uses the SEO fields,
  adds `dateModified`, and uses an absolute `mainEntityOfPage` URL.

### Verification status

- Frontend: `npm ci` in `apps/next-frontend` (with the sandbox CA bundle
  supplied through `NODE_EXTRA_CA_CERTS`), `npm run typecheck` (`tsc --noEmit`),
  and `verify-adrs` pass. ESLint reports the changed files as ignored by the
  current configuration, so lint coverage for them is not established.
- Rust: `cargo` was not available in the authoring environment, so
  `cargo check`, `cargo test -p rustok-blog`, and the PostgreSQL integration
  tests were **not run**. Unit tests were added for slug transliteration and
  featured image validation. Maintainer CI must run them before this slice is
  marked implemented.
- Still required: a PostgreSQL migration smoke for `m20261008_000031`, a
  rename-and-redirect integration test, a claim test for a retired slug, a
  delete-purge test, and a republish test that checks the first `published_at`
  is preserved.

### Open backlog (Known Limitations)

These audit findings are deliberately out of this slice. Each needs its own
owner decision or a larger implementation.

- **H-6 scheduled publishing.** No `scheduled` status, no `publish_at`, and no
  worker. Only `Draft -> Published` exists.
- **H-7 list search and facets.** The `?q=` filter applies to the rows loaded so
  far, and the tag list is built from those rows. `PostsFilter` and
  `PublicPostsFilter` have no text query. Search is not wired to `rustok-search`.
  The public list (`publicBlogPosts`) is keyset-paginated on `(published_at, id)`
  and returns no total. The admin `posts` list is keyset-paginated on the sort
  column and `id` (`nextCursor`, no total); its REST sort options are
  `published_at`, `updated_at`, and `created_at`, and NULL sort values come last.
  The GraphQL `posts` query always sorts by `created_at DESC`.
- **H-8 article richness.** The rich-text profile has no inline images, tables,
  embeds, footnotes, callouts, or heading anchors.
- **H-9 comments.** Readers cannot edit, delete, or report comments. The
  Blog-owned `update_comment`/`delete_comment` service methods are not exposed.
  There are no notifications through `rustok-notifications`. Comment depth is not
  limited.
- **M-1 feed follow-ups.** Only the 25 newest posts. No pagination, Atom,
  per-category, per-tag, or per-author feeds, and no full-text `content:encoded`.
  The site URL fallback is still hard-coded.
- **M-2 follow-ups.** No per-post canonical override, no `noindex`, no separate
  Open Graph image, and the publisher has no logo.
- **M-3 trash.** Published posts cannot be deleted; unpublish followed by delete
  is a hard delete with no recovery window.
- **M-4 preview and revisions.** No draft preview token and no stored revisions.
  Concurrency is handled by `version`, but the Next admin has no conflict UI.
- **M-5 related posts.** Related articles now prefer the same category, but they
  are not ranked by relevance (tags, recency weighting).
- **M-6 editorial features.** No series, sticky posts, co-authors, or author
  archive pages.
- **M-7 archives.** Tag and category archives are query-string filters with no
  crawlable pages, metadata, or canonical URLs.
- **M-8 anonymous read API.** REST `/api/blog/*` requires authentication; reads
  are GraphQL-only for anonymous clients.
- **M-9 audience features.** No newsletter, membership, paid or member-only
  content. Visibility is channel-based only.
- **M-10 reactions.** Posts support only `like`. Comments have no reactions.
- **M-11 admin UX (Next).** Locale is a free-text input, not a select of enabled
  locales. No bulk actions, no admin text search, no autosave, no unsaved-changes
  guard.
- **M-12 duplicated storefronts.** Blog is implemented in both the Next
  storefront and the Leptos `BlogView`. The Leptos storefront resolves retired
  slugs but does not redirect them. The canonical storefront decision is open.
- **M-13 sorting.** The authenticated GraphQL `posts` path hard-codes
  `CreatedAt DESC`, and the filter exposes no sort field.
- **L-1** Counter types are `i32` in storage and `i64` in DTOs
  (`comment_count`); overflow semantics are undocumented.
- **L-2** Reading time is computed on the frontend from plain text, so it can
  differ across consumers.
- **L-3** The table of contents is built on the client and has only heading
  levels 2 to 4.
- **L-4** Most blog documentation covers evidence and migration tracking; there
  is no user-facing feature roadmap.
- **L-5** No frontend tests exist for the Blog storefront packages.
- **L-6** The post page calls `fetchPublishedPost` twice per request (metadata
  and page) without an explicit cache key.

### Competitive reference (from model knowledge, not web-verified)

This comparison was written from the model's knowledge of the platforms and was
**not** re-checked against live product documentation in this slice. Cells marked
"verify" need checking before external use.

| Capability | RusTok Blog (after this slice) | WordPress core | Ghost | Audit ID |
|---|---|---|---|---|
| Draft / publish / archive | Yes | Yes | Yes | |
| Scheduled publishing | No | Yes | Yes | H-6 |
| Revisions / history | No (CAS `version` only) | Yes | verify | M-4 |
| Draft preview link | No | Yes | Yes | M-4 |
| Trash / undo delete | No (hard delete) | Yes (30 days default) | verify | M-3 |
| Sticky post | No | Yes | verify | M-6 |
| Slug change redirect | Yes (Next storefront, canonical route registry) | Yes | Yes (redirects file) | H-3 |
| Localized slugs | No (global canonical slug, by contract) | Via plugins | No | H-2 |
| Multi-author | Single author | Roles; co-authors via plugin | Staff users | M-6 |
| Inline images / embeds / tables | No | Yes (blocks) | Yes (cards) | H-8 |
| Featured image alt text | No (URL only) | Yes (media library) | Yes | H-10 |
| Full-text search of posts | Platform search only, not wired to list | Yes | Yes | H-7 |
| Category and tag archive pages | Query filters only | Yes | Yes | M-7 |
| Threaded comments and moderation | Yes | Yes | Yes | |
| Comment edit, delete, report | No | Plugins / policy | verify | H-9 |
| RSS / Atom per taxonomy | RSS 2.0 only, newest 25 | Yes | Yes | M-1 |
| SEO fields, JSON-LD, sitemap | Yes | Yes (core and plugins) | Yes | M-2 |
| Newsletter / membership | No | Plugins | Yes | M-9 |
| Reactions | `like` on posts only | Plugins | verify | M-10 |
| Anonymous read API | GraphQL only | REST | Content API | M-8 |
| View analytics | Removed (field had no writer) | Plugins | Native | H-5 |
| Multi-tenant modular runtime, outbox events | Yes | No | No | strength |

### Next waves (from the audit roadmap)

1. H-6 scheduled publishing with a worker on the outbox or cron.
2. M-3 trash with retention; M-4 draft preview tokens and revisions.
3. H-7 server-side `q`, tag and category facets, and relevance-based related
   posts (M-5); M-7 crawlable archives; M-1 Atom and per-taxonomy feeds.
4. H-8 article profile extension; H-9 comment edit window, delete, report,
   depth limit, and notifications.
5. M-6 series, sticky posts, and co-authors; M-9 and M-8 as product decisions;
   M-12 choice of the canonical storefront.
