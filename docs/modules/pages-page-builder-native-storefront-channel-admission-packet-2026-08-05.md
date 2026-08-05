# Pages / Page Builder Native Storefront Channel Admission Packet

Date: 2026-08-05
Status: source-ready / execution-pending / FFA-FBA-not-promoted
Scope: routed channel context → Pages module binding admission → registered native storefront server function → composite cache

## Rechecked cursor

PR #2988 retained the real registered Leptos endpoint `/api/fn/pages/storefront-data` with real Pages owner data, `HostRuntimeContext`, trusted tenant context and the typed Pages cache runtime. That packet intentionally omitted channel context, so the production `ChannelService::is_module_enabled` gate remained source-connected but unretained at the real route boundary.

This slice closes that source cursor without changing production behavior.

## Registered route admission packet: ready, unvalidated

`crates/rustok-pages/storefront/tests/native_storefront_channel_admission_sqlite.rs` mounts the real server-function registry through the same wildcard Axum shape used by the server host:

```text
POST /api/fn/{*fn_name}
  -> handle_server_fns_with_context
  -> provide_context(HostRuntimeContext)
  -> pages/storefront-data
```

The request carries trusted `TenantContextExtension` and `ChannelContextExtension` values. The production `RequestContext` extractor derives tenant, channel id, channel slug and locale; the test does not call a private adapter helper or construct a replacement request context.

The fixture applies the real Outbox, Channel and Pages migrations, creates the routed channel through `ChannelService::create_channel`, manages the Pages binding through `ChannelService::bind_module`, and creates/publishes the Pages owner document through `PageService`.

A minimal test-only `tenants(id)` parent table is created because the Channel migration owns a real foreign key to the platform tenant table while this focused package does not import the global migration stack. No production schema or migration is changed.

## Disabled before cache access

The first request uses an explicit `pages=false` channel-module binding. The registered route is rejected with the production module-disabled error before:

- generation snapshot read;
- composite cache lookup;
- cache fill;
- Pages page or list owner reads.

The recording cache has zero generation reads, zero gets, zero puts and no stored keys.

## Enabled miss and refill

The binding is changed to `pages=true` through the same Channel owner. The next request succeeds, loads the published Pages owner response, misses the generation-bound composite key and fills it with the production Pages storefront TTL.

This confirms that the gate is not a permanent fixture shortcut: the same route, request context and cache runtime proceed when the owner binding allows Pages.

## Disabled with a populated cache

The binding is changed back to `pages=false` while the successful response remains stored under the current composite key. A third request is rejected before any additional generation read, cache get or cache put.

Therefore an already-populated cache value cannot bypass routed-channel admission.

## Production ordering retained

The real adapter remains unchanged and keeps:

1. trusted request and tenant extraction;
2. routed channel detection;
3. `ChannelService::is_module_enabled(channel_id, "pages")`;
4. rejection when disabled;
5. locale/channel variant construction;
6. generation snapshot and composite cache key;
7. cache hit or authoritative Pages owner reads;
8. best-effort fill only after the complete owner response.

The channel owner retains the current compatibility policy that an absent module binding defaults to enabled. This packet tests explicit disabled/enabled bindings and does not change that policy.

## Evidence boundary

Machine evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-native-storefront-channel-admission-source.json`;
- `crates/rustok-pages/storefront/tests/native_storefront_channel_admission_sqlite.rs`;
- `crates/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs`.

The execution list is empty and every validation flag remains false. Tests, Cargo, formatting, verifiers, SQLite/Axum, the Leptos route, workflows and CI were not run.

## Deliberate limits

This slice does not:

- modify production Pages, Page Builder, Channel, cache or server-function behavior;
- alter the absent-binding compatibility policy;
- change migrations, entities, DTOs, routes, codecs, cache keys or TTL;
- execute a verified immutable Page Builder artifact through the registered native route;
- connect durable `NodePublished` relay delivery to this request in one process;
- claim PostgreSQL, browser, workflow, CI or tenant rollout evidence;
- promote FFA or FBA status.

## Remaining cursor

The next source packet should exercise the registered native route with a verified immutable Page Builder artifact, including channel-constrained owner selection and the returned `fly_artifact_url` body. After that, one exact-revision continuity packet should connect durable `NodePublished` relay delivery to generation rotation and the next admitted native storefront miss/refill.

## Suggested maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs
cargo test -p rustok-pages-storefront --features ssr --test native_storefront_channel_admission_sqlite -- --nocapture
cargo check -p rustok-pages-storefront --features ssr --all-targets
cargo check -p rustok-channel --all-targets
cargo check -p rustok-pages --all-targets
```
