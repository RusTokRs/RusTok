# Pages / Page Builder cache-consumer exact-main evidence actualization — 2026-08-15

Status: `source-ready / exact-main-execution-pending / registry-admission-separate`.

## Fresh boundary

This execution slice starts from fresh `main@bfbe9cd9a92dcf8224e9a7d7068a86f0506a7a12`.

The immediately preceding terminal-inventory merge was `58a214a6a801494d34b4889fcdcdb8a71c00d518`, where the canonical Page Builder FBA inventory had two recursive pending evidence nodes:

1. `/provider/consumer_properties_contract/executed_evidence`;
2. `/consumers/0/cache_consumer/executed_evidence`.

`main` then advanced through `bfbe9cd9...`. That change intersects the cache-consumer execution boundary: it changes `Cargo.toml`, `Cargo.lock`, `crates/rustok-cache/src/**`, `crates/rustok-pages/storefront/src/transport/native_server_adapter.rs`, `apps/server/src/services/module_event_dispatcher.rs` and adjacent server/Page Builder code. Therefore no previously source-ready cache packet is treated as executed evidence for the current source. The cache-consumer packet must execute again on the exact current revision.

The canonical target remains `pending`. Provider consumer-properties remains independently `pending` and is not inferred by this packet.

## Canonical cache-consumer scope

The FBA registry binds the Pages cache consumer to:

- `PageCacheInvalidationEventHandler` and the Pages-owned `PageCacheInvalidationPort` / `PagesCacheReadPort` contract;
- production `ServerPagesCachePort` composition;
- `CacheNamespaceGenerationStore` route/page/artifact generation rotation;
- `NodeUpdated`, `NodePublished`, `NodeUnpublished` and `NodeDeleted` lifecycle envelopes;
- the generation-aware native storefront and artifact HTTP readers;
- no inline lifecycle-owner invalidation;
- fail-open cache read behavior that returns to authoritative owner-source reads.

The retained execution packet therefore uses the existing source-ready harnesses rather than creating a second cache policy.

## Execution packet

The workflow executes all of these existing source guards:

- `verify-pages-cache-invalidation.mjs`;
- `verify-pages-publish-rollback-outbox-cache-postgres.mjs`;
- `verify-pages-outbox-relay-restart-postgres.mjs`;
- `verify-pages-production-relay-generation-gate.mjs`;
- `verify-pages-production-relay-native-route.mjs`;
- `verify-pages-native-storefront-cache.mjs`;
- `verify-pages-native-storefront-relay-continuity.mjs`;
- `verify-pages-artifact-http-cache.mjs`;
- `verify-pages-explicit-artifact-repair-cache.mjs`.

The runtime packet executes:

1. PostgreSQL publish/rollback durable outbox → cache-generation rotation and miss/refill/hit;
2. PostgreSQL outbox failure/restart recovery before successful cache handling;
3. SQLite/Axum artifact HTTP generation miss/refill/hit plus conditional `304`;
4. native storefront generation-aware miss/refill/hit and fail-open source behavior;
5. explicit repair activation lifecycle cache regression;
6. SQLite native storefront relay continuity;
7. the production `TenantGenerationDeliveryGate` → `ServerPagesCachePort` → registered Pages native route integration;
8. production server cache-provider and generation-gate unit packets;
9. `cargo check --locked` for Pages, Pages storefront SSR and server `mod-pages` targets.

PostgreSQL uses version 16 and the existing environment-gated harness contracts.

## Evidence semantics

A successful PR run validates the exact PR head only. It does not mutate the FBA registry and it does not create an exact-main receipt.

After the evidence PR is squash-merged, the same workflow runs on exact `push/main`. Only after every source verifier, runtime packet and Cargo check succeeds does the workflow create a bounded receipt containing:

- exact `GITHUB_SHA` and workflow provenance;
- the target cache-consumer registry pre-state (`pending`);
- the provider consumer-properties sibling pre-state (`pending`);
- pass/fail facts for the bounded cache packet;
- SHA-256 hashes of the required source/test/verifier files;
- explicit registry/readiness/promotion non-claims.

The receipt retains no database URL, credentials, tenant identity, raw database rows, event payloads or HTTP bodies.

## Lifecycle boundary

`pull_request.paths` includes the canonical FBA registry and this workflow. `push/main.paths` deliberately excludes the canonical registry and the workflow file itself while retaining cache/runtime/source/test/execution-contract/actualization triggers.

Therefore:

- the initial evidence merge produces one exact-main receipt because the new execution contract/actualization are push triggers;
- a later registry-only `pending -> verified` admission can be validated on the PR but cannot create a post-admission receipt that still requires target pre-state `pending`;
- runtime/source drift remains fail-closed and causes future exact-main re-execution.

## Non-claims

This slice does not:

- set `/consumers/0/cache_consumer/executed_evidence` to `verified`;
- verify provider consumer-properties;
- remove Pages `execution-rollout-pending`;
- recompute the terminal inventory;
- claim owner/platform readiness;
- promote Pages FFA or Page Builder FBA.

## Next cursor

After a successful retained exact-main artifact, create a separate one-line registry admission PR for only `/consumers/0/cache_consumer/executed_evidence: pending -> verified`. After that admission, recompute terminal inventory `2 -> 1` in another PR if the canonical registry remains unchanged.
