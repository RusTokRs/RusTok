# Pages cache-consumer production relay native-route evidence actualization — 2026-08-15

Status: `source-ready / exact-main-execution-pending / cache-consumer-admission-pending`.

## Fresh base

This slice starts from fresh `main@c2a01dc3abf0a5a7007a9a2ccc45648266cf99f1`.

The terminal inventory on the latest admitted source still has two Page Builder FBA `executed_evidence: pending` blockers:

1. `/provider/consumer_properties_contract/executed_evidence`;
2. `/consumers/0/cache_consumer/executed_evidence`.

The cache-consumer target remains `pending`; provider consumer-properties remains an independent blocker.

## Why this packet

The retained `pages_production_relay_native_route_source_v1` packet is the strongest existing cache-consumer source boundary because it crosses the production delivery path rather than exercising only cache primitives. It mounts reviewed publish, the real `OutboxRelay`, production `TenantGenerationDeliveryGate`, production `ServerPagesCachePort`, the canonical local listener readiness boundary and the registered Pages storefront server function.

The retained integration sequence covers durable `NodeCreated`, `NodeUpdated` and `NodePublished`, route/page/artifact generation rotation before acknowledgement, pre-rotation miss/fill, post-rotation miss/refill/hit, old-key physical retention without current-key reachability, stable reviewed immutable artifact URL, and process-bounded duplicate no-op for the later asynchronous Pages listener delivery.

## Current-main drift recheck

Since the preceding terminal-inventory merge, current `main` changed broad formatting/build surfaces and also touched some cache-adjacent code including the native storefront adapter, module-event composition, `rustok-cache`, Cargo manifests and lockfile. Therefore this slice does not reuse historical execution evidence and does not infer validity from the 2026-08-05 source authoring state.

Instead the workflow re-runs the retained source verifier plus supporting production generation-gate, native storefront relay-continuity and cache-invalidation verifiers on the exact PR/main checkout, then executes the production relay native-route server integration and checks the `rustok-server` Pages profile.

## Exact execution packet

PR/main validation:

- `node crates/rustok-pages/scripts/verify/verify-pages-production-relay-native-route.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-relay-continuity.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`;
- `cargo test --locked -p rustok-server --features mod-pages --test pages_production_relay_native_route_sqlite -- --nocapture`;
- `cargo check --locked -p rustok-server --features mod-pages --all-targets`.

The exact-main receipt is minted only after a successful `push/main` run and requires the canonical cache-consumer pre-state to remain `pending`. Registry admission is deliberately separate, and the workflow excludes the canonical registry from `push/main.paths` so a later registry-only admission cannot mint a post-admission receipt that expects the old pre-state.

## Boundaries

This evidence slice does not mutate Pages/Page Builder runtime, migrations, DTOs, cache policy, canonical FBA registry, terminal inventory, local readiness plans or central readiness status. It does not claim browser execution, cross-process exact-once invalidation, owner/platform approval, Pages FFA promotion or Page Builder FBA promotion.

## Next cursor

After a successful retained exact-main receipt, create a separate one-line admission PR for `/consumers/0/cache_consumer/executed_evidence: pending -> verified`, confirm that registry-only merge does not create a new exact-main receipt, then recompute terminal inventory `2 -> 1` if provider consumer-properties remains the only canonical FBA blocker.
