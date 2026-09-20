# rustok-blog implementation plan — slice 100 continuation

Status: `storefront_comment_form_active_source_verified`.

This slice corrects the previous source inventory after a fresh audit of the active Blog storefront. The package does expose an authenticated public comment write surface alongside its dual-path public comment read surface.

## Re-audit result

The active `rustok-blog-storefront` package owns the public comment write surface and also the dual-path public comment read surface.

It owns:

- dual-path read access for published posts;
- approved public Comments reads;
- comment pagination;
- GraphQL and native SSR fetch adapters;
- a Blog-bound public comment composer;
- GraphQL and native create-comment transports;
- the authenticated command path into the Blog `CommentService` and Comments owner port.

The write path is bounded by the Blog adapter contract:

- authenticated actor and exact current tenant;
- `comments:create` effective permission;
- tenant-level Blog module enablement;
- current enabled Blog channel;
- published post visibility for that channel;
- stable `command_id` passed through to Comments idempotency;
- canonical `RichTextDocument` input;
- owner-controlled `CommentService::create_public_comment`.

The source inventory is retained in:

`crates/modules/rustok-blog/contracts/evidence/blog-comments-storefront-write-surface.json`

Fail-closed source guard:

`scripts/verify/verify-blog-comments-storefront-write-surface.mjs`

## Planning correction

The historical degraded mode `hide_comment_form` remains present in the Blog/Comments FBA registry vocabulary, but it now maps to a real active storefront write surface rather than a nonexistent one.

Its current interpretation is:

`fallback_vocabulary_for_active_storefront_write_surface`.

The concrete source result is:

`comment_form_fallback = planned`.

The fallback behavior itself has not been runtime-verified and is not implemented by this slice. The existing cached public Comments snapshot remains the independently implemented degraded read path.

## Remaining storefront fallback boundary

The active degraded storefront Comments source result is the cached public read snapshot:

- live approved reads refresh the snapshot best-effort;
- `ExternalService` and `Timeout` may consume an exact valid snapshot;
- stale data preserves `UNAVAILABLE` / `TIMEOUT` and is disclosed as cached;
- all other errors remain fail-closed;
- GraphQL and native SSR use the same snapshot policy and host cache capability.

The active comment-form degraded mode is a separate planned boundary. No claim of runtime fallback execution is made.

## Preserved boundaries

This correction does not invent or newly authorize a storefront write surface. It actualizes the surface already present in source and documentation.

It does not change:

- Comments owner storage or write APIs;
- Blog CommentService write behavior;
- GraphQL Blog mutation ownership;
- native admin moderation surfaces;
- tenant/channel enforcement on storefront writes;
- cache behavior from slice 99.

## Validation boundary

Suggested maintainer source check:

```bash
node scripts/verify/verify-blog-comments-storefront-write-surface.mjs
```

No tests, Cargo commands, Node verifiers, formatting, builds, browser targets, HTTP scenarios, Redis scenarios, workflows, CI, or runtime validation were executed by the implementation agent.

## Next cursor

The storefront Comments write surface is now source-verified and machine-guarded. The remaining fallback work on this line is runtime evidence for cached public reads plus future deliberate implementation of the `hide_comment_form` degraded UI mode, with no promotion claim beyond `boundary_ready`.
