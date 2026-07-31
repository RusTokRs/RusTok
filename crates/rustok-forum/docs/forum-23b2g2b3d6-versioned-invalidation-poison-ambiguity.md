# FORUM-23B2G2B3D6 external-Iggy poison publish/mark ambiguity proof

## Status

`source_ready_maintainer_execution_pending`

This slice continues the frozen Forum Search runtime-evidence matrix after merged
D5 semantic-poison proof #2777. It isolates the remaining raw-poison ambiguity
window where the broker accepts a deterministic DLQ publication but the process
stops before PostgreSQL records `published`.

The machine-readable contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-poison-ambiguity-source-proof.json
```

The executable target is:

```text
crates/rustok-search/tests/forum_versioned_invalidation_poison_ambiguity_iggy.rs
```

The production ordering reference is:

```text
apps/server/src/services/forum_search_contract_consumer.rs
```

## Runtime topology

The harness requires one PostgreSQL database and two distinct external Iggy
instances:

- `RUSTOK_FORUM_SEARCH_POISON_DEDUP_ENABLED_IGGY_ADDRESS` points to an instance
  whose server-side deterministic message-ID duplicate suppression is enabled;
- `RUSTOK_FORUM_SEARCH_POISON_DEDUP_DISABLED_IGGY_ADDRESS` points to a distinct
  instance whose duplicate suppression is disabled.

Both addresses must use bounded `host:port` form and must differ. Optional shared
credentials use `RUSTOK_IGGY_EXTERNAL_TEST_USERNAME` and
`RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD` and must be supplied together.

Each mode creates a unique one-partition evidence stream and an isolated
PostgreSQL schema. The real connector migration list creates the durable receipt
store. Source deliveries use the exact production consumer group
`rustok-search-forum-projection-v1` and topic `domain`; DLQ publications use the
production `IggyTransport::move_to_dlq` path.

## Covered sequence

For each broker mode, two malformed source payloads are published. The first
payload follows this exact sequence:

```text
receive exact raw poison delivery
  -> reserve_and_claim durable receipt
  -> receipt state is publishing
  -> publish deterministic DLQ entry
  -> do not call mark_published
  -> retain the exact source offset unacknowledged
  -> second publisher observes Busy before lease expiry
  -> stop the first transport and consumer group
  -> wait for the bounded one-second publish lease to expire
  -> reconstruct transport and production consumer group
  -> redeliver the same bytes, offset and delivery UUID
  -> reclaim the expired receipt with a new publisher
  -> stale publisher fails mark_published with ClaimLost
  -> republish with the same deterministic broker message ID
  -> inspect physical DLQ multiplicity
  -> mark_published
  -> acknowledge the exact source delivery
  -> mark_acknowledged
  -> advance to the second malformed source payload
```

The dedup-enabled instance must contain exactly one physical DLQ message after the
retry. The dedup-disabled instance must contain exactly two. This is a deliberate
positive/negative capability proof: PostgreSQL receipt leasing fences concurrent
live publishers, but cannot create physical exactly-once semantics across a
successful broker write and a missing database mark without broker deduplication.

## Generated evidence

Only after both modes pass, the target writes:

```text
target/forum-search-versioned-invalidation-poison-ambiguity-evidence.json
```

The artifact records the exact source commit, consumer group, source positions,
deterministic delivery and DLQ identities, claim takeover, stale-publisher
fencing, expected and observed physical message counts, receipt terminal state,
and advancement to the next source delivery. It is executable output and must not
be hand-edited or committed as a static result.

## Relationship to merged D5

`FORUM-23B2G2B3D5` merged through PR #2777 at
`1d2654785047efde1d29a9682732990acc56f9b5`. D5 owns valid typed semantic
identity-conflict poison, deterministic semantic DLQ publication and
`AlreadyPublished` restart recovery. D6 does not duplicate that scenario; it
uses malformed raw bytes solely to expose the publish-before-`mark_published`
ambiguity under two reviewed broker modes.

The parent D0 contract already registers D5. D6 intentionally defers its own
registration until D6 is merged, so canonical `main` never lists an unmerged
subproof.

## Deliberate limits

This slice does not claim:

- successful PostgreSQL or external-Iggy execution;
- server-owned background worker-loop or retry/backoff execution;
- active production Iggy deduplication configuration readback;
- semantic identity-conflict poison, which is already owned by merged D5;
- arbitrary multi-process contention beyond one lease-expiry takeover;
- TLS, replication, multiple partitions or broker failover;
- Search projector execution, owner-checkpoint advancement, missing-delivery
  repair, deletion/ACL visibility ordering, Search-disabled recovery, completion
  of `FORUM-23B2G2B3D`, or closure of `LINK-FORUM-03`.

No production Rust path, migration, event schema, digest, runtime flag, consumer
group, broker topic, Search query, public API or `Cargo.lock` entry changes. The
only manifest change is the direct test-only `iggy` dependency already present in
the workspace lock graph.

## Maintainer verification

```bash
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
RUSTOK_FORUM_SEARCH_POISON_DEDUP_ENABLED_IGGY_ADDRESS="127.0.0.1:8090" \
RUSTOK_FORUM_SEARCH_POISON_DEDUP_DISABLED_IGGY_ADDRESS="127.0.0.1:8091" \
  cargo test -p rustok-search \
  --test forum_versioned_invalidation_poison_ambiguity_iggy \
  -- --nocapture --test-threads=1

node scripts/verify/verify-forum-search-versioned-invalidation-d6-poison-ambiguity.mjs
cargo check -p rustok-search --all-targets
cargo xtask module validate forum
cargo xtask module validate search
git diff --check
```

No command above was run by the implementation agent, per maintainer request.
