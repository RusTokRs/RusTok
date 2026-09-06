# Profiles checkpoint: raw poison PostgreSQL/Iggy ordering

## Status

Source-complete, runtime-pending.

This checkpoint records the combined PostgreSQL + external Iggy evidence boundary added after the separate connector receipt, cursor lifecycle, physical header, dedup behavior, and retained dedup tooling slices.

## Delivered source evidence

`rustok-social-graph/tests/index_raw_poison_postgres_iggy.rs` now defines two opt-in cases using the first approved raw-poison owner consumer:

1. **Published before source acknowledgement**
   - unique PostgreSQL schema with connector receipt migrations;
   - unique external Iggy stream and one partition;
   - production `SocialGraphIndexConsumer` typed receive;
   - neutral receipt `Claimed`/`publishing`;
   - exact-byte production DLQ publication;
   - receipt remains `publishing` after broker success;
   - durable `published` before source acknowledgement;
   - source acknowledgement before best-effort `acknowledged` bookkeeping;
   - next source offset becomes visible only afterward.

2. **Acknowledgement-only redelivery**
   - first process publishes and persists `published` but does not acknowledge the source;
   - a new transport and the same fixed consumer group receive the same offset, bytes, and UUID;
   - the receipt store returns `AlreadyPublished`;
   - recovery performs no second DLQ publication;
   - source acknowledgement and `acknowledged` bookkeeping complete;
   - the next source offset becomes visible.

The machine contract is:

```text
crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-source.json
```

The static parity guard is:

```text
node scripts/verify/verify-social-graph-index-raw-poison-postgres-iggy.mjs
```

It locks the harness order against `apps/server/src/services/social_graph_index_worker.rs`.

## Profiles boundary

This evidence never authorizes profile presentation.

Profiles still:

- never authorizes from an event, Index projection, broker offset, DLQ record, poison receipt, metric, or evidence packet;
- resolves `followers_only` only through authoritative Social Graph owner ports;
- evaluates privacy before localized/profile-media presentation;
- treats restricted or unavailable public rows as absent;
- keeps Media descriptors Media-owned.

The combined harness proves only the raw delivery terminalization order. It does not prove profile visibility, storefront behavior, follower policy, or presentation correctness.

## Remaining evidence

- execute this harness on reviewed PostgreSQL and disposable external Iggy;
- retain bounded toolchain, server artifact, source/output digest, and all-pass results;
- exercise the broker-success/`mark_published` ambiguity window;
- execute multi-replica claim ownership with the combined broker path;
- prove bundled mode, TLS/auth/failover, and production recovery-window sufficiency;
- retain independent Profiles privacy/storefront runtime evidence.

## Verification status

Tests, Cargo commands, formatters, source verifiers, PostgreSQL, external/bundled Iggy, and multi-replica scenarios were not run while authoring this checkpoint.
