# FORUM-20AS topic-create audience transport composition

**Status:** source-ready / unvalidated

`FORUM-20AS` composes the `FORUM-20AR` topic-create audience owner gate into
both Forum GraphQL topic-create mutations and both REST create handlers. Topic
DTOs remain unchanged: tenant and actor identity come only from authenticated
transport extensions.

A shared Forum transport helper validates the authenticated tenant against the
resolved tenant and, when an HTTP request snapshot exists, validates its tenant
and user against the same principal. It then builds one read-only `PortContext`
with a five-second deadline, a bounded generated correlation id, the effective
locale, permission claims, and the middleware-resolved route channel. A wrong
tenant or actor fails before an audience facts provider can be called.

Forum GraphQL now declares a manifest-backed runtime-data factory. The factory
consumes `SharedForumAudienceFactsPort` from neutral `GraphqlRuntimeInputs`
without importing a server adapter or discovering Groups storage. The Forum
HTTP runtime reads the same typed value from `HostRuntimeContext`. Both host
contexts already receive `ModuleRuntimeExtensions`, where the feature-guarded
server Groups adapter was published by `FORUM-20Q`; this slice consumes that
existing publication rather than creating a second provider path.

When `mod-forum` and `mod-groups` are compiled, unresolved Groups selectors can
therefore reach the Groups-owned effective-membership read port from GraphQL or
REST. When the provider is absent, unrestricted categories and locally decisive
role/explicit-user policies keep working, while unresolved trust, Channel, or
Groups selectors continue to fail closed through the owner capability error.

Both legacy create methods and inline-quote command methods still delegate to
the same `TopicService` owner facade. `FORUM-20AR` authorization remains before
all topic, translation, relation, counter, user-stat, and event writes.

This slice adds no Forum-to-Groups crate dependency, migration, topic-create DTO
field, trust facts adapter, or Channel membership facts adapter. Reply and
moderation audience policies remain separate follow-ups.

Source guards are
`scripts/verify/verify-forum-topic-create-audience-transport-composition.mjs`
and the inline contract tests in `src/topic_create_transport.rs` and
`src/graphql/runtime_data.rs`. Tests, Cargo commands, formatting, and verifiers
were not run by the implementation agent.
