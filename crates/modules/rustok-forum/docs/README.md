# Documentation `rustok-forum`

`rustok-forum` is the domain module for forum/Q&A scenarios. The module operates on
forum-owned persistence and remains an independent bounded-context boundary.

## Canonical roadmap

[The implementation plan](./implementation-plan.md) is the single source of
truth for Forum task status, sequencing, definitions of done, the planned shared
notifications module, and cross-module release gates.

The [Forum moderation revision migration contract](./forum-moderation-revision-migration-contract.md)
documents the schema, trigger parity, and migration guarantees for Forum-owned
moderation revision fencing clocks.

## Purpose

- publish the canonical Forum runtime contract for categories, topics, replies and moderation;
- keep Forum-owned transport surfaces, Q&A capabilities and UI packages inside the module;
- keep REST handlers on a narrow `ForumHttpRuntime` with explicit DB/event bus handles;
- enforce tenant-module lifecycle admission once at the Forum REST router boundary before handler dispatch;
- resolve selected merged-source topic IDs through the immutable merge receipt ledger;
- own deterministic localized topic route identity plus immutable redirect/tombstone history;
- own locale-aware category route identity plus immutable localized slug history without conflating identity and visibility authorization;
- expose manager-only move, merge, split, reply-branch fork and reply-range owners without duplicating policy;
- evolve the Forum as a taxonomy-aware and channel-aware domain with explicit observability.

## Scope

- `CategoryService`, `TopicService`, `ReplyService`, `ModerationService`;
- Forum-owned storage for categories, topics, replies, votes, solutions, subscriptions and user stats;
- transport surfaces: GraphQL, REST, Leptos admin/storefront packages and the module-owned Next-admin package;
- Forum widget contract-freeze and Page Builder consumer evidence;
- tag attachments via `forum_topic_tags` with shared vocabulary in `rustok-taxonomy`;
- visibility, moderation and user-facing derived fields in Forum read/write contracts;
- attachment relation heads/rows and Media owner-reference retention coordination; canonical Category presentation is consumed from Taxonomy rather than reimplemented in Forum.

## Topic and reply restoration

Archiving a topic locks it in the same write. Reopening keeps the lock until an
authorized operator unlocks it. The archive-lock migration also normalizes the
saved lock on archived restore snapshots. Rollback removes enforcement and
preserves those normalized values.

Delete snapshots preserve the exact prior topic status, including `pending`,
and record the authorized path for new deletions. Author requests, moderation,
and internal service operations have distinct reasons. A topic's delete reason
also applies to replies deleted with its thread. Historical snapshots with no
recorded reason remain unset. Restoring a pending topic leaves its first
publication to moderation. Migration rollback refuses to discard a recorded
reason or a pending snapshot.

Category and author counters exclude archived topics and their replies and
solutions. They continue to count pending topics and approved replies in active
threads. The counter migration reconciles rows affected by archived topics, and
solution reconciliation uses the same topic visibility rule.

## Integration

- uses `rustok-content` only as a shared helper/orchestration dependency;
- uses `rustok-taxonomy` as the canonical Category owner (identity, localized copy, hierarchy, routes and presentation);
- uses `rustok-taxonomy` as a shared dictionary for tag identity;
- uses the Media owner port for attachment reference retention/release; Forum does not read Media tables.
- uses `rustok-profiles` for the author presentation contract;
- uses `rustok-channel` for visibility and SEO gating;
- the canonical `use_reactions` engagement setting is persisted in static module
  settings and consumed through the tenant-module runtime API. Forum never imports
  the `tenant_modules` persistence entity for engagement selection; Reactions remains
  optional and selected reaction mode fails closed when that provider is unavailable.
  See [`docs/architecture/settings.md`](../../../../docs/architecture/settings.md);
