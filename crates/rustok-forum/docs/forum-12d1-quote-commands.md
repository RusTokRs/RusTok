---
id: doc://crates/rustok-forum/docs/forum-12d1-quote-commands.md
kind: implementation_record
language: en
status: delivered
owners:
  - rustok-forum
  - rustok-notifications-program
last_reviewed: 2026-07-22
---

# FORUM-12D1 quote owner commands

FORUM-12D1 exposes a bounded owner command for replacing quote relations on an
existing topic translation or reply body. It does not make the relation
persistence seam public and does not call Notifications.

## Command semantics

- `SetForumQuotesInput` carries an exact source locale and at most 32 unique
  typed quote references.
- The submitted list is a full replacement for that source revision projection.
- An empty list clears all quote relations while preserving mentions extracted
  from the unchanged canonical body.
- Duplicate quote references are normalized deterministically before the bound
  is enforced.
- Topic and reply targets require their corresponding update owner scope.
- Missing, foreign-tenant, kind-mismatched or target-mismatched revisions fail
  with the existing safe `FORUM_QUOTE_TARGET_UNAVAILABLE` class.

## Transaction boundary

The owner service loads the exact locale body, prepares mention and quote
relations before opening the transaction, then locks and re-reads the source
through `MentionRelationService::persist_in_tx`. The new immutable relation
revision, mention targets, quote targets, mention events, transactional outbox
rows and Forum event journal rows commit or roll back together.

An identical replacement replays the current relation revision. Unchanged or
removed mentions do not produce mention events.

## Transport

REST:

- `PUT /api/forum/topics/{id}/quotes`
- `PUT /api/forum/replies/{id}/quotes`

GraphQL:

- `setForumTopicQuotes`
- `setForumReplyQuotes`

REST, GraphQL and OpenAPI consume `ForumQuoteCommandService`; they never import
`MentionRelationService`, `PreparedMentionRelations` or `persist_in_tx`.

## Compatibility and remaining scope

Existing topic/reply create and body-edit DTOs are unchanged. Inline quote input
for source create/body edit remains FORUM-12D2 so that those commands can adopt
the relation input without breaking current Rust callers. Notifications fan-out,
source/target opening visibility rechecks and PostgreSQL concurrency/runtime
evidence remain separate work.

## Verification

```bash
node scripts/verify/verify-forum-quote-commands.mjs
cargo test -p rustok-forum quote_command
cargo xtask module validate forum
```

Maintainer verification was not executed while publishing this slice.
