# FORUM-15B member-card statistics actualization — 2026-08-10

Status: `source-ready / bounded-member-card-read / profile-privacy-authoritative / runtime-evidence-open`

## Fresh cursor

This slice started from `main@d3781ddbabc3a4122324b991ddb2a31eca0a80cb` after FORUM-15A. The intervening main movement changed Forum/Page Builder rollout evidence and related verification sources, not Forum GraphQL profile composition or Forum user-stat ownership.

The canonical FORUM-15 ledger remains `in_progress`: this slice adds a bounded source contract for member-card composition, while retained runtime/no-N+1 evidence and product integration into the complete Forum member-card experience remain open.

## Owner boundaries

Profiles remains authoritative for public identity and presentation privacy. Forum does not copy handle, display name, avatar or privacy/block state and does not read Profiles/Social Graph private tables.

Forum remains authoritative only for Forum-local statistics. The member-card read composes:

- Profiles-owned `GqlProfileSummary` admitted through `ProfileSummaryLoader` / `ProfilePresentationService`;
- Forum-owned topic/reply/solution counters from `forum_user_stats`.

The composition order is security-significant: Profiles presentation admission happens first, and Forum statistics are loaded only for the identities returned by that owner. A Forum statistics row can never make a hidden or blocked profile visible.

## New bounded GraphQL contract

`forumMemberCards(userIds, locale)` is mounted on `ForumQuery` through `ForumMemberCardQuery`.

The contract:

1. requires the Forum module and authenticated `forum_topics:read`, matching the existing single-user Forum stats admission surface;
2. accepts at most `MAX_FORUM_MEMBER_CARD_USER_IDS = 100` requested IDs;
3. rejects nil IDs and deduplicates repeated IDs while preserving first-request order;
4. resolves locale from the request/tenant context;
5. loads Profiles presentation in one bounded batch;
6. loads Forum statistics in one `forum_user_stats` query for visible profile IDs only;
7. zero-fills missing Forum statistics rows without manufacturing a profile;
8. returns cards in first-request order for visible identities only.

The response shape is:

```text
GqlForumMemberCard {
  user_id
  profile: GqlProfileSummary
  forum_stats: GqlForumMemberStats {
    topic_count
    reply_count
    solution_count
  }
}
```

## Profiles presentation behavior

The preferred host path is the request-scoped `DataLoader<ProfileSummaryLoader>`. The host binds that loader to the request audience, so authenticated/block/privacy behavior stays owned by Profiles.

As established in FORUM-15A, if the loader is absent the query falls back to `ProfilePresentationService::new`, which is anonymous/fail-closed rather than raw `ProfileService`.

The new query does not invoke `ProfilePrivacyService` directly and does not reproduce its matrix.

## No-N+1 source shape

For a request of up to 100 IDs:

- input IDs are deduplicated before any owner read;
- one loader `load_many` call or one fallback `find_profile_summaries` call handles Profiles presentation;
- one SeaORM query loads all matching Forum user-stat rows;
- there is no per-user `UserStatsService::get` loop;
- there is no per-user Profiles read loop.

This establishes a bounded no-N+1 source shape. Retained database/query-count/runtime evidence is still maintainer work and is not claimed here.

## Compatibility

Existing `authorProfile` fields on Forum topic/reply GraphQL types are unchanged. This slice adds a separate batch member-card read instead of changing established topic/reply response shapes.

That keeps existing consumers compatible and gives authenticated Forum member-card consumers an explicit bounded enrichment contract.

## Remaining FORUM-15 work

FORUM-15 remains `in_progress`. Remaining work includes:

- integrating the batch member-card contract into the intended Forum member-card UI surfaces;
- retained runtime/query-count evidence proving the bounded source shape under real host composition;
- browser/runtime evidence for privacy/block behavior and locale presentation;
- any additional permitted Forum activity/reputation composition required by later tasks without moving ownership into Forum.

Because those broader deliverables remain open, the canonical ledger wording is still materially true and is not rewritten merely to mark this source slice.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database scenario, migration, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-member-card-stats-source.mjs
```
