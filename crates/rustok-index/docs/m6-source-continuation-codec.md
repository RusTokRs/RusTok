# M6 confidential source continuation codec

Status: `source_complete_locale_scope_owner_execution_pending`.

## Purpose

`IndexSourceCursor` is an owner-controlled JSON value bounded to 8 KiB. It is safe for an internal
checkpoint store but not for public transport because it can contain owner identifiers, partition
positions, or other implementation details.

`IndexSourceContinuationCodec` provides a transport-neutral authenticated and confidential envelope.
The server-owned keyring, sealed source-page service, and bounded GraphQL transports are composed
separately around this database-neutral contract.

## Canonical scope

`IndexSourceContinuationScope` binds:

- non-nil tenant UUID;
- exact `SchemaRef`, including schema routing version;
- canonical owner module;
- canonical source name;
- source scan locale identity: schema-wide (`None`) or one exact canonical `LocaleKey`.

`IndexSourceContinuationScope::from_registry(tenant_id, schema, sources)` constructs only the
schema-wide scope. `IndexSourceContinuationScope::for_locale(tenant_id, schema, locale, sources)`
constructs only the exact canonical-locale scope. Both source identities come from the frozen source
registry rather than caller-controlled source fields.

Opening compares every scope field before returning a raw cursor. A schema-wide token cannot open
under an exact-locale scope. A locale token cannot open schema-wide or under another locale. Locale
aliases/casing first canonicalize through `LocaleKey`, so equivalent locale input maps to one token
scope.

## Cryptographic envelope

The repository has one current unversioned continuation envelope. It uses AES-256-GCM and a fresh
96-bit operating-system nonce for every seal operation.

The URL-safe unpadded outer envelope contains only bounded key ID, nonce, authenticated ciphertext,
and the GCM tag. Encrypted claims contain tenant, exact schema, canonical owner/source, optional
canonical locale, issued-at, expiry, and the complete raw cursor.

The fixed domain string and key ID are authenticated as additional data. The cursor JSON, tenant,
schema, source identity, locale, timestamps, and entity-like positions are never cleartext.

There is no internal continuation version byte, version-tagged claim family, legacy decoder, or
fallback parser. This is a pre-release repository-owned contract: changing its shape replaces the
canonical encoder/decoder atomically and invalidates superseded token shapes rather than accumulating
parallel formats.

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

One key ID is active for sealing. Every retained key can decrypt tokens produced by the current
canonical envelope. A safe sequence is:

1. add a new key while retaining the old key;
2. switch the active ID;
3. wait longer than maximum token lifetime plus operational skew;
4. remove the old key.

A token naming a removed key fails closed. Codec `Debug` exposes only active ID and key count; token
`Debug` exposes only encoded length.

Key rotation is not format compatibility: retained keys decrypt only the one current envelope shape.

## Server composition and GraphQL boundary

The server validates bounded deployment configuration containing only key IDs and `SecretRef`
values. Secret material must be canonical URL-safe unpadded base64 decoding to exactly 32 bytes.

Because resolution is asynchronous, keys are resolved inside sealed server adapters before token
parsing or scan-request construction. One short-lived codec opens the incoming token and seals the
outgoing cursor.

`diagnoseIndexSourcePage` and the current schema-wide `runIndexReplayShadow` transport treat
continuation as an opaque bounded string, authorize before untrusted input parsing, and never expose
raw cursor, keyring handle, secret reference, or decoded key material.

The locale-safe scope is source-complete. Exact-locale Shadow transport remains a separate next slice:
it must carry canonical locale through the dry-run scan request and choose `for_locale` rather than
changing or bypassing the continuation codec.

## Deliberate limits

This slice does not add or claim:

- persisted continuation state, multi-page scheduling, or restart jobs;
- exact-locale Shadow GraphQL input or dry-run locale execution;
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
node scripts/verify/verify-index-replay-shadow-graphql-transport.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, secret-resolver or GraphQL scenarios, workflows, or CI
were executed by the implementation agent.
