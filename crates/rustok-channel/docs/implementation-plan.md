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
fail-safe bypass on allocator exhaustion. Its invalidation is currently
process-local; another replica may retain the previous resolution until the
60-second TTL expires unless the owner adopts a durable generation.

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

1. **Approve or replace the cross-replica cache stale bound.** Decide explicitly
   whether the current 60-second TTL is an acceptable bound after a channel or
   policy mutation. If it is not, reserve a channel-resolution generation in the
   committing transaction or consume a persisted event offset on every replica;
   rotate the affected tenant token before acknowledging recovery and perform a
   full clear on an unverified first event or gap.
   **Depends on:** channel mutation ownership and an approved durable delivery
   source; process-local middleware invalidation alone is insufficient.
   **Done when:** the owner documentation names the accepted stale bound, and
   multi-replica evidence proves either bounded TTL convergence or durable
   generation recovery during missed-event scenarios.

2. **Collect full runtime evidence for channel resolution.** Exercise
   `ChannelReadPort` and server middleware with real locale/OAuth facts, policy
   selection, inactive/degraded behavior, cache isolation, generation rollover,
   and the approved cross-replica behavior before promotion beyond
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
- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- Targeted server middleware, generation rollover, multi-replica convergence,
  and policy-lifecycle tests.

## References

- [Host cache contract inventory](../rustok-cache/docs/host-cache-inventory.md)

## Change rules

1. Keep resolution precedence and policy ownership in this module.
2. Update local docs, `rustok-module.toml`, server middleware docs, and route
   selection documentation with a public contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
