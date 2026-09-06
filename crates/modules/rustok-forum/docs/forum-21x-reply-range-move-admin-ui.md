# FORUM-21X reply-range move admin composition

## Status

`source_ready_maintainer_execution_pending`

FORUM-21X composes the existing FORUM-21T manager GraphQL command in the
module-owned Leptos and Next-admin surfaces. It adds no owner method, migration,
receipt, semantic event, REST route or native transport.

Machine contract:

```text
crates/rustok-forum/contracts/forum-reply-range-move-admin-ui.json
```

## Routes and authority

The module-owned routes are:

- Leptos: `/modules/forum/reply-range`;
- Next-admin: `/dashboard/forum/reply-range`.

Both paths require the existing admin/module admission boundary and call
`moveForumTopicReplyRange`. Routed tenant and authenticated actor authority
remain server-owned; the UI does not place actor IDs or permission snapshots in
the command.

## Command and retry identity

The form selects an existing source and target topic, accepts positive inclusive
`startPosition` and `endPosition`, and requires a bounded reason. One operation
UUID remains stable for an exact retry. Editing source, target, either endpoint
or reason rotates the operation UUID and clears the previous receipt.

The target is an existing topic, so this workflow does not allocate a target
topic UUID.

## Position boundary

The ordinary `forumReplies` read contract does not expose canonical owner
positions, and historical move/split operations may leave sparse positions.
The admin therefore never derives movement endpoints from visible row order.

Managers enter exact owner positions. The UI performs only scalar validation:
positive integers and `start <= end`. The owner remains authoritative for:

- occupied endpoints and sparse ranges;
- the 500-reply bound and source non-emptiness;
- asymmetric parent-edge policy;
- deterministic target append positions;
- effective ACL and reply-create equality;
- accepted-solution conflicts;
- topic/category counters, audit, events and replay conflicts.

## Receipt

Both surfaces display the immutable owner receipt, including source and target
ranges, moved total and published counts, operation/event IDs and resulting
owner state. Neither surface reads operation or item audit tables.

## Compatibility

FORUM-21X is admin composition only. Existing direct owner callers and the
FORUM-21T GraphQL schema remain unchanged. No transport fallback is introduced.
Canonical localized URL aliases remain owned by FORUM-24.

## Maintainer verification

```bash
node scripts/verify/verify-forum-reply-range-move-owner.mjs
node scripts/verify/verify-forum-reply-range-move-graphql-transport.mjs
node scripts/verify/verify-forum-reply-range-move-admin-ui.mjs
cargo test -p rustok-forum-admin topic_reply_range_model -- --nocapture
cargo check -p rustok-forum-admin --all-targets
npm --prefix apps/next-admin run typecheck
npm run verify:forum:admin-boundary
npm run verify:blog:forum-ui-ownership
```

No command above was run by the implementation agent, per maintainer request.
