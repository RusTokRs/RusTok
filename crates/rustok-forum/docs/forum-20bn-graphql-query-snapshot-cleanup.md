# FORUM-20BN — GraphQL query snapshot cleanup

## Purpose

Forum GraphQL query composition has used `src/graphql/query_runtime.rs` as the canonical compiled owner since FORUM-20BG/BH. The older `src/graphql/query.rs` file remained in the repository only because source verifiers and the FORUM-11 diagnostics workflow still read or patched it as text.

FORUM-20BN migrates those consumers to the live runtime and deletes the unreachable snapshot.

## Canonical runtime

`crates/rustok-forum/src/graphql/mod.rs` continues to select:

```rust
#[path = "query_runtime.rs"]
mod query;
```

No GraphQL field names, merged root composition, REST routes, DTOs or transport selectors change in this cleanup.

The live runtime retains the existing exact owner boundaries:

- category single/list/storefront reads use `ForumCategoryAudienceReadService`;
- reply owner and storefront reads use `ForumReplyAudienceReadService`;
- trusted category/reply `PortContext` construction remains transport-owned;
- public storefront replies retain the approved-only filter and current route-channel scope;
- public channel module enablement remains checked before storefront output.

## Migrated source locks

The following consumers now read only `query_runtime.rs`:

- Forum reply audience verifier;
- Forum reply legacy cutover verifier;
- Forum category audience verifier;
- Channel proof-point verifier.

The Channel proof-point verifier now locks the real public reply path rather than a test name inside an uncompiled snapshot: `public_channel_slug`, `is_topic_visible_for_channel`, the exact public reply owner call and `PUBLIC_REPLY_STATUSES`.

The FORUM-11 diagnostics workflow no longer patches `query.rs` or includes it in its formatting target list. It formats the canonical `query_runtime.rs` instead, so an automated diagnostic pass cannot recreate the deleted file.

## Removal invariant

`crates/rustok-forum/src/graphql/query.rs` must not exist after this task. The FORUM-20BN verifier fails if:

- the snapshot returns;
- `mod.rs` stops selecting `query_runtime.rs`;
- any migrated verifier or diagnostics workflow references `query.rs`;
- exact category/reply owner markers disappear from the live runtime;
- historical and current contracts stop recording the cleanup completion.

## Compatibility

This is repository cleanup only:

- no GraphQL schema or field change;
- no REST or public DTO change;
- no migration;
- no workspace dependency or `Cargo.lock` change;
- no FFA/FBA readiness promotion.

The large canonical implementation plan and `CRATE_API.md` remain conflict-sensitive synchronization debt and are not rewritten through the GitHub Contents API in this slice.

## Remaining work

FORUM-20BO should decide whether approved public replies become independent Search documents under FORUM-23. Runtime evidence for Search rebuild preservation, projection cleanup, query execution and exact-visible bulk reads also remains maintainer-owned.

## Validation status

No tests, Cargo commands, formatting, verifiers, workflows or CI were run by the implementation agent.