- selected merge reads resolve through the immutable receipt, while mutation commands keep exact identity semantics;
- `mergeForumTopic` and `mergeForumTopicResolvingSolution` remain the only admin merge command contracts;
- FORUM-21N composes those commands in Leptos and Next-admin without changing the owner, receipt or event schema;
- FORUM-21O selects direct authenticated native owner composition for Leptos SSR/hydrate while retaining GraphQL for CSR/headless with no fallback;
- FORUM-21P adds the transport-neutral selected-reply split owner with immutable receipt/event, parent-closed movement, exact access-policy cloning and counter reconciliation;
- FORUM-21Q adds the transport-neutral reply-branch fork owner with deterministic copied identities, complete bounded revision/relation provenance, source immutability and explicit non-copy policy; FORUM-21U provides its manager GraphQL transport;
- FORUM-21R exposes `splitForumTopicReplies` as a routed-tenant, `forum_topics:manage` GraphQL adapter over the unchanged split owner and immutable receipt; FORUM-21V provides its Leptos and Next-admin composition;
- FORUM-21S adds the transport-neutral bounded reply-range move owner with deterministic append positions, explicit asymmetric parent policy, unchanged reply-owned references and checked ACL/solution/counter reconciliation; FORUM-21T provides its manager GraphQL transport;
- FORUM-21T exposes `moveForumTopicReplyRange` as a routed-tenant, `forum_topics:manage` GraphQL adapter over the unchanged reply-range owner and immutable receipt; FORUM-21X provides its Leptos and Next-admin composition without inferring canonical positions from visible row order;
- FORUM-21U exposes `forkForumTopicReplyBranch` as a routed-tenant, `forum_topics:manage` GraphQL adapter over the unchanged fork owner and immutable receipt; admin composition remains follow-up scope.
- FORUM-21V composes `splitForumTopicReplies` in the module-owned Leptos and Next-admin surfaces with stable retry/target identities and no transport-local movement policy.
- FORUM-24A adds `ForumTopicRouteService`, a twelve-hex topic identity, exact-locale canonical descriptors and an append-only redirect/tombstone ledger; host mounting and owner write composition remain follow-up scope.
- FORUM-24B composes immutable localized source-route redirects into new topic merges in the same owner transaction without changing merge receipts or events.
- FORUM-24C records immutable localized `gone` routes in the topic delete transaction while preserving existing merge redirects.
- FORUM-24D adds an explicit owner command for localized topic slug changes with atomic old-route aliases and delete/merge lifecycle resolution.
- FORUM-24E provides a bounded, cursor-resumable owner repair that ensures exact route aliases for immutable merge receipts created before FORUM-24B.
- FORUM-24F exposes the localized topic slug rename owner through an additive routed-tenant GraphQL mutation while preserving owner-defined update and ownership semantics.
- FORUM-24G composes that mutation in the module-owned Leptos and Next-admin packages without adding route, alias, merge or locale-selection policy to either UI.
- FORUM-24H exposes visibility-safe storefront canonical/redirect resolution through a legacy-compatible GraphQL field and exact category/topic audience recheck.
- FORUM-24I composes the canonical localized topic route in `rustok-forum-storefront` and the shared Rust storefront router, cuts topic-card navigation over from UUID query links, and preserves native/GraphQL selected-path parity.
- FORUM-24J records an immutable anonymous visibility and route-channel snapshot before topic deletion, seals channel scope with count and SHA-256 digest, and exposes only a boolean owner decision.
- FORUM-24K adds a separate GraphQL route decision field, consumes the FORUM-24J boolean in GraphQL and native storefront paths, and maps only owner-authorized tombstones to private no-store `410 Gone` while preserving `404` for missing or unauthorized history.
- FORUM-24L adds `ForumCategoryRouteService` for `/{locale}/forum/c/{slug}`, reuses the existing tenant/locale slug uniqueness and shared locale fallback order, hides archived categories, and fails closed when first-available lookup crosses category identities; transport, aliases and SEO remain follow-up scope.
- FORUM-24M composes explicit and name-derived category slug changes into an append-only tenant/locale route ledger, permanently reserves historical keys, and resolves exact-locale aliases before fallback current routes; transport, tombstone disclosure and SEO remain follow-up scope.
- FORUM-24N exposes visibility-safe category route decisions through additive GraphQL and native storefront transports, rechecks the canonical category through the exact audience owner, and leaves host mounting, links, HTTP status policy and SEO unchanged.
- FORUM-24O mounts the canonical localized category route in the shared Rust storefront, cuts category-card links over from UUID query selection, and applies private fail-closed `308`, `404` and `503` host policy without adding category tombstones or SEO composition.
- FORUM-24P rewrites Forum category/topic SEO canonical, alternate, bulk and sitemap routes through the route owners, recognizes localized public paths, keeps visibility rechecks exact, and composes the existing SEO head on both Rust storefront route mounts.
- FORUM-24Q cuts Forum Search navigation over to exact owner-projected localized category/topic routes, validates route envelopes centrally in Search, and leaves stale UUID route documents non-navigable until reindex without adding a compatibility fallback.
- FORUM-24R adds an executable PostgreSQL harness for the FORUM-24Q reindex handoff, exercising real owner writes, the durable Forum inbox, staged tenant replacement, canonical Search URL acceptance and cross-tenant isolation without changing runtime code.
- FORUM-24S adds executable SQLite evidence that the native category and topic route decisions resolve through registered Axum/Leptos server-function endpoints with trusted host and tenant context, owner-created aliases and fail-closed missing results.

