# Pages / Page Builder Native Storefront Reviewed Artifact Packet

Date: 2026-08-05
Status: source-ready / execution-pending / FFA-FBA-not-promoted
Scope: reviewed Fly body → Page Builder sanitization/materialization → Pages atomic publish/binding → routed native storefront selection → verified artifact URL → composite cache

## Rechecked cursor

PR #2990 retained routed-channel Pages module admission before any native storefront generation or cache read. The next open parity seam was the builder-body branch inside the same registered `/api/fn/pages/storefront-data` endpoint: production source already loaded a bound immutable artifact, but the retained server-function fixture still used a non-builder HTML body.

This packet closes that source cursor without changing production behavior.

## Reviewed publication fixture

`crates/rustok-pages/storefront/tests/native_storefront_reviewed_artifact_sqlite.rs` creates a real GrapesJS/Fly body through `PageService::create` and publishes it through `PageService::publish_reviewed` with:

- the exact current page version;
- the exact localized body revision;
- a deterministic idempotency key;
- `PageBuilderReviewedPublishRuntime::new` with an explicit scenario and transient JSON context.

The owner transaction therefore reaches the authoritative Page Builder sanitizer, runtime materialization/compiler, immutable artifact persistence and binding, page state transition, transactional `NodeUpdated`/`NodePublished` outbox writes and durable publish receipt.

The harness does not manually invent a storefront artifact. It queries the record produced by the reviewed owner and requires non-null:

- `materialization_hash`;
- `materialization_identity`;
- `runtime_snapshots`.

It also retains SHA-256-shaped receipt, artifact and content identities before mounting the route.

## Registered routed endpoint

The same real server-function registry is mounted through:

```text
POST /api/fn/{*fn_name}
  -> handle_server_fns_with_context
  -> provide_context(HostRuntimeContext)
  -> pages/storefront-data
```

Requests carry trusted `TenantContextExtension` and `ChannelContextExtension` values. Production derives `RequestContext`; the test does not construct or call a replacement native adapter.

The fixture applies real Outbox, Channel and Pages migrations. A minimal test-only `tenants(id)` table satisfies the Channel foreign key, and an empty test-only `tenant_modules` table exposes the real tenant-module query used by reviewed publication while retaining the production absent-row compatibility policy. No production migration or schema is changed.

## Visible channel and immutable artifact URL

The reviewed page is constrained to channel slug `web`. A real `web` channel with an explicit enabled Pages binding is created through `ChannelService`.

The admitted request must return:

```text
format = fly_artifact_url
content = /api/pages/{page_id}/artifact?locale=en&channel=web
```

This proves that the registered native route consumes the binding produced by reviewed publication and preserves the selected public channel in the artifact URL. The complete owner response is cached only after the artifact service has reloaded and verified the published binding and full materialization envelope.

## Hidden-channel isolation

A second real channel, `mobile`, also has Pages enabled but is not present in the page visibility set. The same registered route succeeds without returning the reviewed artifact URL.

The `web` and `mobile` requests use different composite cache keys. Therefore channel authorization is not merely encoded in the returned URL; it participates in both authoritative owner selection and cache variation.

## Integrity failure cannot fill a fresh key

After the valid `web/en` response is verified and cached, the retained artifact document is deliberately modified without updating its immutable hashes. A new `web/fr` request uses a distinct requested-locale cache variant and therefore misses the prior key.

The production artifact service reconstructs the Page Builder materialization envelope, rejects the corrupted record, and the server function returns an error. The recording cache observes the generation read and miss lookup but no additional put and no new stored value.

This source sequence retains the required ordering:

```text
channel admission
→ generation/key
→ cache miss
→ Pages page and channel selection
→ published binding lookup
→ Page Builder artifact/materialization integrity verification
→ artifact URL composition
→ complete response cache fill
```

A cache value cannot be created from an unverified immutable artifact.

## Evidence boundary

Machine evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-native-storefront-reviewed-artifact-source.json`;
- `crates/rustok-pages/storefront/tests/native_storefront_reviewed_artifact_sqlite.rs`;
- `crates/rustok-pages/scripts/verify/verify-pages-native-storefront-reviewed-artifact.mjs`.

The execution list is empty and every validation flag remains false. Tests, Cargo, formatting, verifiers, SQLite/Axum, the Leptos route, workflows and CI were not run.

## Deliberate limits

This slice does not:

- modify production Pages, Page Builder, Channel, cache or server-function behavior;
- change reviewed publish, sanitizer, materialization, binding or receipt contracts;
- change migrations, entities, DTOs, routes, codecs, cache keys or TTL;
- prove durable relay delivery or generation rotation in the same process as the native request;
- claim PostgreSQL, browser, workflow, CI or observed tenant evidence;
- promote FFA or FBA status.

## Remaining cursor

The next vertical source packet should connect one exact reviewed publish revision and its committed `NodePublished` event to the durable dispatcher, real Pages generation rotation and the next admitted registered native storefront miss/refill. That packet should retain event/correlation identity, old-key physical retention and a newly reachable composite key in one process.

## Suggested maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-reviewed-artifact.mjs
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_reviewed_artifact_sqlite -- --nocapture
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-page-builder --all-targets
cargo check -p rustok-pages --all-targets
cargo check -p rustok-channel --all-targets
```
