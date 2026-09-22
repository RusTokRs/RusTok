# FORUM-20AW reply-create audience transport composition

Status: source-verified / runtime evidence pending.

## Delivered

- GraphQL standard `createForumReply` and inline-quote `createForumReplyWithQuotes` build an exact authenticated read-only `PortContext` before invoking the reply owner.
- REST standard and command reply-create handlers build the same exact context from `TenantContext`, `AuthContext`, and `RequestContext`.
- Tenant and actor identity come only from authenticated transport extensions; request DTOs do not carry or select either identity.
- The standard GraphQL mutation accepts an optional tenant UUID for headless callers and otherwise resolves the trusted request tenant; an explicit mismatch still fails closed.
- The context forwards effective permission claims, the resolved route channel when available, request locale, a bounded five-second deadline, and a unique transport correlation identity.
- `ForumGraphqlRuntimeData` and `ForumHttpRuntime` compose `ReplyService` from the same optional `SharedForumAudienceFactsPort` already published by the host for topic creation.
- Missing provider composition remains valid for unrestricted, role-only, and explicit-user decisions; unresolved trust, Channel, or Groups selectors continue to fail closed in the owner.
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

Source checks executed on 2026-08-11:

```text
cargo test -p rustok-forum reply_create_transport -- --nocapture
node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs
```

The targeted library test passed with three cases, and the source verifier
passed. Live database mutation evidence, the broader Forum module validation
gate, workflows, and CI remain pending.
