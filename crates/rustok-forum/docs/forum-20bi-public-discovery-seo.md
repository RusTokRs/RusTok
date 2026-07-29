# FORUM-20BI — exact public discovery, SEO, and Search routes

`FORUM-20BI` publishes one anonymous public-discovery owner for Forum
cross-consumer surfaces. SEO and route resolution no longer infer public access
from system reads or transport-local open/channel checks.

## Delivered boundary

- `ForumPublicDiscoveryService` composes the exact category and topic audience
  read owners for anonymous consumers.
- Public category discovery enforces the inherited public/authenticated floor
  and every richer category audience layer.
- Public topic discovery additionally enforces open status, route-channel
  visibility, inherited category layers, and optional topic-local narrowing.
- Missing, foreign, closed, route-channel-denied, inherited-policy-denied, and
  topic-policy-denied targets are all represented as absent.
- Authentication-, role-, trust-, Groups-, explicit-user-, and unavailable
  Channel-dependent targets cannot be weakened into public SEO visibility.
- Existing Forum SEO authoring loads retain the legacy managed/system read so
  editors may administer targets independently of public publication.
- Public SEO target loads, route resolution, bulk summaries, and sitemap
  candidates are filtered through exact anonymous discovery.
- Existing SEO target slugs, record mapping, Open Graph fields, structured data,
  canonical route shape, and alternate locale mapping remain owned by the
  existing mapper in `seo_targets.rs`.
- Search now recognizes only canonical `forum/forum_category` and
  `forum/forum_topic` result pairs and maps them to the module-owned storefront
  query keys `category` and `topic`.
- Spoofed source/entity pairs and Forum replies without a delivered storefront
  deep-link contract remain non-navigable.

## Search/index scope

This slice does not insert Forum rows into `search_documents`, subscribe the
Search ingestion handler to Forum events, or duplicate Forum audience policy in
Search SQL. The exact public-discovery owner and canonical URL contract are the
required admission and navigation boundaries for that downstream consumer.

The full Search-owned Forum projection must consume a neutral exact-discovery
capability, remove or reauthorize documents after policy changes, and preserve
owner revision ordering. That work remains `FORUM-20BJ` and the broader
`FORUM-23` projection program.

## Compatibility

No migration, dependency, REST route, GraphQL field, SEO target slug, Forum
storefront route, public DTO, or legacy product/content/blog Search URL changed.
The old GraphQL `query.rs` snapshot and bulk read commands are untouched.

The canonical `implementation-plan.md` and `CRATE_API.md` are not replaced
through the GitHub contents API in this slice. Their conflict-safe
repository-local synchronization debt is recorded in the machine contract.

## Validation handoff

The implementation agent did not run tests, Cargo commands, formatting,
verifiers, workflows, or CI, per maintainer request.

Suggested maintainer commands:

```text
cargo test -p rustok-forum seo_audience_targets -- --nocapture
cargo test -p rustok-search canonical_url_derives_forum -- --nocapture
node scripts/verify/verify-forum-category-audience-read.mjs
node scripts/verify/verify-forum-public-discovery-seo.mjs
node scripts/verify/verify-search-canonical-url-contract.mjs
cargo xtask module validate forum
```