## Attachment relation concurrency

Forum attachment relations use an independent monotonic attachment-set revision for
lost-update detection. `expected_relation_revision` is the CAS token: `0` represents
the absence of any committed relation set, while every committed replacement advances
the token. `source_revision` is only Forum content provenance and does not advance for
attachment-only changes. The immutable mention/quote relation revision stream is not
reused for attachment state.

Attachment persistence remains gated on a Media owner reference-retention/control
contract because Media lifecycle admission is a read fact and does not reserve an
asset against deletion.

## Verification

- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- `npm run verify:forum:admin-boundary`
- `npm run verify:forum:storefront-boundary`
- `node scripts/verify/verify-forum-topic-route-storefront-graphql.mjs`
- `node scripts/verify/verify-forum-topic-route-storefront-mount.mjs`
- `node scripts/verify/verify-forum-topic-route-tombstone-visibility-owner.mjs`
- `node scripts/verify/verify-forum-topic-route-authorized-gone.mjs`
- `node scripts/verify/verify-forum-category-route-identity-owner.mjs`
- `node scripts/verify/verify-forum-category-slug-alias-owner.mjs`
- `node scripts/verify/verify-forum-category-route-storefront-transport.mjs`
- `node scripts/verify/verify-forum-category-route-storefront-mount.mjs`
- `node scripts/verify/verify-forum-canonical-route-seo-policy.mjs`
- `node scripts/verify/verify-forum-search-canonical-route-cutover.mjs`
- `node scripts/verify/verify-forum-search-canonical-route-reindex-harness.mjs`
- `node scripts/verify/verify-forum-native-host-route-evidence.mjs`
- `node scripts/verify/verify-forum-reply-anchor-consumer.mjs`
- `node scripts/verify/verify-forum-storefront-cursor-paging.mjs`
- task-specific owner, transport, UI and runtime commands from the canonical plan

## Related documents

- [README crate](../README.md)
- [Canonical implementation plan](./implementation-plan.md)
- [Accepted Forum slug/locale decision](../../../../DECISIONS/2026-03-29-forum-slug-locale-contract.md)
- [Admin UI package](../admin/README.md)
- [Storefront UI package](../storefront/README.md)
- [Event flow contract](../../../../docs/architecture/event-flow-contract.md)

## Source owner notes

