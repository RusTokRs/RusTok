# FORUM-23B2G2B3D4 poison and DLQ recovery source proof

## Status

`source_ready_maintainer_execution_pending`

This slice continues the frozen `FORUM-23B2G2B3D0` runtime matrix after the
merged PostgreSQL ingress proof `FORUM-23B2G2B3D2` and external-Iggy
acknowledgement-restart proof `FORUM-23B2G2B3D3`.

D4 adds one bounded source test for the connector-owned poison receipt and DLQ
recovery protocol already composed by the Forum Search typed consumer. It does
not repeat the D2 inbox or D3 broker cursor scenarios.

The machine-readable contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-d4-poison-recovery-source-proof.json
```

## Durable poison and DLQ recovery

`apps/server/tests/forum_search_poison_protocol.rs` applies the real
`rustok-iggy-connector` migrations to SQLite and exercises the public production
receipt, decode-failure and DLQ entry types used by
`forum_search_contract_consumer.rs`.

The covered order is exact:

```text
reserve_and_claim
  -> publish deterministic DLQ entry
  -> mark_published
  -> acknowledge exact source position
  -> mark_acknowledged
```

The source proof covers:

- raw poison exact-byte identity and deterministic broker message ID;
- semantic poison code
  `forum.search_projection.contract_inbox_identity_conflict` through the same
  receipt protocol;
- an acknowledgement failure after durable `published`;
- redelivery returning `AlreadyPublished`, with no application-level second
  publication;
- acknowledgement-only recovery to durable `acknowledged`;
- a failed DLQ publication releasing its claim, retaining `reserved`, and
  allowing a restarted publisher to reclaim and complete the terminal result.

The existing server worker remains the production composition point. Its source
order is guarded by
`scripts/verify/verify-forum-search-versioned-invalidation-d4-poison-recovery.mjs`.

## Deliberate limits

This source slice does not claim physical broker exactly-once publication.
Publication may succeed while the process fails before `mark_published`; that
confirmation ambiguity still depends on deterministic broker message IDs,
retained broker deduplication policy and external-Iggy evidence.

It also does not claim:

- successful execution of the new test or verifiers;
- an external Iggy poison delivery, DLQ publication or server-worker restart;
- multi-process claim contention or lease expiry;
- owner-checkpoint missing-delivery repair;
- deletion/ACL/Search-disabled end-to-end correlation;
- completion of `FORUM-23B2G2B3D` or closure of `LINK-FORUM-03`.

D3 already owns the external-Iggy acknowledgement/restart subproof. D4 neither
replaces nor duplicates it.

## Compatibility

No production migration, event schema, digest, runtime flag, topic, consumer
group, public API, Search query, dependency or `Cargo.lock` entry changes. No
second inbox, projector, reconciler or ordering clock is introduced.

## Maintainer verification

```bash
cargo test -p rustok-server --test forum_search_poison_protocol -- --nocapture
node scripts/verify/verify-forum-search-versioned-invalidation-d4-poison-recovery.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-runtime-evidence.mjs
cargo check -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
git diff --check
```

No command above was run by the implementation agent.
