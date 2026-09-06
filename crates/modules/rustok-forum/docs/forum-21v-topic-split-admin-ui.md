# FORUM-21V topic split admin composition

## Status

`source_ready_maintainer_execution_pending`

FORUM-21V composes the existing FORUM-21P selected-reply split owner and FORUM-21R manager GraphQL command in both public Forum admin surfaces. It adds no new movement, access, solution, counter or idempotency policy.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-split-admin-ui.json
```

## Leptos admin

The module-owned Leptos route subpath `split` renders `ForumTopicSplitAdmin`. It loads active topic candidates and up to 500 replies through a dedicated GraphQL adapter, lets a manager select one source and one reply set, collects target locale/title/optional slug and reason, then calls `splitForumTopicReplies`.

The form owns two generated UUIDs:

- the split operation ID;
- the new target-topic ID.

An unchanged retry retains both UUIDs. Any edit to the source topic, selected reply IDs, target locale/title/slug or reason rotates both and clears a stale receipt. A failed transport attempt does not rotate either identity.

## Next admin

The module package exports `ForumTopicSplit`, the shared command model and the GraphQL API functions. The application route `/dashboard/forum/split` only resolves authenticated tenant context, loads topic candidates and mounts the module component. Navigation requires `forum_topics:manage`.

The client loads source replies after topic selection. It displays reply preview, status, identity and whether the reply has a parent. The same operation/target identity lifecycle is used as in Leptos.

## Selection boundary

The owner remains authoritative for the complete parent-closed rule and source-nonempty invariant. Both UI models provide early feedback for the reply graph visible to the form:

- a selected loaded child requires its loaded parent;
- a selected loaded parent requires every loaded child;
- at least one reply must remain in the source;
- no more than 500 unique UUIDs may be submitted.

The UI does not assign reply positions, discover hidden descendants, move rows, copy bodies or relations, reconcile access, move solutions or update counters. Those remain atomic owner responsibilities.

## Receipt and compatibility

After success both surfaces display the immutable owner receipt, including operation and event IDs, source and target identities, moved counts and target published count. Exact replay remains an owner replay and does not create a second UI idempotency record.

FORUM-21V adds no migration, owner method, GraphQL field, REST route, native split transport, receipt change or semantic-event change. Existing headless callers of `splitForumTopicReplies` remain unchanged.

## Remaining scope

FORUM-21 remains `planned`. Remaining work includes maintainer SQLite/PostgreSQL execution evidence, mounted-browser evidence, admin composition for reply-branch fork and bounded reply-range movement, and final localized aliases/tombstones under FORUM-24.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-split-owner.mjs
node scripts/verify/verify-forum-topic-split-graphql-transport.mjs
node scripts/verify/verify-forum-topic-split-admin-ui.mjs
cargo test -p rustok-forum-admin topic_split_model -- --nocapture
cargo check -p rustok-forum-admin --all-targets
npm --prefix apps/next-admin run typecheck
npm run verify:forum:admin-boundary
```

No command above was run by the implementation agent, per maintainer request.
