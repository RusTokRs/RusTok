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

This durable cross-replica contract is source-complete but not compiled or
multi-replica verified on the current revision until the permanent cache gate
and failure-recovery scenarios pass.

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

1. **Execute durable cross-replica cache recovery evidence.** The owner decision
   is implemented in source: database triggers reserve one global resolution
   generation in the committing transaction, PubSub is the fast path, and every
   serving replica periodically reconciles from the database before accepting a
   new baseline. Prove this behavior rather than reverting to a TTL-only stale
   bound.
   **Depends on:** the permanent compiled cache gate, migrated PostgreSQL and
   SQLite fixtures, multiple serving replicas, and controllable Redis failure.
   **Done when:** tests prove startup seeding, normal publication, concurrent
   mutations, dropped publication, listener lag, Redis disconnect/reconnect,
   database outage/recovery, generation regression handling, and terminal-worker
   readiness without serving a stale channel resolution beyond the documented
   five-second reconciliation interval.

2. **Collect full runtime evidence for channel resolution.** Exercise
   `ChannelReadPort` and server middleware with real locale/OAuth facts, policy
   selection, inactive/degraded behavior, cache isolation, generation rollover,
   and the durable cross-replica behavior before promotion beyond
   `boundary_ready`.
   **Depends on:** a composed server runtime and representative request fixtures.
   **Done when:** targeted Rust middleware/port tests provide reproducible
   runtime evidence for every published read and fallback profile.

3. **Extend channel-aware proof points only with owner evidence.** New domain
   reads must use the already resolved `ChannelContext`, local tests, and local
   documentation; they must not introduce a second channel-selection mechanism.
   **Depends on:** the consuming module's public contract.
   **Done when:** the proof-point verifier and affected module docs identify the
   same resolved-channel source and visibility behavior.

4. **Defer richer target or connector taxonomy until pressure is concrete.**
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
- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- Targeted server middleware, generation rollover, multi-replica convergence,
  and policy-lifecycle tests.

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
