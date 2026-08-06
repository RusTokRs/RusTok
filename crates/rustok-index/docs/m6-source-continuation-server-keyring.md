# M6 server-owned source continuation keyring

Status: `source_complete_owner_execution_pending`.

## Purpose

`IndexSourceContinuationKeyringRuntime` supplies exact 32-byte AES keys to the database-neutral
continuation codec without embedding raw material in settings, module extensions, logs, errors, or
debug output.

It retains only one active key ID, one lifetime in 1 through 900 seconds, at most 16
key-ID-to-`SecretRef` mappings, and one process-owned `SecretResolverRegistry`.

## Configuration

`RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON` is read only when a frozen Index source registry
exists. The raw JSON is rejected above 16 KiB before deserialization.

This slice admits deployment-owned `env` and `mounted_file` aliases. Mounted-file references require
`RUSTOK_INDEX_SOURCE_CONTINUATION_SECRET_MOUNT_ROOT`.

Key IDs are bounded to 64 bytes. Reference keys are bounded to 256 bytes, use a restricted ASCII
syntax, and must be unique with their resolver aliases. The active key must be present.

## Secret wire format

Each secret is canonical URL-safe unpadded base64. The encoded value must contain exactly 43 bytes
before decoding and must decode to exactly 32 bytes.

Missing, forbidden, malformed, noncanonical, or wrong-length values fail closed without exposing the
resolver cause, reference key, secret value, or decoded material.

## Resolution timing

Composition validates configuration shape, bounds, active-key presence, uniqueness, aliases,
lifetime, and resolver policy synchronously.

`diagnose_source_page_sealed` resolves every reference asynchronously after authorization and limit
validation but before token parsing or source scan. The decoded map constructs one short-lived
`IndexSourceContinuationCodec`, which opens the incoming token and seals the outgoing cursor before
the map and codec are dropped.

The keyring is passed privately into `IndexDriftSourcePageDiagnosisRuntime` and is not inserted as a
separately retrievable extension capability.

## Rotation

One active key seals. Retained non-active keys decrypt existing tokens. After adding and activating a
new key, deployment must wait longer than maximum token lifetime plus operational skew before removing
the old key.

A token naming a removed key fails closed before cursor return.

## Sealed service and GraphQL boundary

`diagnose_source_page_sealed` authorizes, derives canonical scope, resolves keys, opens the incoming
token before scan-request construction, diagnoses exactly one page, and seals the outgoing cursor.

`diagnoseIndexSourcePage` is the only public source-page transport in this slice. It accepts one
bounded schema identity, one limit, and one optional opaque token. It delegates exactly once to the
sealed method and returns only counters, finding receipts, completion state, and an opaque token.

No raw cursor, source entity ID, owner/index payload, secret reference, or key material crosses the
GraphQL resolver.

## Deliberate limits

This slice does not add:

- persisted cursor state or multi-page jobs;
- background scanning, scheduling, lifecycle commands, or repair;
- cloud, Vault, or Kubernetes resolver configuration specific to this Index keyring;
- retained secret-resolution, rotation, expiry, authorization, database, GraphQL, workflow, or CI
  evidence.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-server index_source_continuation_runtime -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-source-continuation-server.mjs
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-drift-source-page-graphql-transport.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, cryptographic integration, GraphQL scenarios,
workflows, or CI were run by the implementation agent.
