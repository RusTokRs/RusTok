# FORUM-20AW reply-create audience transport composition

Status: source-ready / unvalidated.

## Delivered

- GraphQL legacy `createForumReply` and inline-quote `createForumReplyWithQuotes` build an exact authenticated read-only `PortContext` before invoking the reply owner.
- REST legacy and command reply-create handlers build the same exact context from `TenantContext`, `AuthContext`, and `RequestContext`.
- Tenant and actor identity come only from authenticated transport extensions; request DTOs do not carry or select either identity.
- The context forwards effective permission claims, the resolved route channel when available, request locale, a bounded five-second deadline, and a unique transport correlation identity.
- `ForumGraphqlRuntimeData` and `ForumHttpRuntime` compose `ReplyService` from the same optional `SharedForumAudienceFactsPort` already published by the host for topic creation.
- Missing provider composition remains compatible for unrestricted, role-only, and explicit-user decisions; unresolved trust, Channel, or Groups selectors continue to fail closed in the owner.
- Both public reply-create owner methods remain gated before reply, body, relation, counter, user-stat, and event writes.

## Boundary

- No reply-create, quote, GraphQL, REST, or OpenAPI DTO fields changed.
- No migrations or dependencies changed.
- No Forum-to-Groups crate dependency was added.
- No trust facts adapter or Forum trust owner state was added by this slice.
  Authoritative Forum trust is now supplied separately through
  `ForumUserTrustAudienceFactsPort`.
- No topic-local reply audience narrowing or moderation audience policy was added
  by this slice; those boundaries were subsequently delivered in `FORUM-20AX`
  and `FORUM-20AY`.
- The host publication path is reused without a second provider registry or transport-owned facts query.

## Canonical plan synchronization

Resolved by `FORUM-20BA`. The canonical plan records reply-create owner
enforcement and exact GraphQL/REST composition before the later topic-local and
moderation audience slices.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
cargo test -p rustok-forum reply_create_transport -- --nocapture
cargo test -p rustok-forum graphql::runtime_data -- --nocapture
node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs
node scripts/verify/verify-forum-reply-create-audience-enforcement.mjs
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows, and CI remain the maintainer's responsibility for this slice.
