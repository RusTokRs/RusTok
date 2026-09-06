# Profiles checkpoint: external DLQ duplicate scan runtime

Status: **runtime harness source-complete; external execution and retained evidence pending**.

## New source boundary

The bounded external-Iggy scanner now has one opt-in runtime harness:

```text
crates/rustok-iggy/tests/dlq_duplicate_external_scan.rs
```

Machine contract:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-runtime-source.json
```

Verifier:

```text
scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs
```

Operator guide:

```text
crates/rustok-iggy/docs/dlq-duplicate-external-scan-runtime-evidence.md
```

No Profiles service, GraphQL field, storefront behavior, relation policy, or authorization port changed.

## Controlled physical evidence

The harness requires a reviewed disposable external broker with message-ID deduplication disabled. It publishes four physical DLQ messages only through production `IggyTransport::move_to_dlq`:

```text
A, A: same deterministic header UUID and same exact bytes
B1, B2: same deterministic header UUID and different exact bytes
```

The expected count-only result is:

```text
total_messages = 4
unique_message_ids = 2
duplicate_messages = 2
duplicate_groups = 2
conflicting_payload_groups = 1
max_copies_per_message_id = 2
```

The conflict remains a manual-review signal. Neither the UUID nor the bytes are exposed by the result.

## Repeatable read-only scan

The same explicit request is executed twice:

```text
partition = 1
start offset = 0
maximum messages = 4
batch size = 4
```

The second summary must equal the first. This establishes the expected source shape for repeatable explicit-offset observation; execution remains pending and no complete-history claim is made.

## Stored-offset boundary

The harness reads the exact standalone scanner consumer offset before publication, after the first scan, and after the second scan. All three results must be absent.

This is read-only evidence around the scanner's existing:

```text
PollingStrategy::offset
auto_commit = false
```

The source contains no offset-store/delete call, acknowledgement, consumer-group cursor, direct SDK producer, stream deletion, purge, replay, or retry.

## Profiles authorization remains unchanged

No profile visibility, audience, ownership, relationship, block, mute, follow, friendship, or presentation decision may depend on:

- whether this harness ran;
- the selected broker, stream, partition, or offset;
- the count-only duplicate summary;
- an absent or present consumer offset;
- deduplication configuration;
- future retained execution metadata.

Profiles continues to authorize through owner ports. Physical duplicate observation is operational evidence only.

## Privacy boundary

The harness does not log or retain:

- broker address or credentials;
- generated stream name;
- partition offsets;
- physical header UUIDs;
- payload bytes or payload digests;
- raw Iggy errors.

A future retained packet must preserve this boundary and contain only bounded reviewed artifact metadata, source/output hashes, exact command provenance, and aggregate pass status.

## Remaining work

1. execute the harness against a reviewed dedup-disabled disposable external service;
2. prove the three absent-offset observations at runtime;
3. add clean-commit retained capture and strict current-source verification;
4. define alert thresholds outside Profiles;
5. keep acknowledgement/delete/replay reconciliation separate and explicitly authorized;
6. continue comparing receipt and physical duplicate health only as identifier-free aggregates.

Tests, Cargo commands, formatters, source verifiers, external-Iggy scans, and retained capture were not run by the implementation agent.
