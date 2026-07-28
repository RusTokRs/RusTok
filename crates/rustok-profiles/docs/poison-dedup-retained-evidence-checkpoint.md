# Profiles checkpoint: retained external-Iggy dedup evidence

## Status

The external-Iggy dedup behavior source scenarios are already defined for four separately reviewed broker modes:

- disabled: repeated immutable `A` produces `0 -> 1 -> 2`;
- enabled: immediate repeated `A` produces `0 -> 1 -> 1`;
- one-entry capacity: `A, A, B, A` produces `0 -> 1 -> 1 -> 2 -> 3`;
- expiry: `A, A, bounded wait, A` produces `0 -> 1 -> 1 -> 2`.

This checkpoint adds the retained-execution layer. The following are source-complete:

- versioned execution contract;
- four exact per-case Cargo commands;
- required distinct broker addresses and reviewed external configuration files;
- parser for `[system.message_deduplication]` with scenario-specific validation;
- canonical non-secret configuration SHA-256;
- clean-commit, unchanged-source, and post-run clean-tree gates;
- skip/zero-test rejection;
- atomic evidence packet writing;
- strict verifier with pending and executed modes.

The canonical execution JSON is intentionally absent. External-Iggy runtime execution remains maintainer work.

## Privacy and ownership boundary

Profiles never authorizes presentation from:

- Iggy deduplication configuration or observed message counts;
- broker addresses or server artifact labels;
- deterministic delivery UUIDs;
- DLQ state or receipt state;
- retained execution packets or their digests.

`followers_only` remains resolved exclusively through authoritative Social Graph owner ports. Index, broker, receipt, metric, and evidence state never authorize visibility.

The retained packet omits broker addresses, configuration paths, full config contents and full-file hashes, credentials, connection strings, raw test logs, delivery UUIDs, payloads, and source coordinates. It stores only bounded environment-variable names, reviewed server artifact labels, canonical non-secret dedup settings/digests, source/output hashes, exact command arrays, timestamps, toolchain versions, and aggregate per-case pass results.

## Remaining evidence

This checkpoint does not prove:

- server configuration readback through Iggy;
- that the reviewed dedup window covers maximum publication lease, process restart, reconnect, or operator recovery horizons;
- a PostgreSQL receipt/Iggy publication transaction;
- physical exactly-once publication;
- bundled mode, TLS/authentication/failover, or multi-replica behavior.

Next evidence must execute the clean-commit runner against four reviewed disposable brokers, review and commit the generated packet, then compare `max_entries` and `expiry` with the maximum production recovery horizon. If that horizon cannot be guaranteed and monitored, production requires a stronger database-owned outbox or broker-transaction design before claiming stronger duplicate suppression.

## Maintainer commands

```bash
node scripts/verify/verify-iggy-contract-poison-external-dedup-evidence.mjs
node scripts/verify/verify-iggy-contract-poison-external-dedup-retained-evidence.mjs

# After supplying all required address/config/artifact environment variables:
node scripts/evidence/capture-iggy-contract-poison-external-dedup.mjs
node scripts/verify/verify-iggy-contract-poison-external-dedup-retained-evidence.mjs
```

Tests, Cargo commands, source verifiers, external Iggy scenarios, and server configuration changes were not executed while authoring this checkpoint.
