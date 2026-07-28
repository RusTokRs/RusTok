# Profiles checkpoint: retained raw poison PostgreSQL/Iggy evidence

## Status

Capture tooling is source-complete; runtime evidence is pending.

The combined Social Graph raw-poison ordering harness now has a locked retained-execution path:

- clean-commit runner;
- two independent exact test commands;
- current production/test source SHA-256 binding;
- reviewed PostgreSQL and Iggy server artifact labels;
- atomic canonical packet writing;
- strict pending/executed verifier;
- privacy-safe aggregate packet projection.

The future canonical packet is:

```text
crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-execution.json
```

It remains absent until a maintainer successfully executes both cases.

## Profiles authorization boundary

Neither the source harness nor its retained packet authorizes presentation.

Profiles never authorizes from:

- broker delivery or DLQ state;
- source offset, acknowledgement token, or deterministic delivery UUID;
- neutral poison receipt state;
- PostgreSQL/Iggy artifact labels;
- source/output hashes;
- metrics, health snapshots, or evidence packet results.

`followers_only` continues to resolve only through authoritative Social Graph owner ports. Privacy is evaluated before localization, tags, summaries, and Media-backed descriptors. Restricted or unavailable public rows remain absent.

## Packet privacy

The retained packet omits database URLs, broker addresses, credentials, connection strings, schema/stream names, payloads, offsets, acknowledgement tokens, delivery UUIDs, and raw test output.

It retains only bounded provenance and aggregate pass evidence:

- commit and timestamps;
- Cargo/Rust versions;
- reviewed service artifact labels;
- environment-variable names for service endpoints;
- exact command arrays;
- source/output digests and output sizes;
- two all-pass ordering cases.

## Remaining work

- execute the clean-commit runner on reviewed PostgreSQL and disposable external Iggy;
- review and commit the canonical packet;
- repeat whenever a bound source hash changes;
- exercise broker-success before `mark_published` process loss;
- prove combined multi-replica claim ownership;
- establish production dedup-window sufficiency or adopt a stronger outbox/transaction design;
- retain independent Profiles privacy/storefront runtime evidence.

## Verification status

Tests, Cargo commands, formatters, source verifiers, PostgreSQL, external/bundled Iggy, and retained capture were not run while authoring this checkpoint.
