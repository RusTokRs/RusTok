# Profiles checkpoint: retained external DLQ duplicate scan evidence

Status: **retained tooling source-complete; canonical runtime packet absent**.

## New evidence boundary

The external-Iggy physical duplicate scan now has a clean-commit retained path:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-execution-contract.json
scripts/evidence/capture-iggy-dlq-duplicate-external-scan.mjs
scripts/verify/verify-iggy-dlq-duplicate-external-scan-retained.mjs
```

The canonical execution packet is intentionally absent:

```text
crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-execution.json
```

No Profiles runtime, API, storage, presentation, or authorization code changed.

## Reviewed external service

A maintainer must provide:

- one external Iggy `host:port` through an environment variable;
- an absolute reviewed config path outside the repository;
- a bounded reviewed Iggy artifact/version label;
- optional paired credentials.

The runner reads only `system.message_deduplication.enabled` and requires `false`. It does not perform active server configuration readback.

The endpoint, credentials, config path, full config, unrelated settings, and full-file hash are not retained.

## Retained count-only assertions

The exact all-pass runtime case binds:

```text
total_messages = 4
unique_message_ids = 2
duplicate_messages = 2
duplicate_groups = 2
conflicting_payload_groups = 1
max_copies_per_message_id = 2
```

It also binds three aggregate absent-offset assertions:

```text
before fixture publication = absent
after first scan = absent
after second scan = absent
```

No actual partition, offset, message UUID, payload, or payload digest is included.

## Clean-commit and freshness boundary

The runner requires a clean working tree and a full Git commit before execution. It hashes current production, scanner, classifier, runtime harness, contracts, runner, and verifier sources.

After the exact test passes, it requires:

- unchanged `HEAD`;
- unchanged source SHA-256 values;
- a still-clean working tree;
- atomic packet creation.

The strict verifier rejects a packet generated from another commit or stale source hashes.

## Profiles authorization remains unchanged

No profile visibility, ownership, audience, relationship, block, mute, follow, friendship, or presentation decision may depend on:

- the reviewed broker artifact or config digest;
- whether the retained runner executed;
- duplicate or conflicting-payload counts;
- absent-offset assertions;
- source/output hashes;
- execution timestamps or toolchain metadata.

Profiles continues to authorize through owner ports. This packet is operational evidence about downstream neutralization only.

## Privacy boundary

A retained packet may contain only:

- environment-variable names;
- bounded reviewed artifact label;
- canonical `enabled=false` configuration and digest;
- Git/toolchain/timestamps;
- exact command provenance;
- current source SHA-256 values;
- aggregate summary and absent-offset assertions;
- one `pass` result and output digest/size.

It excludes endpoints, credentials, config paths/content, raw logs, stream names, partitions, offsets, UUIDs, payloads, acknowledgement tokens, and raw Iggy errors.

## Remaining work

1. execute the exact case against a reviewed dedup-disabled disposable broker;
2. review and commit the canonical packet only after success;
3. re-execute whenever a bound source hash changes;
4. define alert thresholds outside Profiles;
5. keep acknowledgement/delete/replay reconciliation separate and explicitly authorized;
6. preserve identifier-free aggregate comparison with poison receipt health.

Tests, Cargo commands, formatters, source verifiers, external-Iggy scans, and retained capture were not run by the implementation agent.
