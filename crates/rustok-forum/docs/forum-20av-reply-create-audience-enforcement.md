# FORUM-20AV reply-create audience enforcement

Status: source-ready / unvalidated.

This note records the delivered owner boundary only. The canonical Forum roadmap
remains [`implementation-plan.md`](implementation-plan.md); this file does not
replace or duplicate its backlog.

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
  composition without changing existing DTOs.
- Authorization completes before reply-body/relation preparation and before
  reply, body, relation, counter, user-stat, or domain-event writes.
- Denials use one generic public message and do not disclose the failing
  category layer.

## Compatibility

Categories without a configured reply-create layer retain historical behavior.
No migration, DTO, GraphQL, REST, OpenAPI, or dependency change is introduced in
this slice. GraphQL and REST still call the context-free methods; policies that
need trust, Channel, or Groups facts remain fail closed until the separate
transport-composition slice supplies exact authenticated context.

Forum trust remains unavailable. `forum_user_stats` activity counters are not
an authoritative trust model and are not used by this boundary.

## Verification status

The implementation agent did not run tests, Cargo commands, formatting,
verifiers, workflow checks, or CI. Maintainer commands are recorded in
`forum-reply-create-audience-enforcement.json`.

The canonical plan was not rewritten in this slice because the available GitHub
connector supports only complete-file replacement and the plan exceeds two
thousand lines; risking unrelated roadmap loss was rejected. A later safe
repository-local edit should advance the ledger from `FORUM-20AU` to
`FORUM-20AV` and record transport composition as the next bounded slice.
