# M6 confidential source continuation codec

Status: `source_complete_server_composition_complete_transport_pending`.

## Purpose

`IndexSourceCursor` is an owner-controlled JSON value bounded to 8 KiB. It is safe for an internal
checkpoint store, but it is not a public continuation token: its JSON can contain owner identifiers,
partition positions, or other implementation details.

`IndexSourceContinuationCodec` adds a transport-neutral authenticated and confidential envelope. It
does not mount a GraphQL, HTTP, CLI, MCP, or native-admin operation and does not read configuration or
resolve secrets by itself.

Server-owned `SecretRef` composition and an internal sealed page method now exist separately. See
[M6 server-owned source continuation keyring](./m6-source-continuation-server-keyring.md).

## Canonical scope

`IndexSourceContinuationScope::from_registry(tenant_id, schema, sources)` is the only public scope
constructor. It resolves the frozen `IndexSourceDescriptor` and binds:

- the non-nil tenant UUID;
- the exact `SchemaRef`, including version;
- the canonical owner module;
- the canonical source name.

A caller cannot independently supply an owner/source identity that disagrees with replay
composition. Opening a token compares every scope field before returning its raw cursor.

## Cryptographic envelope

Version 1 uses AES-256-GCM and a fresh 96-bit operating-system nonce for every seal operation.

The URL-safe, unpadded outer envelope contains only:

1. one contract version byte;
2. one bounded key-id length byte;
3. a bounded key id;
4. the 96-bit nonce;
5. authenticated ciphertext and the GCM tag.

The encrypted claims contain:

- the contract version;
- tenant and exact schema;
- canonical owner/source identity;
- issued-at and expiry timestamps;
- the complete raw `IndexSourceCursor`.

The domain string, outer version, and key id are authenticated as additional data. Changing the
ciphertext, nonce, version, or a known key id fails authentication. The raw cursor JSON, tenant,
schema, source identity, timestamps, and entity-like owner positions are never present in
cleartext.

This contract is intentionally separate from the query `CursorCodec`, whose checksum detects
accidental corruption but is neither keyed nor confidential.

## Time and size bounds

- lifetime must be between 1 second and 15 minutes;
- issued-at may be at most 30 seconds ahead of the validating clock;
- expiry is strict and checked before cursor return;
- key ids are at most 64 bytes and use bounded lowercase machine-name syntax;
- the keyring contains 1 through 16 AES-256 keys;
- plaintext, decoded envelope, and encoded token each have independent hard limits;
- oversized encoded input is rejected before base64 decoding or decryption.

The lifetime is embedded inside authenticated ciphertext and revalidated during opening, so a caller
cannot extend expiry by editing the outer token.

## Key rotation

One configured key id is active for sealing. Every retained key can decrypt. A deployment can:

1. add a new key while retaining the previous key;
2. switch the active key id;
3. wait longer than the maximum configured token lifetime and operational skew;
4. remove the previous key.

A token naming a removed or otherwise unavailable key fails closed before source access. The codec
`Debug` implementation exposes only active key id and key count; token `Debug` exposes only encoded
length. Key bytes and token contents are not formatted.

## Server composition

The server now validates bounded deployment configuration containing only key IDs and `SecretRef`
values. Secret material is URL-safe unpadded base64 and must decode to exactly 32 bytes.

Because secret resolution is asynchronous, the server resolves keys inside
`diagnose_source_page_sealed` before parsing the incoming token or constructing
`IndexSourceScanRequest`. It builds one short-lived codec, opens the incoming token, executes one
page, seals the outgoing cursor, and drops the local key map.

The keyring is private to the page runtime and is not published as an independently retrievable
extension. Configuration, secret-resolution, and exact-length failures expose no raw resolver cause,
reference key, or key material.

## Deliberate limits

This slice does not add or claim:

- a public source-page transport;
- cloud/Vault/Kubernetes keyring configuration specific to this Index boundary;
- cursor persistence, multi-page scheduling, or restart state;
- stale Index-only or orphan-link discovery;
- finding lifecycle or repair;
- retained cryptographic, server, GraphQL, workflow, or CI execution evidence.

The next safe slice is one bounded transport over the existing sealed internal page method. It must
preserve request-bound authorization, accept and return only the opaque continuation, expose no raw
cursor or source entity identifiers, and delegate exactly once.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-server index_source_continuation_runtime -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-source-continuation-server.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, secret-resolver scenarios, workflows, or CI were
executed by the implementation agent.