# forum 21c topic merge subscription reconciliation

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-topic-merge-subscription-reconciliation.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 21c topic merge subscription reconciliation`.
- Verifier: `scripts/verify/verify-forum-topic-merge-subscription-reconciliation.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- marker: # FORUM-21C topic merge subscription reconciliation
- marker: `source_ready_maintainer_execution_pending`
- marker: crates/modules/rustok-forum/contracts/forum-topic-merge-subscription-reconciliation.json
- marker: target subscription is always authoritative
- marker: At most 500 source topic subscription rows
- marker: topic row FOR SHARE
- marker: BEFORE INSERT OR UPDATE OR DELETE
- marker: revision from 7 to 8
- marker: forum.subscription.changed.v1
- marker: 4 = 1 moved + 2 equal + 1 conflict
- marker: automatic source/target topic-author subscriptions
- marker: FORUM_TOPIC_MERGE_SUBSCRIPTION_RECONCILIATION_CONFLICT
- marker: No command above was run by the implementation agent
