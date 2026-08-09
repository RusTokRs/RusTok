# rustok-blog implementation plan — slice 100 continuation

Status: `storefront_comment_form_fallback_not_applicable_source_verified`.

This slice resolves the next cursor from slice 99 by re-auditing the active Blog storefront write surface. The result is a planning correction, not a new UI feature: the active storefront package is read-only and has no public comment form or create-comment transport to hide when the Comments owner is unavailable.

## Re-audit result

The active `rustok-blog-storefront` package owns:

- dual-path read access for published posts;
- approved public Comments reads;
- comment pagination;
- GraphQL and native SSR fetch adapters;
- Leptos rendering of the selected post and its public Comments projection.

It does **not** own:

- a `<form>` or `<textarea>` comment composer;
- a submit handler;
- `CreateCommentInput` in storefront source;
- a storefront `create_comment` call;
- a GraphQL storefront mutation;
- a native server function that writes Comments.

The source inventory is retained in:

`crates/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json`

Fail-closed source guard:

`scripts/verify/verify-blog-comments-storefront-write-surface.mjs`

## Planning correction

The historical degraded mode `hide_comment_form` remains present in the Blog/Comments FBA registry vocabulary. Slice 100 does not perform a registry schema migration solely to delete that legacy token because the consumer/provider registries currently source-lock the same vocabulary and the token does not activate behavior.

Its canonical interpretation is now:

`compatibility_vocabulary_not_active_storefront_surface`

and the concrete implementation result is:

`comment_form_fallback = not_applicable_no_storefront_write_surface`.

`blog-comments-runtime-fallback-smoke.json` therefore keeps the legacy `comment_form_fallback = planned` field only for current aggregate registry compatibility and adds an explicit interpretation pointer plus the authoritative `storefront_write_surface` block. The `create_comment` fallback case is marked `legacy_not_applicable_no_storefront_write_surface` and is not an implementation target.

This avoids inventing a public comment composer merely to satisfy an obsolete fallback placeholder.

## Remaining storefront fallback boundary

The only active degraded storefront Comments source result is the cached public read snapshot implemented in slice 99:

- live approved read refreshes the snapshot best-effort;
- `ExternalService` and `Timeout` may consume an exact valid snapshot;
- stale data preserves `UNAVAILABLE` / `TIMEOUT` and is disclosed as cached;
- all other errors remain fail-closed;
- GraphQL and native SSR use the same snapshot policy and host cache capability.

Its source is ready, but cached read fallback runtime evidence is still maintainer-owned and pending. The broad `fallback_smoke.status = planned` now means **cached read fallback runtime evidence**, not a missing comment-form implementation.

## Preserved boundaries

Slice 100 changes no production behavior and authorizes no new storefront write surface. It does not change:

- Comments owner storage or write APIs;
- Blog CommentService write behavior;
- GraphQL Blog mutations used by authenticated/admin surfaces;
- native admin moderation surfaces;
- storefront routing or UI behavior;
- cache behavior from slice 99;
- FFA/FBA promotion status.

The existing `hide_comment_form` registry token remains compatibility vocabulary until a future deliberate registry schema migration can remove or rename it together on both consumer and provider sides.

## Validation boundary

Suggested maintainer source check:

```bash
node scripts/verify/verify-blog-comments-storefront-write-surface.mjs
```

No tests, Cargo commands, Node verifiers, formatting, builds, browser targets, HTTP scenarios, Redis scenarios, workflows, CI, or runtime validation were executed by the implementation agent.

## Next cursor

Do not add a storefront comment form as fallback work. The storefront Comments fallback line now has no remaining source-only write task.

The next result on this line is maintainer execution of cached read fallback runtime evidence. For additional autonomous Blog source work, return to the broader implementation plan and select an independent source gap rather than adding more fallback scaffolding.
