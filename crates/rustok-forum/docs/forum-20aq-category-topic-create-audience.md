# FORUM-20AQ category topic-create audience policy

**Status:** source-ready / unvalidated

`FORUM-20AQ` adds a Forum-owned, normalized category policy for deciding who may
create topics. It is intentionally separate from category/topic visibility. A
category may therefore remain readable while topic creation is narrowed for that
category subtree.

Each category contributes at most one non-empty local topic-create audience layer.
Effective policy is the ordered root-to-category conjunction of every configured
layer. Within one layer, positive role, trust, Channel membership, Groups
membership, and explicit-user selectors remain a union, while explicit deny wins.
A descendant can narrow an ancestor rule but cannot replace or broaden it.

The owner exposes managed `get` and atomic replacement `set` commands under
`forum_categories:manage`. Replacement takes the tenant category-tree lock,
normalizes raw input before persistence, deletes the prior local layer, and inserts
the policy plus typed role/channel/group/user relations in one transaction. An
empty constraint set removes only the local layer and restores inheritance.

PostgreSQL and SQLite enforce tenant/category composite foreign keys, typed role,
trust, channel, group and user checks, immutable stored rows, and bounded direct
channel/group/allow/deny inserts. The new tables do not reuse or mutate the existing
visibility audience tables.

This slice does **not** change `TopicService::create`, REST, GraphQL, storefront,
notification, or facts-provider behavior. Command-time topic-create authorization
requires an exact caller `PortContext` and optional owner facts composition and is a
separate follow-up. Reply and moderation audience policies also remain open.

Source evidence is
`tests/category_topic_create_audience_policy_sqlite.rs`; the static guard is
`scripts/verify/verify-forum-category-topic-create-audience-policy.mjs`.
Tests, Cargo commands, formatting, and verifiers were not run by the implementation
agent.