- [forum-12-postgres-runtime-proof.md](./forum-12-postgres-runtime-proof.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-16-postgres-runtime-proof.md](./forum-16-postgres-runtime-proof.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20ap-topic-descriptor-materialization.md](./forum-20ap-topic-descriptor-materialization.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20aq-category-topic-create-audience.md](./forum-20aq-category-topic-create-audience.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20av-reply-create-audience-enforcement.md](./forum-20av-reply-create-audience-enforcement.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20aw-reply-create-audience-transport-composition.md](./forum-20aw-reply-create-audience-transport-composition.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20ax-topic-reply-create-audience.md](./forum-20ax-topic-reply-create-audience.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20ay-moderation-audience-policy.md](./forum-20ay-moderation-audience-policy.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20az-moderation-audience-transport-composition.md](./forum-20az-moderation-audience-transport-composition.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20ba-audience-plan-sync.md](./forum-20ba-audience-plan-sync.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20bb-topic-audience-exact-read.md](./forum-20bb-topic-audience-exact-read.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20bc-topic-audience-transport-composition.md](./forum-20bc-topic-audience-transport-composition.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-20bm-search-rebuild-scope-preservation.md](./forum-20bm-search-rebuild-scope-preservation.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23a8-profile-service-mutation-boundary.md](./forum-23a8-profile-service-mutation-boundary.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23a9-profile-event-aware-mutation-api.md](./forum-23a9-profile-event-aware-mutation-api.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g1-search-durable-ingest-sequence.md](./forum-23b2g1-search-durable-ingest-sequence.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g1a-search-ingest-sequence-index.md](./forum-23b2g1a-search-ingest-sequence-index.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2a-search-owner-revision-ledger.md](./forum-23b2g2a-search-owner-revision-ledger.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2a1-search-owner-revision-counter-hardening.md](./forum-23b2g2a1-search-owner-revision-counter-hardening.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b1-search-owner-revision-source.md](./forum-23b2g2b1-search-owner-revision-source.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3b2-versioned-invalidation-publisher.md](./forum-23b2g2b3b2-versioned-invalidation-publisher.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3c-versioned-invalidation-consumer.md](./forum-23b2g2b3c-versioned-invalidation-consumer.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-24h-topic-route-storefront-graphql.md](./forum-24h-topic-route-storefront-graphql.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-24o-category-route-storefront-mount.md](./forum-24o-category-route-storefront-mount.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-24p-canonical-route-seo-policy.md](./forum-24p-canonical-route-seo-policy.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-24q-search-canonical-route-cutover.md](./forum-24q-search-canonical-route-cutover.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-26a-user-trust-state.md](./forum-26a-user-trust-state.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-26c-posting-policy-contract.md](./forum-26c-posting-policy-contract.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-26d-posting-policy-evaluator.md](./forum-26d-posting-policy-evaluator.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-26e-posting-policy-facts.md](./forum-26e-posting-policy-facts.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-26g-topic-reading-posting-facts.md](./forum-26g-topic-reading-posting-facts.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-26h-approved-posts-posting-facts.md](./forum-26h-approved-posts-posting-facts.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-26i-approved-posts-index-hardening.md](./forum-26i-approved-posts-index-hardening.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-26j-create-window-posting-facts.md](./forum-26j-create-window-posting-facts.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- # FORUM-21T reply-range move GraphQL transport
- `source_ready_maintainer_execution_pending`
- ForumReplyRangeMoveService::move_reply_range
- immutable owner receipt
- FORUM-21` entry remains `planned`
- No command above was run by the implementation agent
- forum.topic.reply_range_moved
- # FORUM-21I/J canonical merged-topic resolution and HTTP redirect
- One source of truth
- at most 32 edges
- FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT
- 308 Permanent Redirect
- Cache-Control: private, no-store
- FORUM-21A through FORUM-21X
- FORUM-21J
- the only source-to-target canonical edge
- Canonical merged-topic resolution and HTTP redirect
- Location: /api/forum/topics/{canonical_topic_id}
- forum_permission_denied
- [FORUM-21I/J canonical resolution and HTTP redirect]
- # FORUM-24E historical topic-merge route backfill
- [FORUM-24E historical merge route backfill](./forum-24e-topic-merge-route-backfill.md)
- # FORUM-24G topic slug rename admin UI
- `renameForumTopicSlug`
- `forum_topics:update`
- `/modules/forum/rename-slug`
- `/dashboard/forum/rename-slug`
- # FORUM-21R topic split GraphQL transport
- ForumTopicSplitService::split_selected_replies
- forum.topic.split
- [forum-21a-topic-move-owner.md](./forum-21a-topic-move-owner.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21b-topic-merge-owner.md](./forum-21b-topic-merge-owner.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21c-topic-merge-subscription-reconciliation.md](./forum-21c-topic-merge-subscription-reconciliation.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21i-topic-canonical-resolution.md](./forum-21i-topic-canonical-resolution.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21m-topic-merge-cross-category.md](./forum-21m-topic-merge-cross-category.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21n-topic-merge-admin-ui.md](./forum-21n-topic-merge-admin-ui.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21o-topic-merge-native-admin.md](./forum-21o-topic-merge-native-admin.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21p-topic-split-owner.md](./forum-21p-topic-split-owner.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21r-topic-split-graphql-transport.md](./forum-21r-topic-split-graphql-transport.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21s-reply-range-move-owner.md](./forum-21s-reply-range-move-owner.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21t-reply-range-move-graphql-transport.md](./forum-21t-reply-range-move-graphql-transport.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d10-normal-delivery-proof.md](./forum-23b2g2b3d10-normal-delivery-proof.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d11-aggregate-evidence-assembler.md](./forum-23b2g2b3d11-aggregate-evidence-assembler.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d12-retained-evidence-promotion.md](./forum-23b2g2b3d12-retained-evidence-promotion.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d13-link-forum-03-evidence-assembler.md](./forum-23b2g2b3d13-link-forum-03-evidence-assembler.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d17-complete-link-evidence-assembler.md](./forum-23b2g2b3d17-complete-link-evidence-assembler.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d18-complete-link-evidence-promotion.md](./forum-23b2g2b3d18-complete-link-evidence-promotion.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d19-link-forum-03-canonical-promotion-packet.md](./forum-23b2g2b3d19-link-forum-03-canonical-promotion-packet.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d2-postgres-ingress-proof.md](./forum-23b2g2b3d2-postgres-ingress-proof.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d3-ack-restart-proof.md](./forum-23b2g2b3d3-ack-restart-proof.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d4-raw-poison-proof.md](./forum-23b2g2b3d4-raw-poison-proof.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-23b2g2b3d5-semantic-poison-proof.md](./forum-23b2g2b3d5-semantic-poison-proof.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-24b-topic-merge-route-aliases.md](./forum-24b-topic-merge-route-aliases.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-24g-topic-slug-rename-admin-ui.md](./forum-24g-topic-slug-rename-admin-ui.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-moderation-effect-postgres-contract.md](./forum-moderation-effect-postgres-contract.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-moderation-revision-concurrency-contract.md](./forum-moderation-revision-concurrency-contract.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-moderation-terminal-concurrency-contract.md](./forum-moderation-terminal-concurrency-contract.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- publish_contract_in_tx_with_causation<C, E>
- MAX_FORUM_TOPIC_FORK_REPLIES: usize = 500
- # FORUM-21H topic merge accepted-solution policy
- FORUM-21L
- ## Explicit competing-solution resolution
- ## Append-only resolution audit
- schema version 1
- # FORUM-21L competing accepted-solution resolution
- merge_topic_resolving_solution
- forum_topic_merge_solution_resolutions
- Resolution audit ledger
- forum.topic.merged / schema version 1
- FORUM_TOPIC_MERGE_OPERATION_CONFLICT
- [FORUM-21L competing solution resolution]
- pub const FORUM_TOPIC_ROUTE_SHORT_ID_LEN: usize = 12
- # FORUM-24F topic slug rename GraphQL transport
- TopicService::rename_slug
- # FORUM-24D topic slug rename owner
- Topic slug changed
- [forum-21h-topic-merge-solution-policy.md](./forum-21h-topic-merge-solution-policy.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-21l-topic-merge-solution-resolution.md](./forum-21l-topic-merge-solution-resolution.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-24a-topic-route-identity-owner.md](./forum-24a-topic-route-identity-owner.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-24d-topic-slug-rename-owner.md](./forum-24d-topic-slug-rename-owner.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
- [forum-24f-topic-slug-rename-graphql-transport.md](./forum-24f-topic-slug-rename-graphql-transport.md) — source-ready owner note; runtime execution and admission remain maintainer-owned.
