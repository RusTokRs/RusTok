# FORUM-20AV reply-create audience enforcement

Status: source-ready / partially validated.

This note records the delivered owner boundary only. The canonical Forum roadmap
remains [`implementation-plan.md`](implementation-plan.md); this file does not replace or duplicate its backlog.

## Delivered

- `ForumReplyCreateAudienceAuthorizationService` requires
  `forum_replies:create`, resolves the tenant-scoped topic category, loads the
  inherited category reply-create policy, and requires every non-empty
  root-to-category layer to allow the current actor.
- Explicit deny wins. Matching local role or explicit-user decisions do not
  require optional owner facts.
- Trust, Channel, and Groups selectors require an exact caller `PortContext` and
  the optional `SharedForumAudienceFactsPort`; missing context or provider fails
  closed before the raw reply owner is called.
- `ReplyService::create` and `ReplyService::create_command` both route through
  the owner gate. Additive context-aware variants support exact future transport
  composition without changing the canonical DTOs.
- Authorization completes before reply-body/relation preparation and before
  reply, body, relation, counter, user-stat, or domain-event writes.
- Denials use one generic public message and do not disclose the failing
  category layer.

## Current boundary

Categories without a configured reply-create layer use the canonical unrestricted policy.
No migration, DTO, GraphQL, REST, OpenAPI, or dependency change was introduced
in this owner slice. GraphQL and REST now compose exact authenticated transport
context through the delivered `FORUM-20AW` boundary.

Authoritative Forum trust is now published by the Forum owner and host-composed
through `ForumUserTrustAudienceFactsPort`. This slice did not create that trust
state and never derives trust from `forum_user_stats` activity counters.

## Canonical plan synchronization

Resolved by `FORUM-20BA`. The canonical ledger records this owner enforcement,
its `FORUM-20AW` transport composition, topic-local narrowing, moderation policy,
and existing moderation transport composition through `FORUM-20AZ`.

## Verification status

The source verifier was executed on 2026-08-11. The retained SQLite/runtime
evidence, workflows, and CI remain pending and are recorded in
`forum-reply-create-audience-enforcement.json`.
