# Implementation plan for `rustok-channel`

## Current state

`rustok-channel` owns request-channel resolution, typed policies, and the
channel admin package. Resolution order is fixed and verified:
`explicit selectors -> built-in host slice -> typed policies -> explicit default
-> unresolved`. The built-in host slice remains a deliberate fast layer, not a
policy-only fallback.

The built-in host fast-path is retained: explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved.

Server middleware supplies locale and OAuth-app request facts, and the cache key
includes both. The admin package keeps a Leptos-free core, owner transport
facade, native server adapter, and REST secondary adapter; it is host-neutral.

The host channel cache is byte-weighted, uses bounded request facts, and has a
bounded monotonic tenant-generation registry with full-clear rollover and
fail-safe bypass on allocator exhaustion. Channel mutations advance
`channel_resolution_invalidation_state` through database triggers in the same
transaction as the changed channel row. Successful REST/native mutations clear
the local tenant token and publish the durable generation as a low-latency fast
path. Every serving replica owns supervised local/Redis/reconcile workers; the
five-second database reconciliation performs a safe namespace-wide local clear
when delivery was missed, the generation regressed, or a replica starts from an
unverified baseline. The worker runtime is a critical host guardrail.

The source now includes six durable-recovery evidence layers:

- SQLite reader tests prove that independent replica handles observe committed
  generations without PubSub, rolled-back changes do not advance the epoch, and
  missing generation state fails closed before recovery.
- A server runtime test starts two independent listener/cache runtimes on one
  database, proves both become not-ready when generation state disappears, and
  proves both recover after the durable state returns without Redis.
- An ignored PostgreSQL integration test, wired into the permanent cache
  workflow with ephemeral Postgres 17, covers statement triggers, an independent
  replica connection, commit/rollback, concurrent owner mutations and migration
  replay after state loss.
- An ignored live Redis server test publishes only from replica A and proves
  remote replicas consume validated invalidations before their five-second poll.
  Replica B covers fail-closed degradation, while a fresh replica C covers
  recovery so the result cannot be explained by the same worker's periodic tick.
- An ignored Axum integration test makes replica B cache the old default-channel
  name, commits a triggered update, publishes only from replica A, and requires
  B to return the new resolved channel name within three seconds. This protects
  actual resolved-value convergence rather than inferring correctness from
  worker readiness alone.
- A non-Redis Axum integration test confirms the old value remains cached after
  a direct committed mutation with no publication, then requires the persisted
  generation poll to replace it within the documented recovery window. This is
  the source contract for a completely missed fast-path event.

This durable cross-replica contract, normal delivery and missed-publication
scenarios are source-complete. They are not compiled or live verified on the
current revision until the permanent cache workflow reports successful
compiled, PostgreSQL and Redis jobs. Redis disconnect/reconnect and listener-lag
fault injection remain separate evidence.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `ChannelReadPort` / `channel.read_projection.v1` in
  `crates/rustok-channel/contracts/channel-fba-registry.json`.
- Static and fallback evidence:
  `crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json`
  and `crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json`.
- `scripts/verify/verify-channel-admin-boundary.mjs`,
  `npm run verify:channel:resolution-contract`, and
  `npm run verify:channel:proof-points` lock the UI boundary, canonical
  resolution order, and current consumer proof points.

Current proof points are `rustok-pages`, `rustok-blog`, `rustok-commerce`, and
`rustok-forum`; `verify:channel:proof-points` keeps their channel-aware
contracts documented and source-locked.

## Open results

1. **Execute the permanent durable cache gate.** Run the source-complete SQLite,
   server two-replica, missed-publication, PostgreSQL, Redis readiness and
   resolved-value scenarios on one reconciled `main` revision, then fix every
   format, compile, test or Clippy failure before recording the revision as
   verified.
   **Depends on:** GitHub Actions visibility or another Rust 1.96 build
   environment with ephemeral PostgreSQL and Redis.
   **Done when:** `compiled-contract`, `postgres-channel`, and `live-redis` pass
   on the same revision and the result is recorded without copying raw logs.

2. **Collect transport-failure stale-resolution evidence.** Exercise listener
   lag, Redis disconnect/reconnect and database outage/recovery while two serving
   replicas resolve real channel requests. Prove reconciliation rotates the
   namespace before an obsolete resolution can be served beyond the five-second
   recovery bound.
   **Depends on:** controllable Redis failure, migrated PostgreSQL/SQLite
   fixtures, and representative channel request data.
   **Done when:** tests cover listener lag, Redis disconnect/reconnect,
   generation regression and terminal-worker readiness while checking the
   resolved channel result rather than only worker state.

3. **Collect full runtime evidence for channel resolution.** Exercise
   `ChannelReadPort` and server middleware with real locale/OAuth facts, policy
   selection, inactive/degraded behavior, cache isolation, generation rollover,
   and the durable cross-replica behavior before promotion beyond
   `boundary_ready`.
   **Depends on:** a composed server runtime and representative request fixtures.
   **Done when:** targeted Rust middleware/port tests provide reproducible
   runtime evidence for every published read and fallback profile.

4. **Extend channel-aware proof points only with owner evidence.** New domain
   reads must use the already resolved `ChannelContext`, local tests, and local
   documentation; they must not introduce a second channel-selection mechanism.
   **Depends on:** the consuming module's public contract.
   **Done when:** the proof-point verifier and affected module docs identify the
   same resolved-channel source and visibility behavior.

5. **Defer richer target or connector taxonomy until pressure is concrete.**
   Do not add speculative target types or connector abstraction merely to expand
   the model.
   **Depends on:** a demonstrated runtime/product need.
   **Done when:** a new type has resolution semantics, migration ownership,
   operator UI implications, and focused contract tests.

## Verification

- `npm run verify:channel:admin-boundary`
- `npm run verify:channel:fba`
- `npm run verify:channel:resolution-contract`
- `npm run verify:channel:proof-points`
- `cargo check -p rustok-channel --lib`
- `cargo test -p rustok-channel invalidation_generation --lib`
- `cargo test -p rustok-channel sqlite_triggers_advance_generation_and_replay_preserves_it --lib`
- `cargo test -p rustok-server channel_cache_invalidation --lib`
- `cargo test -p rustok-server --test channel_cache_architecture_guard`
- `cargo test -p rustok-server --test channel_cache_resolved_value missed_publication_refreshes_remote_resolved_value_via_durable_poll`
- `RUSTOK_CHANNEL_TEST_POSTGRES_URL=postgres://... cargo test -p rustok-channel --test postgres_invalidation_generation -- --ignored --nocapture --test-threads=1`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://... cargo test -p rustok-server redis_publication_drives_remote_replica_readiness_recovery --lib -- --ignored --nocapture --test-threads=1`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://... cargo test -p rustok-server --test channel_cache_resolved_value -- --ignored --nocapture --test-threads=1`
- `cargo clippy -p rustok-channel --lib -- -D warnings`
- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- Targeted Redis disconnect/reconnect, listener-lag, database recovery and
  policy-lifecycle tests.

## References

- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)
- [Cache operations and recovery runbook](../../rustok-cache/docs/operations.md)

## Change rules

1. Keep resolution precedence and policy ownership in this module.
2. Keep durable generation allocation in the same database transaction as the
   channel mutation; PubSub must never become the source of truth.
3. Update local docs, `rustok-module.toml`, server middleware docs, and route
   selection documentation with a public contract change.
4. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
