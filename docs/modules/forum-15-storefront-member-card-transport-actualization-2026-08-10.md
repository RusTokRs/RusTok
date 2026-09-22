# FORUM-15D storefront member-card transport actualization — 2026-08-10

Status: `source-ready / dual-path-transport / authenticated-stats / anonymous-content-preserved / ui-presentation-open / maintainer-execution-open`

## Fresh cursor

This slice started from `main@2e630fc26db867b1a0febbace588839d7d1adbf3`, the FORUM-15C merge.

FORUM-15 remains `in_progress`. FORUM-15C moved privacy-aware Profiles + Forum-stat composition into the shared `ForumMemberCardService`, but the real Forum storefront still had no author identity or member-card transport payload.

The storefront has two real read paths:

- GraphQL for headless/CSR;
- a native server adapter for SSR/hydrate.

This slice carries the same bounded member-card enrichment through both paths without adding a direct `rustok-profiles` dependency to `rustok-forum-storefront`.

## Permission boundary preserved

FORUM-15B deliberately established Forum member-card statistics as authenticated `forum_topics:read` data, matching the existing single-user Forum statistics admission surface.

During this slice a possible public `forumStorefrontMemberCards(userIds, locale)` field was reviewed and rejected before PR finalization. Such a field would have accepted arbitrary user IDs and widened Forum topic/reply/solution counters to anonymous callers.

The final source does **not** add a public member-card statistics GraphQL field.

Instead:

- the GraphQL storefront adapter reuses the existing authenticated `forumMemberCards(userIds, locale)` contract;
- anonymous or permission-denied enrichment is treated as unavailable and yields an empty member-card vector while Forum content remains readable;
- the native storefront adapter invokes `ForumMemberCardService` only when the request principal has `Permission::FORUM_TOPICS_READ`;
- principals without that permission, including anonymous requests, receive the ordinary Forum content snapshot with an empty member-card vector.

The content visibility/read contracts are unchanged by enrichment admission.

## Author identity transport

The storefront transport models now carry optional `authorId` for:

- topic-list items;
- the selected topic;
- replies.

The fields use serde defaults so older/public payloads that omit author identity remain readable.

The GraphQL storefront queries request the existing Forum-owned `authorId` fields. The native adapter maps the same IDs from the Forum owner DTOs. No profile source data is copied into Forum persistence.

The personalized unread GraphQL DTO (`GqlForumStorefrontUnreadTopic`) previously omitted author identity even though its underlying `ForumStorefrontUnreadTopic.topic` already carries it. FORUM-15D adds only `author_id: Option<Uuid>` plus the direct mapper assignment. Its existing authenticated `forum_topics:list` admission, unread semantics and bulk-read mutations remain unchanged.

## Bounded member-card payload

`StorefrontForumData` now contains a default-empty `member_cards` vector with local transport DTOs for:

- user ID;
- handle;
- display name;
- tags;
- avatar Media ID;
- preferred locale;
- Forum topic/reply/solution counts.

The local storefront models intentionally do not expose the Profiles visibility enum. Profiles presentation has already made the visibility/block decision before a member card is emitted.

The Leptos UI explicitly ignores `member_cards` in this slice. That one-line destructuring change only keeps the new transport payload compile-shaped; visual author/member-card presentation remains the next slice.

## GraphQL path

The GraphQL storefront adapter:

1. reads topics, selected topic and replies through the existing storefront contracts;
2. requests `authorId` on those payloads;
3. deduplicates author IDs across the whole storefront snapshot while preserving first occurrence;
4. skips enrichment when no author IDs exist;
5. otherwise performs at most one `forumMemberCards` request for the snapshot;
6. degrades authentication/permission failure to an empty member-card vector so anonymous Forum content remains available.

No per-topic or per-reply member-card request is introduced.

The slice intentionally does not migrate the existing legacy storefront reply query to the richer reply-audience field; that would be a separate read-surface cutover and is outside FORUM-15D.

## Native SSR/hydrate path

The native adapter uses the same Forum owner service directly and does not self-call GraphQL.

It:

1. maps the same topic/reply author IDs into storefront transport DTOs;
2. checks `Permission::FORUM_TOPICS_READ` before member-card enrichment;
3. deduplicates the author set for the snapshot;
4. calls `ForumMemberCardService::read_for_audience` once when enrichment is admitted;
5. maps human principals to `ForumMemberCardAudience::Authenticated { actor_id }`;
6. maps OAuth service principals to `TrustedService { actor_id: None }`, matching the host Profiles audience policy;
7. maps owner member cards into storefront-local DTOs without naming Profiles types in the storefront package.

The owner service remains responsible for Profiles privacy/block admission before its single Forum statistics query.

## No-N+1 source shape

The storefront currently bounds the visible composition to 20 topics, one selected topic and 20 replies, so there are at most 41 author slots before deduplication, below `MAX_FORUM_MEMBER_CARD_USER_IDS = 100`.

For one storefront snapshot:

- author IDs are deduplicated once;
- GraphQL performs at most one member-card batch request;
- native SSR performs at most one `ForumMemberCardService` call;
- the owner service performs one bounded Profiles presentation batch and one Forum statistics query;
- no per-author member-card/stat/profile loop is added.

Retained query-count/runtime evidence is still open and is not claimed by this source slice.

## Remaining FORUM-15 work

FORUM-15 is not complete. The next bounded slice can consume `StorefrontForumData.member_cards` in the Leptos storefront and compose author/member-card presentation for topic feed, selected topic and replies without adding new owner reads.

Still open:

- visual member-card/author presentation on intended Forum surfaces;
- retained browser/runtime privacy and block evidence;
- retained query-count evidence for the dual-path bounded composition;
- any later permitted Forum trust/activity enrichment that preserves external ownership.

The canonical FORUM-15 ledger remains materially correct and stays `in_progress`.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database scenario, migration, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-storefront-member-card-transport-source.mjs
```
