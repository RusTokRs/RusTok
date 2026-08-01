# FORUM-23B2G2B3D8 host worker retry lifecycle proof

## Status

`source_ready_maintainer_execution_pending`

This slice continues the frozen Forum Search runtime-evidence matrix after merged
D7 multi-process serialization proof #2788. D8 executes the real server-owned typed
contract consumer through its public startup API. It does not call the private
processing helpers directly and does not substitute a component-level retry
loop.

The machine-readable contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-host-worker-retry-proof.json
```

The executable target is:

```text
apps/server/tests/forum_versioned_invalidation_host_worker_retry_iggy.rs
```

The production lifecycle path is:

```text
start_forum_search_contract_consumer_if_enabled
  -> tokio::spawn(forum_search_contract_consumer_loop)
  -> PersistentContractConsumerGroup::receive_delivery
  -> process_contract_event
  -> ForumSearchContractIngress::ingest
  -> retry_delay / wait_or_stop
  -> acknowledge
```

## Runtime topology

The proof requires PostgreSQL and one external Iggy instance:

- `RUSTOK_SEARCH_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` fallback;
- `RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS`;
- optional paired `RUSTOK_IGGY_EXTERNAL_TEST_USERNAME` and
  `RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD`.

It creates an isolated Search schema, applies the production Search migrations
and uses a unique one-partition Iggy stream. Source deliveries use production
consumer group `rustok-search-forum-projection-v1` and topic `domain`.

The test builds a real `ServerRuntimeContext`, inserts the exact configured
`Arc<IggyTransport>` and an `EventRuntime` whose delivery profile is
`outbox_iggy`, then calls the public server startup function. The worker flag is
enabled only for this serialized test and restored afterwards.

No dependency manifest or `Cargo.lock` change is needed because the server host
already owns Search, Iggy transport, connector and PostgreSQL dependencies.

## Exact retry exhaustion

Before worker startup, the isolated schema installs:

1. a PostgreSQL sequence named `forum_search_worker_retry_attempts`;
2. a `BEFORE INSERT` trigger on `search_projection_inbox`;
3. a trigger function that advances the sequence and raises SQLSTATE `40001`.

The insert transaction rolls back, while PostgreSQL sequence advancement is
non-transactional. This preserves an exact externally observable count of
production admission attempts without changing production code or adding a test
hook.

The worker is configured with:

```text
max_attempts = 3
base_backoff = 25 ms
max_backoff = 50 ms
```

The first valid typed delivery must therefore produce exactly three sequence
advances. After the third retryable storage failure, the production worker must
finish, the Search inbox must still contain zero rows and the broker offset must
remain uncommitted.

## Restart and redelivery

The test removes only the injected trigger and function, takes the finished
worker handle from `ServerRuntimeContext`, and invokes the same public startup
function again.

The restarted worker must have a different lifecycle instance ID. It receives
the unacknowledged first delivery and continues through four valid caused Forum
invalidations. Every exact legacy root event ID must appear once in
`search_projection_inbox`.

Reaching later deliveries demonstrates that the fixed production consumer group
acknowledged and advanced beyond the recovered first delivery. After graceful
shutdown, reopening the same group must expose no remaining event or decode
failure.

## Stop-aware idle lifecycle

The test configures:

```text
RUSTOK_FORUM_SEARCH_CONTRACT_CONSUMER_IDLE_POLL_MS=5000
```

After all four deliveries have been processed and the worker is idle, the
host-owned shared `StopHandle` is signalled. The worker must finish within one
second, proving that shutdown preempts the five-second idle poll rather than
waiting for the sleep to expire or aborting the task externally.

## Generated evidence

Only after the scenario, transport shutdown and PostgreSQL schema cleanup
succeed, the target writes:

```text
target/forum-search-versioned-invalidation-host-worker-retry-evidence.json
```

The artifact records the exact source commit, both worker instance IDs,
configured and observed retry counts, inbox counts before and after restart,
root event IDs, consumer-group emptiness, configured idle poll and measured stop
latency. It must not be hand-edited or committed as a static result.

## Relationship to D7 and parked ambiguity evidence

`FORUM-23B2G2B3D7` merged through PR #2788 at
`ed5bdacfdbf8107f3a8f4eed39b705d455a85c63` and owns multi-process
serialization. The parent D0 contract already registers D7.

Closed PR #2783 contains a useful publish-before-`mark_published` ambiguity
harness, but it remains parked because its retained source still identifies
itself as D6. D8 does not claim or duplicate that ambiguity proof.

D8 intentionally defers its own D0 registration until D8 is merged, so
canonical `main` never lists an unmerged subproof.

## Deliberate limits

This slice does not claim:

- successful PostgreSQL or external-Iggy execution;
- injected acknowledgement failure after durable admission, already covered by
  D3;
- raw or semantic poison terminalization, owned by D4 and D5;
- publish-before-`mark_published` ambiguity resolution;
- missing-delivery owner repair, owned by D6;
- multi-process advisory-lock and scan-cursor contention, owned by D7;
- arbitrary Iggy consumer-group contention;
- deletion/ACL ordering, Search-disabled recovery or storefront visibility;
- completion of `FORUM-23B2G2B3D` or closure of `LINK-FORUM-03`.

No production Rust path, migration, event schema, digest, runtime flag,
consumer-group identity, broker topic, Search query, public API, dependency
manifest or `Cargo.lock` entry changes.

## Maintainer verification

```bash
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS="127.0.0.1:8090" \
  cargo test --locked -p rustok-server \
  --test forum_versioned_invalidation_host_worker_retry_iggy \
  -- --nocapture --test-threads=1

node scripts/verify/verify-forum-search-versioned-invalidation-d8-host-worker-retry.mjs
cargo check --locked -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
git diff --check
```

No command above was run by the implementation agent, per maintainer request.
