# FORUM-20AZ moderation audience transport composition

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

- # FORUM-20AZ moderation audience transport composition
- source-ready / unvalidated
- existing GraphQL `markForumTopicSolution`
- existing REST `POST /api/forum/topics/{topic_id}/solution/{reply_id}`
- exact tenant-scoped topic author
- No new GraphQL field, REST route, OpenAPI shape
- Authoritative Forum trust is host-composed through
- Resolved by `FORUM-20BA`
- were not run by the implementation agent
