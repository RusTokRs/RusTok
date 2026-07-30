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

Cycle-001 source changes add database-owned fail-closed invariants for channel
selection and tenant relations. PostgreSQL and SQLite serialize default-channel,
active-policy-set and primary-target promotion; host target claims are rebuilt
from authoritative channel rows and uniquely scoped by tenant; OAuth bindings
and policy actions reject cross-tenant relations and incompatible parent tenant
moves. Historical duplicates or mismatches block the migration instead of being
silently rewritten. These changes remain source-complete, not compiled/live
verified, until the current migration and cache workflows finish on one SHA.

The source now includes eleven durable-recovery evidence layers:

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
- A deterministic local-broadcast test exceeds the 256-message buffer, observes
  `RecvError::Lagged`, proves the runtime fails closed while durable state is
  absent, and recovers readiness only after database reconciliation succeeds.
- A combined serving-path lag test caches an old channel name, subscribes both
  the real worker and a probe at the same cursor, publishes 300 no-Redis events
  without a suspension point, confirms `RecvError::Lagged` with two subscribers,
  observes not-ready plus the stale Axum value, restores only durable state and
  requires both readiness and the resolved channel name to recover without a
  replacement fast-path publication.
- A resolved-value database-state-loss test commits a new channel value, removes
  the generation table before reconciliation, observes critical readiness
  degrade, restores the persisted generation and requires the Axum replica to
  return the new value.
- A resolved-value generation-regression test first applies a forward epoch,
  rewinds the persisted epoch, commits a new channel value and requires the next
  reconciliation to rebuild the namespace instead of retaining the old value.
- An ignored self-hosted Redis restart test starts two existing replicas, stops
  Redis, proves polling convergence during the outage, restarts Redis, waits for
  both original subscriptions to return through `PUBSUB NUMSUB`, and then
  requires a new resolved value to arrive within three seconds before the next
  database poll.

The durable cross-replica contract, normal delivery, missed publication, local
listener lag, database loss/recovery, generation regression and Redis
restart/reconnect scenarios are source-complete. The cache-owned live suite also
uses Redis 7 `CLIENT PAUSE` to protect the shared two-second operation timeout,
fast circuit-open rejection and half-open recovery contract. None of these are
compiled or live verified on the current revision until the permanent cache
workflow reports successful compiled, PostgreSQL and Redis jobs.

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

1. **Move channel display copy to locale-attributed storage.** `channels.slug`
   remains the language-neutral identity, while `channels.name` is still
   human-facing copy in the base row. Add tenant-composite
   `channel_translations`, backfill legacy copy with truthful provenance, require
   a host-resolved effective locale for writes, and return exact-locale/fallback
   projections consistently from REST, native server functions, owner ports,
   cache values and admin UI. Policy-set display names must be classified and
   cut over in the same owner migration if they are tenant-visible copy.
   **Depends on:** the accepted multilingual database contract and translation
   control-plane boundary.
   **Done when:** base rows are language-neutral, locale columns are at least
   `VARCHAR(32)`, writes are atomic with translations, and runtime tests cover
   requested -> tenant default -> first available selection without returning
   storage-only `und` as a request fallback.

2. **Execute the permanent durable cache gate.** Run the source-complete SQLite,
   server two-replica, lagged-listener resolved-value, PostgreSQL, Redis readiness,
   Redis restart and cache-owned latency/circuit scenarios on one reconciled
   `main` revision, then fix every format, compile, test or Clippy failure before
   recording the revision as verified.
   **Depends on:** GitHub Actions visibility or another Rust 1.96 build
   environment with ephemeral PostgreSQL, Redis 7 and `redis-server`.
   **Done when:** `compiled-contract`, `postgres-channel`, and `live-redis` pass
   on the same revision and the result is recorded without copying raw logs.

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
- `cargo test -p rustok-channel --lib`
- `cargo test -p rustok-channel invalidation_generation --lib`
- `cargo test -p rustok-channel sqlite_triggers_advance_generation_and_replay_preserves_it --lib`
- `cargo test -p rustok-server channel_cache_invalidation --lib`
- `cargo test -p rustok-server --test channel_cache_architecture_guard`
- `cargo test -p rustok-server --test channel_cache_resolved_value`
- `RUSTOK_CHANNEL_TEST_POSTGRES_URL=postgres://... cargo test -p rustok-channel --test postgres_invalidation_generation -- --ignored --nocapture --test-threads=1`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://... RUSTOK_CACHE_REDIS_SERVER_BIN=/usr/bin/redis-server cargo test -p rustok-server --test channel_cache_resolved_value -- --ignored --nocapture --test-threads=1`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://... cargo test -p rustok-server redis_publication_drives_remote_replica_readiness_recovery --lib -- --ignored --nocapture --test-threads=1`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://... cargo test -p rustok-cache --test real_redis_hardening -- --ignored --nocapture --test-threads=1`
- `cargo clippy -p rustok-channel --lib -- -D warnings`
- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- Targeted policy-lifecycle and migration-invariant tests.

## References

- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)
- [Cache operations and recovery runbook](../../rustok-cache/docs/operations.md)
- [Multilingual database contract audit](../../../docs/architecture/database-multilingual-audit.md)

## Change rules

1. Keep resolution precedence and policy ownership in this module.
2. Keep durable generation allocation in the same database transaction as the
   channel mutation; PubSub must never become the source of truth.
3. Update local docs, `rustok-module.toml`, server middleware docs, and route
   selection documentation with a public contract change.
4. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
5. Keep channel base rows language-neutral; tenant-visible display copy belongs
   to locale-attributed owner translations.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-07-30`
- Scope inspected: `channel ownership and transport boundaries; tenant-scoped reads/writes; OAuth and policy relation integrity; default/active/primary selection concurrency; host claim uniqueness; durable invalidation and migration replay; cache dimensions; multilingual database contract`
- Findings: `P0=0, P1=5, P2=0, P3=1`
- Fixed in this pass: `added DB-owned single-default, single-active-policy and single-primary promotion; serialized tenant-scoped host claims; rejected cross-tenant OAuth/policy relations and unsafe parent moves; added fail-fast historical preflights, replay-safe derived-state rebuilds and SQLite regressions; isolated the legacy o_auth_apps fixture normalization to cfg(test)`
- Remaining risks or blockers: `P1 channel-display-name multilingual cutover remains open; compile, migration, PostgreSQL and Redis jobs on the current SHA are queued and no pass is claimed`
- Evidence: `owner service, REST/native adapters, ChannelReadPort, navigation consumer, resolution pipeline, all channel migrations, durable invalidation tests, cache workflow, migration workflow and the multilingual database audit were inspected; PR #2469 contains the current source changes`
- Next action: `inspect the first completed workflow jobs, fix every channel-specific failure, then mark the item blocked for the multilingual cutover or complete only if that P1 is resolved`
- Resume command: `cargo xtask module validate channel && cargo xtask module test channel && cargo test -p rustok-channel --lib`
