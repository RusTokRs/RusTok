---
id: doc://crates/rustok-forum/docs/forum-12d2-inline-quote-commands.md
kind: implementation_record
language: en
status: delivered
owners:
  - rustok-forum
  - rustok-notifications-program
last_reviewed: 2026-07-22
---

# FORUM-12D2 inline quote owner commands

FORUM-12D2 composes typed quote references into topic and reply create/body-edit
owner transactions. It keeps the D1 exact-locale replacement command and does
not expose relation persistence to REST or GraphQL.

## Compatibility contract

The existing Rust `CreateTopicInput`, `UpdateTopicInput`, `CreateReplyInput` and
`UpdateReplyInput` structs remain unchanged. In other words, legacy Rust DTOs
remain unchanged. Separate command DTOs carry inline quote input, while the
existing facade methods convert legacy inputs into those commands.

Create commands treat an omitted quote list as an empty initial set. Update
commands use three distinct states:

- omitted updates preserve the latest exact-locale quote set;
- an explicit empty list clears quotes;
- an explicit non-empty list replaces the complete set.

Legacy Rust body updates route through the same command facade, so they preserve
existing quotes instead of silently clearing them.

## Transaction and concurrency boundary

Mention extraction and quote validation happen before the owner transaction.
When an update preserves quotes, the owner records the relation revision used to
build the prepared projection. Inside the transaction it locks the active topic
or reply source and compares the latest exact-locale relation revision before
writing the body.

A concurrent D1 or D2 relation replacement produces retryable
`FORUM_RELATION_REVISION_CONFLICT`. The body update does not restore a stale quote
set. Explicit replacement uses the caller-supplied set and proceeds under the
same source lock. Canonical body persistence, immutable relation persistence,
mention events, outbox rows, owner journal rows and existing counters/events
commit or roll back together.

Soft-deleted sources reject inline relation updates. Quote references remain
bounded to 32 raw entries, exact duplicates are normalized, and quoted revision
identity is never inferred from display text.

## Transport

REST uses the existing content routes with command DTOs:

- `POST /api/forum/topics`;
- `PUT /api/forum/topics/{id}`;
- `POST /api/forum/topics/{id}/replies`;
- `PUT /api/forum/replies/{id}`.

GraphQL keeps the legacy mutations and adds non-breaking D2 command mutations:

- `createForumTopicWithQuotes`;
- `updateForumTopicWithQuotes`;
- `createForumReplyWithQuotes`;
- `updateForumReplyWithQuotes`.

The older GraphQL topic update remains safe because its legacy facade conversion
uses omitted-update preservation. REST, GraphQL and OpenAPI call only public
owner facades and never import `MentionRelationService`, prepared relation types
or transaction persistence helpers.

## Verification

```bash
node scripts/verify/verify-forum-quote-commands.mjs
cargo test -p rustok-forum inline_quote
cargo test -p rustok-forum mention_relation
cargo xtask module validate forum
```

The SQLite source scenario covers preserved snapshot loading, a concurrent D1
replacement, typed CAS conflict and explicit clear semantics. PostgreSQL
concurrency and notifications-off/on runtime evidence remain open.

Maintainer verification was not executed while publishing this slice.
