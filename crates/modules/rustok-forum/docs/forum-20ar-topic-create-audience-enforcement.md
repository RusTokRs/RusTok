# FORUM-20AR topic-create audience enforcement

**Status:** source-ready / unvalidated

`FORUM-20AR` moves the normalized category topic-create audience policy from
managed persistence into the Forum topic owner command path. Authorization runs
before topic, translation, relation, counter, user-stat, or domain-event writes.

The owner first enforces `forum_topics:create`, then loads the bounded inherited
root-to-category topic-create policy. Every configured layer must allow the caller.
Within a layer, explicit deny wins; explicit allow and matching role are resolved
locally. Categories with no configured layer retain their historical create
behavior.

Trust, Channel membership, and Groups membership facts are requested only when a
layer remains undecided after local checks. Such a request requires the exact
caller `PortContext`, including matching tenant and user identity, a read policy,
deadline, locale, and correlation context. Missing context or an absent facts
provider fails closed through the existing Forum audience capability error. A
locally decisive allow or deny never calls the optional provider.

Denied policy decisions return one generic public `Forbidden` message and do not
reveal the category layer or selector that rejected the caller. The diagnostic
authorization model still records the evaluated layer count, denying category,
and canonical audience decision reason for owner-side use.

`TopicService::create` and `create_command` now enforce the policy even without a
transport context. This is sufficient for unrestricted, role, and explicit-user
layers. New `create_with_audience_context` and
`create_command_with_audience_context` methods allow a future transport/runtime to
supply exact owner facts. `TopicService::with_audience_facts` publishes the facts
adapter injection seam.

This slice does **not** compose GraphQL or REST caller contexts, change DTOs, add a
migration, or publish Forum trust/Channel facts adapters. Existing transports
therefore remain compatible for categories without topic-create policy and for
locally decidable policies; externally resolved policies fail closed until the
runtime composition follow-up.

Source evidence is `tests/topic_create_audience_enforcement_sqlite.rs`; the static
guard is `scripts/verify/verify-forum-topic-create-audience-enforcement.mjs`.
Tests, Cargo commands, formatting, and verifiers were not run by the implementation
agent.
