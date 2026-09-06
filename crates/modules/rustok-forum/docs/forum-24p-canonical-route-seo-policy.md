# FORUM-24P canonical route SEO and hreflang policy

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24P aligns the existing visibility-safe Forum SEO target providers with the localized storefront routes already owned and mounted by FORUM-24A through FORUM-24O:

```text
/{locale}/forum/c/{slug}
/{locale}/forum/t/{short_id}/{slug}
```

The slice changes only route identity inside the existing public SEO composition. It preserves the established Forum title, description, robots, Open Graph and structured-data mapping.

Machine contract:

```text
crates/rustok-forum/contracts/forum-canonical-route-seo-policy.json
```

## Owner boundary

The registered audience-safe SEO wrappers continue to delegate content mapping to the existing Forum SEO providers and public authorization to `ForumPublicDiscoveryService`.

Canonical and alternate paths come only from:

```rust
ForumCategoryRouteService
ForumTopicRouteService
```

The SEO wrapper does not rebuild slug normalization, short topic identity, merge resolution, alias lookup, category fallback order or archived-route policy. Route-resolution conflicts and persistence failures propagate instead of selecting an arbitrary canonical path.

Legacy UUID module routes remain accepted by the existing SEO resolver for compatibility. Managed authoring loads retain their legacy mapping so archived or unpublished records remain configurable. Public target loads, hreflang alternates, bulk summaries and sitemap candidates emit only localized owner routes.

## Canonical and alternate policy

For an admitted category or topic:

- the effective translation receives the exact owner-provided canonical route;
- every available locale is resolved independently through the corresponding route owner;
- a requested alternate that resolves through another locale is rejected rather than emitted under the wrong hreflang;
- alternates are deduplicated and sorted by normalized locale;
- the canonical locale is always represented;
- the public alternate set is bounded to 64 entries.

This slice does not invent or change `x-default`. Selection of a tenant-level default alternate remains in the shared SEO routing/composition layer.

Category aliases and topic aliases may be used to identify the canonical target, but only the current owner descriptor is emitted. An owner-authorized topic `gone` decision produces no SEO target.

## Public discovery

Public category and topic targets are still rechecked through the exact anonymous discovery owner before any canonical or alternate route is exposed. Private, pending, archived, audience-hidden and channel-hidden targets remain absent.

Authoring scope remains available for managed SEO configuration and delegates directly to the legacy mapper. The public route rewrite does not broaden discovery or make authoring depend on public route availability.

## Rust storefront head composition

The exact canonical category and topic route handlers now fetch the existing SEO page context after overwriting the selected owner UUID in the internal module query:

- category routes remove an unrelated `topic` selector;
- topic routes remove an unrelated `category` selector;
- the route owner remains the only HTTP canonical/redirect/gone authority;
- an SEO context redirect cannot override the route-owner decision;
- optional SEO transport failure logs the failure and renders the canonical document without an SEO head instead of converting a valid Forum route into an outage.

The returned public SEO target has already been rewritten to the canonical localized path, so the rendered head uses the public route rather than the internal UUID module compatibility route.

## Structured-data boundary

Existing semantics are preserved:

- categories use `CollectionPage`;
- topics use `DiscussionForumPosting`.

This slice does not emit `QAPage`, `Question` or `Answer`. Those semantics require the explicit topic-kind and accepted-answer policy planned under FORUM-22. It also does not add breadcrumbs, pagination, reply structured data or a new schema mapper.

## Compatibility

This slice does not:

- change category or topic commands;
- change route alias or tombstone storage;
- change topic `410 Gone` authorization;
- change Search result routes;
- change the Next storefront;
- add a migration.

The stale public-discovery SEO verifier is updated to recognize the canonical category/topic card paths and the current wrapper helper names while retaining its existing audience and Search assertions.

## Verification handoff

No tests, Node verifiers, formatting, Cargo commands, SQLite/PostgreSQL execution, migrations, workflows, registered-host requests, browser scenarios or CI were executed while preparing this slice.

Maintainers can run:

```bash
node scripts/verify/verify-forum-canonical-route-seo-policy.mjs
node scripts/verify/verify-forum-public-discovery-seo.mjs
cargo test -p rustok-forum seo_audience_targets::tests -- --nocapture
cargo test -p rustok-forum --test canonical_route_seo_policy_contract -- --nocapture
cargo test -p rustok-storefront forum_category_route::tests -- --nocapture
cargo test -p rustok-storefront forum_topic_route::tests -- --nocapture
cargo check -p rustok-forum --all-targets
cargo check -p rustok-storefront --all-targets --features ssr
```

## Remaining FORUM-24 scope

- Next storefront canonical-route and head parity;
- topic-kind-aware `QAPage` semantics after FORUM-22;
- retained SQLite, PostgreSQL, registered-host, sitemap and browser evidence;
- final canonical-plan ledger reconciliation after maintainer execution.

`crates/rustok-forum/docs/implementation-plan.md` remains the only authoritative roadmap. The connected complete-file writer cannot safely retrieve and replace the full plan losslessly, so this task document records the stable FORUM-24P contract without creating a second backlog or claiming ledger synchronization.
