# FORUM-20AY moderation audience policy

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

- # FORUM-20AY moderation audience policy
- source-ready / unvalidated
- The exact tenant-scoped topic author remains independently authorized
- Existing solution transports were composed in `FORUM-20AZ`
- Resolved by `FORUM-20BA`
- were not run by the implementation agent

## Selected hierarchy continuation

Public moderation admission and richer/moderation category policy management now
retain the configured `ForumTaxonomyPort` through `with_taxonomy_port`. The
canonical moderation policy loader requires that selection and borrows the actual
connection/transaction. GraphQL/HTTP moderation factories forward it with or
without audience facts. Errors from a selected provider never become unrestricted
standalone policy. Owner authorization is inside the category-tree-fenced writer
transaction, before domain effects; a pre-transaction preview cannot authorize a
later write. The exact topic-author solution path is unchanged.

Six added regressions in `tests/support/taxonomy_moderation_authorization.rs` are
mounted by `taxonomy_read_wiring`, including configured GraphQL and HTTP requests.
They remain uncompiled/unexecuted because Cargo is absent. Source/mutation guards
are supplemental evidence only. See the canonical implementation plan for exact
local results, remaining move/merge/tombstone propagation and runtime acceptance.
