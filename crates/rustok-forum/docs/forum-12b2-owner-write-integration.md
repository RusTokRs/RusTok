---
id: doc://crates/rustok-forum/docs/forum-12b2-owner-write-integration.md
kind: implementation_record
language: en
status: delivered
owners:
  - rustok-forum
  - rustok-notifications-program
last_reviewed: 2026-07-22
---

# FORUM-12B2 owner write integration

FORUM-12B2 composes the crate-private mention and quote projection seam into the
active topic and reply owner commands. It does not expose that seam to REST,
GraphQL or another module.

## Transaction boundary

For topic and reply create/edit commands the owner path now follows this order:

1. authorize and canonicalize the source body;
2. resolve mention handles through `ProfilesReader` and prepare the projection
   before opening the owner transaction;
3. write the canonical topic translation or reply body;
4. call `MentionRelationService::persist_in_tx` in the same transaction;
5. run the existing counters and semantic event writes;
6. commit.

`persist_in_tx` re-reads the just-written source body and fails closed if it no
longer matches the prepared fingerprint. A failure rolls back the source body,
relation revision, counters and existing event writes together.

## Owner routing

The public `TopicService` and `ReplyService` facades continue to route through
module-owned owner services. Raw compatibility services remain crate-private.
The small `topic_relation_integration.rs` and `reply_relation_integration.rs`
extensions share their raw module scope only to reuse existing private write
helpers without widening the crate API.

## Compatibility

Create and edit commands project user and typed audience mentions from canonical
Markdown or `rt_json_v1`. Existing source INSERT seed triggers may create the
rollout `legacy` identity first; the active projection is appended in the same
transaction immediately afterward.

Quote command DTOs are intentionally unchanged in this slice, so active commands
pass an empty quote set. Versioned mention events, outbox publication, bounded
owner reads and Notifications delivery remain FORUM-12C / NOTIFY scope.

## Guardrail

`contracts/forum-mention-write-boundary.json` is the machine-readable boundary.
`scripts/verify/verify-forum-mention-integration.mjs` verifies owner delegation,
source-write ordering, same-transaction persistence and transport isolation.

Maintainer verification was not executed while publishing this slice.
