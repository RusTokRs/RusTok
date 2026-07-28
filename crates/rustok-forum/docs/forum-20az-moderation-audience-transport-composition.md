# FORUM-20AZ moderation audience transport composition

Status: source-ready / unvalidated.

## Delivered

- The existing GraphQL `markForumTopicSolution` and `clearForumTopicSolution` mutations now build one exact authenticated moderation `PortContext` and call the context-aware `ModerationService` owner methods.
- The existing REST `POST /api/forum/topics/{topic_id}/solution/{reply_id}` and `DELETE /api/forum/topics/{topic_id}/solution` routes now use dedicated thin handlers with the same exact owner composition.
- Tenant and actor identity come only from `TenantContext`, `AuthContext`, and the middleware `RequestContext`; GraphQL arguments and REST paths cannot select another principal.
- The shared context helper forwards the effective permission snapshot, middleware locale or tenant fallback, the already resolved route channel, a five-second facts deadline, and a bounded unique correlation identifier.
- GraphQL `ForumGraphqlRuntimeData` and HTTP `ForumHttpRuntime` build `ModerationService` from the same optional host-published `SharedForumAudienceFactsPort` already used by topic and reply creation.
- The owner authorization seam validates an explicit context's tenant and user before the first topic or reply lookup. Unresolved trust, Channel, or Groups facts still fail closed when the optional provider is absent.
- Transport admission now requires authentication but does not duplicate owner authorization. The exact tenant-scoped topic author can mark or clear a solution, while every non-author must hold moderator scope and satisfy the inherited moderation audience policy.
- Both transports preserve the owner gate before solution, counter, user-stat, journal, and outbox writes and perform only the existing post-command topic projection read.

## Boundary

- No new GraphQL field, REST route, OpenAPI shape, public request/response DTO, migration, dependency, host/server source, or Forum trust state is added.
- No approve/reject/hide reply transport or pin/lock/status topic transport is introduced because those owner methods have no existing public Forum route in the current runtime.
- Existing context-free `ModerationService` methods remain available for direct owner consumers and locally decidable compatibility; transport call sites use only context-aware methods.
- Trust remains blocked on `FORUM-26` and is never derived from `forum_user_stats` activity counters.

## Canonical plan debt

The canonical `crates/rustok-forum/docs/implementation-plan.md` is intentionally not rewritten in this slice. The available GitHub contents API requires complete-file replacement while the roadmap exceeds two thousand lines; risking unrelated roadmap loss is not acceptable. A later safe repository-local edit must advance the FORUM-20 ledger through `FORUM-20AZ`, retain Forum trust ownership and remaining exact-read migrations as open scope, and fix the previously recorded historical grammar typo.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
node scripts/verify/verify-forum-moderation-audience-transport-composition.mjs
cargo test -p rustok-forum moderation_transport -- --nocapture
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows, and CI remain the maintainer's responsibility for this slice.
