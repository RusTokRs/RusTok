# rustok-blog implementation plan — slice 99 continuation

Status: `storefront_cached_public_comments_snapshot_source_ready_maintainer_execution_pending`.

This slice closes the remaining source-only cached public Comments snapshot gap that followed the typed `AVAILABLE` / `UNAVAILABLE` / `TIMEOUT` storefront state. It does not claim Comments provider runtime execution, browser evidence, Redis execution, or comment-form fallback completion.

## Re-audit

Before this slice, GraphQL and native SSR both preserved the selected article when the Comments owner returned only `ExternalService` or `Timeout`, but they replaced the Comments page with an empty degraded payload. The active Blog plan and `blog-comments-runtime-fallback-smoke.json` still marked `show_cached_thread_snapshot` as planned.

The platform already has the correct cache owner:

- `rustok-cache::CacheService` owns Redis lifecycle and bounded in-memory fallback;
- server runtime initialization retains one process-wide `CacheService`;
- `ModuleRuntimeExtensions` transfers typed host capabilities into both GraphQL and server-function `HostRuntimeContext` composition.

A second Comments table, queue, relay, Redis client, or storefront-only cache owner is therefore unnecessary.

## Slice 99 — one Blog snapshot policy for both storefront transports

New source:

`crates/rustok-blog/src/public_comments_snapshot.rs`

`list_public_comments_with_snapshot` remains downstream of the canonical `CommentService::list_for_post_with_locale_fallback` public-read path. It never queries or mutates Comments persistence directly.

### Live success

A successful approved public Comments read remains authoritative and returns:

- `availability = AVAILABLE`;
- `cached_snapshot = false`;
- the live items and total.

When a snapshot store is present, the same successful page is written best-effort after source validation. Cache write failure cannot replace or fail the owner response.

### Degraded owner read

Only the existing two degradable Blog error kinds are eligible:

- `ExternalService -> UNAVAILABLE`;
- `Timeout -> TIMEOUT`.

For every other `BlogError`, the helper returns the original error and does not consult the snapshot cache.

On an eligible error:

1. an exact valid cache hit returns the original degraded availability plus `cached_snapshot = true` and the retained items/total;
2. a miss, cache failure, oversized entry, invalid JSON, schema drift, identity mismatch, cross-post row, or non-approved row returns the original degraded availability with `cached_snapshot = false`, empty items and total zero.

The availability signal is never rewritten to `AVAILABLE` when stale data is shown.

## Snapshot identity and bounds

The cache identity contains:

- tenant id;
- post id;
- requested locale;
- fallback locale;
- page;
- page size.

The serialized identity is bound to the `blog-public-comments-snapshot-v1` schema prefix and SHA-256 hashed before it becomes a backend key. The cached envelope repeats the complete identity and schema version, so a digest collision or corrupt/misrouted backend value still has to pass exact identity validation before use.

Both writes and reads enforce:

- schema version 1;
- exact identity equality;
- same post id for every item;
- `approved` status for every retained item;
- item count not greater than requested page size;
- total not lower than retained item count;
- maximum encoded payload size of 256 KiB.

This is a bounded stale public projection, not an ownership transfer from Comments.

## Host cache composition

New adapter:

`apps/server/src/services/blog_public_comments_snapshot.rs`

The server adapter reuses the canonical `CacheService` from `ensure_cache_service` and lazily materializes one backend with:

- prefix `blog-public-comments-snapshot-v1`;
- TTL 900 seconds;
- maximum 10,000 keys.

Redis is reused automatically when configured by the existing cache runtime; otherwise the same cache capability provides its bounded in-memory fallback. The Blog adapter never resolves a Redis URL or creates a Redis client directly.

The adapter is registered as `Arc<dyn PublicCommentsSnapshotStore>` in the existing host-provider `ModuleRuntimeExtensions`, so GraphQL schema data and native server-function runtime receive the same typed store.

## Transport and UI parity

GraphQL `publicComments` now exposes `cachedSnapshot` alongside the existing availability state. Native SSR maps the same shared helper result into the same storefront DTO.

The Leptos storefront behavior is:

- live `AVAILABLE`: render the ordinary list;
- `UNAVAILABLE` / `TIMEOUT` without snapshot: retain the existing message-only degraded state;
- `UNAVAILABLE` / `TIMEOUT` with `cachedSnapshot = true`: render a warning that a recent cached snapshot is being shown, then render the retained list and pagination.

Stale Comments are therefore visible but never presented as live.

## Machine evidence

The existing fail-closed source boundary remains authoritative:

- `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json` is advanced to schema v3;
- `scripts/verify/verify-blog-comments-port-boundary.mjs` source-locks the shared snapshot policy, host cache bounds, GraphQL/native parity, UI stale disclosure, and preserved fail-closed error policy.

The broader fallback smoke remains `planned` because comment-form fallback is still a separate unfinished result.

## Preserved boundaries

This slice does not add or change:

- Comments storage, moderation, lifecycle, visibility, counters, threads, or event ownership;
- Blog database schema or migrations;
- a queue, worker, relay, retry lane, or DLQ;
- a direct Redis dependency in Blog;
- moderation Comments caching;
- fallback for validation, forbidden, not-found, conflict, invariant, or other non-availability errors;
- comment submission behavior;
- FFA/FBA promotion.

## Maintainer validation boundary

Suggested source/runtime validation remains maintainer-owned. No tests, Cargo commands, Node verifiers, formatting, builds, browser targets, Redis scenarios, workflows, CI, HTTP execution, or runtime validation were executed by the implementation agent.

## Next cursor

Re-audit the storefront write surface for the planned `hide_comment_form` degraded mode. If a public comment form is still absent from the active storefront, actualize the plan instead of inventing a fallback for a nonexistent surface. If the form exists, keep its write fallback separate from this read snapshot and preserve typed Comments write errors.

Browser/runtime evidence for cached-snapshot behavior remains a separate maintainer-execution result after the source boundary is retained.
