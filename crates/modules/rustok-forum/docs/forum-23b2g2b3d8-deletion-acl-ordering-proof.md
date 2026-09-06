# FORUM-23B2G2B3D8 deletion and ACL ordering proof

## Status

`source_ready_maintainer_execution_pending`

This slice adds the `deletion_acl_ordering` subset of the frozen
`FORUM-23B2G2B3D` runtime-evidence matrix. It combines real Forum owner
transactions, the versioned typed invalidation ingress, legacy Forum status
roots, the production Search reconciler, the production Forum projection
source, PostgreSQL Search execution and current Forum storefront eligibility.

The machine-readable proof contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-deletion-acl-ordering-proof.json
```

The executable test is:

```text
apps/server/tests/forum_versioned_invalidation_deletion_acl_ordering.rs
```

Successful execution writes:

```text
target/forum-search-versioned-invalidation-deletion-acl-ordering-evidence.json
```

The artifact is generated only after the proof and isolated PostgreSQL schema
cleanup succeed. It records the exact Git source commit and must not be
hand-edited.

## Initial visible owner state

The test applies real Outbox, Taxonomy, Forum and Search migrations in one
isolated PostgreSQL schema. Real Forum services create one moderated public
category and three independent public targets:

1. an approved reply that contains `d8hiddenreplymarker`;
2. a topic that contains `d8deletedtopicmarker`;
3. a topic that contains `d8acltopicmarker`.

A real approved-reply legacy root drives the first production Forum rebuild.
Each marker is then queried through `execute_forum_storefront_search`. The
production PostgreSQL engine, exact category scope and real Forum public result
eligibility must return exactly the corresponding owner object before the
negative mutations.

## Real owner mutations and revisions

The test records the owner revision head and then performs three real commands:

- `ModerationService::hide_reply` for the approved reply;
- `TopicService::delete` for the second topic;
- `ForumTopicAudiencePolicyService::set` with `roles_any = [Customer]` for the
  third topic.

The expected contiguous post-baseline ledger shape is:

```text
revision N+1: forum_category / category ID       (reply stopped being public)
revision N+2: forum_topic / deleted topic ID     (topic lifecycle)
revision N+3: forum_category / category ID       (category counters)
revision N+4: forum_topic / ACL topic ID         (richer audience policy)
```

The reply-hide path deliberately has two compatible event channels. Its legacy
`forum.reply.status_changed` event requires a full tenant Forum rebuild, while
its versioned owner revision targets the affected category counter. D8 retains
and delivers both; it does not falsely claim that a category-only refresh can
remove a reply document by itself.

Topic delete likewise retains the legacy `forum.topic.status_changed` root plus
its versioned topic and category owner revisions.

## Out-of-order and duplicate delivery

The test admits the post-mutation roots in this deliberate order:

```text
owner revision N+4
owner revision N+3
owner revision N+2
legacy topic-delete status root
legacy reply-hide status root
owner revision N+1
```

The typed owner revisions are therefore reverse ordered, and the oldest
category-only revision is processed last, after current-state full rebuilds.
The oldest typed root and the legacy hide root are each admitted a second time.

The durable Search inbox must contain exactly six new rows in the stated ingest
order and exactly one row for every repeated root event UUID. Forum owner
revision numbers are not compared with Search `ingest_sequence`.

## Current-state convergence

All owner mutations commit before delivery begins. Consequently every full
Forum rebuild reads current owner state rather than replaying historical state.
The production projection source excludes:

- the hidden reply because its current status is not `approved`;
- the deleted topic because its current owner lifecycle is archived/deleted;
- the richer-policy topic because anonymous public discovery does not satisfy
  the required Customer role.

After the full rebuilds, the final stale category refresh may update the public
category but cannot restore any topic or reply. The retained Search projection
must contain none of the three denied document IDs.

## Storefront owner reauthorization

The test then inserts three intentionally stale `search_documents` rows with
apparently visible Search status:

```text
hidden reply:     forum_reply / approved
deleted topic:    forum_topic / open
ACL-denied topic: forum_topic / open
```

Each row contains its unique marker and the correct category facet, so it is a
real raw PostgreSQL Search candidate. This injection models projection lag or a
stale external writer; it is not presented as a supported owner mutation.

The same production storefront execution path must send each exact candidate
to `ForumSearchResultEligibilityService`. Current Forum owner state rejects all
three before visible totals, items, facets, offset or limit are finalized. Each
marker query must return:

```text
total: 0
items: []
visible facet buckets: 0
```

This is the second barrier: even deliberately stale Search rows cannot become a
storefront visibility oracle or restore denied content.

## Runtime placement

D8 lives in the `rustok-server` integration-test package because it composes
Outbox, Taxonomy, Forum and Search. It uses the dependencies already owned by
the host package and adds no `Cargo.toml` or `Cargo.lock` edge.

Every pooled PostgreSQL connection receives the isolated schema through the
connection URL `search_path` option. This is required because the Search
projector opens nested transactions while Forum projection reads and storefront
queries may use separate pool sessions.

No broker is needed for this bounded delivery-order proof. Typed envelopes are
passed through the production `ForumSearchContractIngress`; legacy roots use
the same durable Search inbox schema and root identity contract.

## Deliberate boundary

D8 does not run the long-lived host worker loop, restart scheduling or Iggy
acknowledgement/poison/DLQ behavior. It does not prove the Search-disabled
profile, aggregate `FORUM-23B2G2B3D` completion or `LINK-FORUM-03` closure.
Those remain separate gates under D0.

## Maintainer verification

```bash
node scripts/verify/verify-forum-search-versioned-invalidation-deletion-acl-ordering-proof.mjs
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-server \
  --test forum_versioned_invalidation_deletion_acl_ordering \
  -- --nocapture --test-threads=1
```

No command above was run by the implementation agent.
