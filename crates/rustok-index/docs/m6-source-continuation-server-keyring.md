# M6 server-owned source continuation keyring

Status: `source_complete_transport_and_owner_execution_pending`.

## Purpose

The database-neutral `IndexSourceContinuationCodec` requires exact 32-byte AES keys. The server
composition must provide those keys without embedding raw material in settings, module extensions,
logs, errors, or debug output.

`IndexSourceContinuationKeyringRuntime` is the deployment-owned bridge. It retains only:

- one bounded active key ID;
- one lifetime in 1 through 900 seconds;
- at most 16 key-ID-to-`SecretRef` mappings;
- one process-owned `SecretResolverRegistry`.

Raw key bytes are resolved only for a single call to
`IndexDriftSourcePageDiagnosisRuntime::diagnose_source_page_sealed`.

## Configuration

The process reads `RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON` only when a frozen Index source
registry exists. Example:

```json
{
  "active_key_id": "current",
  "lifetime_seconds": 300,
  "keys": {
    "current": {
      "resolver": "env",
      "key": "RUSTOK_INDEX_SOURCE_CONTINUATION_KEY_CURRENT"
    },
    "previous": {
      "resolver": "mounted_file",
      "key": "index/continuation/previous"
    }
  }
}
```

This slice admits only deployment-owned `env` and `mounted_file` aliases. Mounted-file references
require `RUSTOK_INDEX_SOURCE_CONTINUATION_SECRET_MOUNT_ROOT`.

Key IDs are lowercase machine names using ASCII letters, digits, `-`, `_`, or `.`, and are bounded to
64 bytes. References must be unique, complete, and permitted by an exact resolver policy. The active
key must be present in the key map.

## Secret wire format

Each referenced secret is URL-safe unpadded base64. Decoding must produce exactly 32 bytes.

A blank value, invalid base64, a value of any other length, missing resolver, forbidden reference,
missing secret, or codec construction failure fails closed. The public server error does not expose
the resolver cause, reference key, secret value, or decoded material.

## Resolution timing

Module runtime composition is synchronous while `SecretResolverRegistry` resolution is asynchronous.
Therefore:

- composition validates JSON shape, bounds, active-key presence, unique references, allowed resolver
  aliases, and resolver policy;
- `diagnose_source_page_sealed` resolves every configured reference before parsing an incoming token
  or constructing `IndexSourceScanRequest`;
- the decoded key map is used to construct one short-lived `IndexSourceContinuationCodec`;
- the codec opens the incoming token and seals the outgoing cursor within the same request;
- the local codec and key map are dropped after the call.

The keyring runtime is passed privately into `IndexDriftSourcePageDiagnosisRuntime`. It is not
inserted as a separately retrievable `ModuleRuntimeExtensions` capability.

## Rotation

One active key seals new continuations. Retained non-active keys are decrypt-only in practice because
`IndexSourceContinuationCodec` always seals with the configured active ID.

A safe rotation sequence is:

1. add the new key reference while retaining the old reference;
2. set the new key ID active;
3. wait longer than the maximum configured token lifetime plus operational clock skew;
4. remove the old reference.

A token naming a removed key fails closed before cursor return.

## Sealed page boundary

`diagnose_source_page_sealed(context, schema, continuation, limit)`:

1. authorizes the request-bound tenant and actor;
2. validates the bounded page limit;
3. derives canonical scope from the frozen source registry;
4. resolves the keyring and constructs the local codec;
5. opens the incoming token before scan-request construction;
6. diagnoses exactly one page;
7. seals the outgoing cursor before returning.

Its result contains an optional opaque token, bounded page counters, and bounded missing-finding
receipts. It contains no raw cursor, source entity ID, owner/index payload, secret reference, or key
material.

## Deliberate limits

This slice does not add:

- a public source-page GraphQL, HTTP, CLI, MCP, or native-admin transport;
- cloud, Vault, or Kubernetes resolver configuration specifically for this Index keyring;
- persisted cursor state or multi-page jobs;
- background scanning, scheduling, lifecycle commands, or repair;
- retained secret-resolution, rotation, expiry, authorization, database, or transport evidence.

## Suggested maintainer validation

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-server index_source_continuation_runtime -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-source-continuation-server.mjs
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets --features mod-product
git diff --check
```

No tests, verifiers, formatting, Cargo checks, cryptographic integration, workflows, or CI were run
by the implementation agent.