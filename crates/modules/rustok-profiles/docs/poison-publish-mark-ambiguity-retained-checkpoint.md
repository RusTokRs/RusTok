# Profiles checkpoint: retained publish/mark ambiguity evidence

Status: **retained tooling source-complete; runtime packet pending**.

## Completed in this checkpoint

The source-level publish/mark ambiguity harness now has a fail-closed retained execution path:

```text
crates/rustok-social-graph/contracts/evidence/index-raw-poison-publish-mark-ambiguity-execution-contract.json
scripts/evidence/capture-social-graph-index-raw-poison-publish-mark-ambiguity.mjs
scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity-retained.mjs
```

The retained gate requires both exact PostgreSQL + external-Iggy scenarios from one clean commit:

```text
dedup enabled:  0 -> 1 -> 1
dedup disabled: 0 -> 1 -> 2
```

## Configuration evidence

The enabled scenario requires reviewed Iggy configuration with:

- `enabled = true`;
- `max_entries >= 1`;
- a positive expiry strictly longer than the 1500 ms lease-recovery wait.

The disabled scenario requires `enabled = false`.

Only the canonical `[system.message_deduplication]` projection and its SHA-256 are retained. Full config files, paths, unrelated settings, and full-file hashes are excluded.

## Runtime provenance

The future packet must retain:

- current Git commit;
- clean-tree status before execution;
- current source hashes for production code, tests, contracts, runner, and verifiers;
- exact command arrays;
- reviewed PostgreSQL and Iggy artifact labels;
- expected physical count sequences;
- two exact all-pass results;
- bounded output hashes and byte counts.

The runner rejects skipped or zero-test executions and writes the packet atomically only after both cases pass.

## Privacy boundary

The retained packet must not contain:

- PostgreSQL URLs;
- Iggy addresses;
- usernames or passwords;
- connection strings;
- config paths or full config contents;
- raw test logs;
- malformed payloads;
- source offsets;
- delivery UUIDs;
- acknowledgement tokens;
- temporary schema or stream names.

This evidence cannot be used as an authorization input. Profiles privacy, visibility, blocks, mutes, follows, and friendship policy continue to be decided by their authoritative owner ports before presentation.

## Interpretation boundary

A successful enabled case demonstrates physical duplicate suppression only for the exact isolated retry while the deterministic message ID remains in the reviewed dedup cache.

It does not prove:

- a PostgreSQL/Iggy transaction;
- physical exactly-once when deduplication is disabled;
- that production expiry and capacity cover every supported outage or load pattern;
- active configuration readback from the running broker;
- TLS, failover, bundled mode, or multi-replica ownership.

The disabled case intentionally preserves the negative result: after broker success and before `mark_published`, lease recovery can produce a second physical DLQ entry even though receipt ownership remains fenced.

## Remaining work

1. execute both exact scenarios against reviewed external services;
2. commit the canonical execution packet only after the retained verifier passes;
3. review production dedup expiry/capacity against the maximum supported recovery interval and load;
4. define operational handling for duplicate physical DLQ entries when dedup is disabled or exhausted;
5. add separate failover and multi-replica evidence.

`index-raw-poison-publish-mark-ambiguity-execution.json` is intentionally absent. Tests and verifiers were not run by the implementation agent.
