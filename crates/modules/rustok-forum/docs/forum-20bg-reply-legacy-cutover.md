# FORUM-20BG — canonical legacy reply cutover

`FORUM-20BG` removes the remaining reply-read bypasses left after the exact
parent-topic audience owner was published.

## Delivered boundary

- Existing GraphQL `forumReplies` keeps its schema and profile enrichment but
  now derives a trusted GraphQL reply-list context and calls
  `ForumReplyAudienceReadService`.
- Existing GraphQL `forumStorefrontReplies` keeps its schema and approved-only
  output while both public and authenticated paths call the exact reply owner.
- The canonical GraphQL storefront adapter continues to request
  `forumStorefrontReplies`; that existing field is now exact, so no second
  GraphQL reply request is required.
- The canonical native `forum/storefront-data` server function constructs the
  exact reply owner from the same host-published audience facts used by topic
  reads. Authenticated reads use a trusted native reply-list context; public
  reads use the route channel.
- The storefront transport selector performs one request for the selected
  transport and no longer replaces the returned reply page.
- Temporary `graphql_reply_audience_adapter.rs` and
  `native_reply_audience_adapter.rs` files are removed.

## Compatibility

No existing GraphQL field name, REST route, native server-function endpoint,
request DTO, response DTO, storefront model, selected-topic gate, mark-read
path, dependency, or migration changed.

The additive `forumAudienceReplies` and `forumStorefrontAudienceReplies` fields
remain available. Existing consumers can continue using the legacy field names,
but those names no longer bypass richer audience authorization.

## Explicitly not delivered

`FORUM-20BG` does not migrate category owner/storefront reads, search/index,
SEO, deep links, visibility-scoped category/all-read commands, or PostgreSQL
runtime evidence. Those begin with `FORUM-20BH`.

The canonical `implementation-plan.md` and `CRATE_API.md` are not replaced
through the GitHub contents API in this slice. Their conflict-safe
repository-local synchronization debt remains explicit in the machine contract.

## Validation handoff

The implementation agent did not run tests, Cargo commands, formatting,
verifiers, workflows, or CI, per maintainer request.

Suggested maintainer commands:

```text
cargo test -p rustok-forum reply_read_transport -- --nocapture
cargo test -p rustok-forum --test topic_audience_exact_read_sqlite -- --nocapture
node scripts/verify/verify-forum-reply-audience-read.mjs
node scripts/verify/verify-forum-reply-legacy-cutover.mjs
cargo xtask module validate forum
```
