# M6 confidential source continuation codec

Status: `source_complete_owner_execution_pending`.

## Purpose

`IndexSourceCursor` is an owner-controlled JSON value bounded to 8 KiB. It is safe for an internal
checkpoint store but not for public transport because it can contain owner identifiers, partition
positions, or other implementation details.

`IndexSourceContinuationCodec` provides a transport-neutral authenticated and confidential envelope.
The server-owned keyring, sealed one-page service method, and bounded GraphQL transport are composed
separately around this database-neutral contract.

## Canonical scope

`IndexSourceContinuationScope::from_registry(tenant_id, schema, sources)` is the only public scope
constructor. It binds:

- non-nil tenant UUID;
- exact `SchemaRef`, including version;
- canonical owner module;
- canonical source name.

Opening compares every scope field before returning a raw cursor.

## Cryptographic envelope

Version 1 uses AES-256-GCM and a fresh 96-bit operating-system nonce for every seal operation.

The URL-safe unpadded outer envelope contains only version, bounded key ID, nonce, authenticated
ciphertext, and the GCM tag. Encrypted claims contain tenant, exact schema, canonical owner/source,
issued-at, expiry, and the complete raw cursor.

The domain string, version, and key ID are authenticated as additional data. The cursor JSON,
tenant, schema, source identity, timestamps, and entity-like positions are never cleartext.

This contract is intentionally separate from the checksum-only query `CursorCodec`.

## Time and size bounds

- lifetime is between 1 second and 15 minutes;
- accepted future clock skew is at most 30 seconds;
- key IDs are at most 64 bytes;
- keyring size is 1 through 16 AES-256 keys;
- plaintext, decoded envelope, and encoded token have independent limits;
- oversized input is rejected before decryption.

Authenticated lifetime claims are revalidated while opening.

## Key rotation

One key ID is active for sealing. Every retained key can decrypt. A safe sequence is:

1. add a new key while retaining the old key;
2. switch the active ID;
3. wait longer than maximum token lifetime plus operational skew;
4. remove the old key.

A token naming a removed key fails closed. Codec `Debug` exposes only active ID and key count; token
`Debug` exposes only encoded length.

## Server composition and GraphQL boundary

The server validates bounded deployment configuration containing only key IDs and `SecretRef`
values. Secret material must be canonical URL-safe unpadded base64 decoding to exactly 32 bytes.

Because resolution is asynchronous, keys are resolved inside `diagnose_source_page_sealed` before
token parsing or scan-request construction. One short-lived codec opens the incoming token and seals
the outgoing cursor.

The root GraphQL mutation `diagnoseIndexSourcePage` treats continuation as an opaque bounded string,
authorizes before schema/limit/token parsing, and delegates exactly once to the sealed method. No raw
cursor, keyring handle, secret reference, or decoded key material crosses the resolver.

## Deliberate limits

This slice does not add or claim:

- persisted cursor state, multi-page scheduling, or restart state;
- stale Index-only or orphan-link discovery;
- finding lifecycle or repair;
- retained cryptographic, secret-resolution, GraphQL, workflow, or CI execution evidence.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-server index_source_continuation_runtime -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-source-continuation-server.mjs
node scripts/verify/verify-index-drift-source-page-graphql-transport.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, secret-resolver or GraphQL scenarios, workflows, or CI
were executed by the implementation agent.
