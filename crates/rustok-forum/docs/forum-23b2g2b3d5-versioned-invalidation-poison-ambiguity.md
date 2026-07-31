# FORUM-23B2G2B3D5 external-Iggy poison ambiguity proof

## Status

`source_ready_maintainer_execution_pending`

This slice adds an executable raw-poison proof for the ambiguity window between a
successful DLQ publish and the durable `mark_published` transition. It follows the
merged D4 external-Iggy/PostgreSQL raw-poison restart proof in PR #2775 and the
merged D3 acknowledgement/restart proof in PR #2770.

The machine-readable contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-poison-ambiguity-source-proof.json
```

The executable target is:

```text
crates/rustok-search/tests/forum_versioned_invalidation_poison_ambiguity_iggy.rs
```

## Runtime topology

The harness requires one PostgreSQL database and two distinct external Iggy
instances:

- `RUSTOK_FORUM_SEARCH_POISON_DEDUP_ENABLED_IGGY_ADDRESS` points to an instance
  with deterministic message-ID duplicate suppression enabled;
- `RUSTOK_FORUM_SEARCH_POISON_DEDUP_DISABLED_IGGY_ADDRESS` points to an instance
  with that suppression disabled.

Both addresses must use `host:port` form and must differ. Optional shared
credentials use `RUSTOK_IGGY_EXTERNAL_TEST_USERNAME` and
`RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD`.

Each mode receives a unique one-partition evidence stream. The harness creates a
unique PostgreSQL schema, applies the real `rustok-iggy-connector` migration list,
and uses the production Forum Search consumer group
`rustok-search-forum-projection-v1` on topic `domain`.

## Covered sequence

For each Iggy mode the target publishes two malformed source payloads and performs
this exact sequence for the first delivery:

```text
receive exact raw poison delivery
  -> reserve_and_claim durable receipt
  -> publish deterministic DLQ entry
  -> simulate process loss before mark_published
  -> retain unacknowledged source offset
  -> wait for the bounded publish lease to expire
  -> reconstruct transport and consumer group
  -> receive the same bytes, offset and delivery identity
  -> reclaim the durable receipt
  -> reject the stale publisher with ClaimLost
  -> republish with the same deterministic broker message ID
  -> mark_published
  -> acknowledge the exact source delivery
  -> mark_acknowledged
  -> advance to the second malformed source payload
```

The dedup-enabled instance must contain one physical DLQ message after the retry.
The dedup-disabled instance must contain two physical messages. This difference is
recorded as broker capability evidence; it does not change the durable receipt
identity or the required published-before-ack ordering.

## Generated evidence

Only after both modes pass, the test writes:

```text
target/forum-search-versioned-invalidation-poison-ambiguity-evidence.json
```

The artifact records the exact source commit, generation time, consumer group,
stream identities, source offsets, deterministic delivery/message IDs, expected
and observed physical DLQ counts, stale-publisher fencing, receipt terminal state,
and advancement to the next source delivery. The artifact is executable output
and must not be hand-edited or committed as a static fixture.

## Relationship to D4

`FORUM-23B2G2B3D4` merged through PR #2775 at
`b612786020859dca377f2e32971b491fbd14644a`. The parent D0 contract already
registers that raw-poison restart proof. D5 remains a separate publish/mark
ambiguity proof and intentionally defers its own parent registration until D5 is
merged, so canonical `main` never lists an unmerged subproof.

## Deliberate limits

This slice does not claim:

- successful PostgreSQL or external-Iggy execution;
- execution of the server-owned Forum Search worker loop;
- semantic identity-conflict poison through external Iggy;
- arbitrary multi-process claim contention beyond one lease-expiry takeover;
- TLS, replication, multiple partitions or broker failover;
- projector execution, owner-checkpoint repair, visibility correlation, completion
  of `FORUM-23B2G2B3D`, or closure of `LINK-FORUM-03`.

No production migration, event schema, digest, runtime flag, consumer group,
broker topic, Search query, public API or `Cargo.lock` entry is changed. The only
new manifest entry relative to current `main` is the test-only direct `iggy`
dependency; the migration-enabled connector dependency is already owned by merged
D4.

## Maintainer verification

```bash
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
RUSTOK_FORUM_SEARCH_POISON_DEDUP_ENABLED_IGGY_ADDRESS="127.0.0.1:8090" \
RUSTOK_FORUM_SEARCH_POISON_DEDUP_DISABLED_IGGY_ADDRESS="127.0.0.1:8091" \
  cargo test -p rustok-search \
  --test forum_versioned_invalidation_poison_ambiguity_iggy \
  -- --nocapture --test-threads=1

node scripts/verify/verify-forum-search-versioned-invalidation-d5-poison-ambiguity.mjs
cargo check -p rustok-search --all-targets
cargo xtask module validate forum
cargo xtask module validate search
git diff --check
```

No command above was run by the implementation agent, per maintainer request.
