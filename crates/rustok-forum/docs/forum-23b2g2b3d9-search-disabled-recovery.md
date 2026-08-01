# FORUM-23B2G2B3D9 Search-disabled owner continuity and recovery proof

## Status

`source_ready_maintainer_execution_pending`

This slice implements the `search_disabled_profile` scenario frozen by the
`FORUM-23B2G2B3D0` runtime-evidence protocol. It proves that a real Forum owner
command retains no synchronous dependency on Search and that the committed
Forum owner ledger can later repair Search through the existing bounded
reconciliation lane.

The machine-readable proof contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-search-disabled-proof.json
```

The executable server-hosted target is:

```text
apps/server/tests/forum_versioned_invalidation_search_disabled_recovery.rs
```

Successful execution writes:

```text
target/forum-search-versioned-invalidation-search-disabled-evidence.json
```

The artifact is executable output, records the exact Git source commit and must
not be hand-edited.

## Relationship to preceding evidence

Merged D6 proves bounded missing-delivery owner repair. Merged D7 proves
independent-process advisory-lock and scan-cursor serialization. Open draft D8
in PR #2789 exercises the real long-running server-owned contract consumer,
bounded retry exhaustion, restart and graceful shutdown.

D9 is an independent D0 scenario. It does not duplicate D8 transport retry or
D7 contention. Instead, it starts before any Search runtime is composed and
proves that Forum owner state and its durable causal record remain sufficient
for later recovery.

The parent D0 contract already registers D7. D8 and D9 intentionally defer
their own registration until each slice merges.

## Disabled capability profile

The target creates one isolated PostgreSQL schema and applies the production
server migration graph. Search storage begins empty. To make the dependency
boundary executable rather than documentary, the target temporarily renames
the four Search-owned tables used by this rollout:

```text
search_documents
search_projection_inbox
search_projection_owner_checkpoints
search_projection_owner_scan_cursors
```

The disabled runtime uses:

```text
rustok.search.enabled=false
```

and contains no `SharedForumProjectionOwnerRevisionSourcePort` or Search
projection-source runtime.

While those Search tables are unavailable, the target invokes the public Forum
owner facade:

```text
rustok_forum::CategoryService::create
```

with an administrative `SecurityContext`. This is the production category
owner command, not direct fixture SQL.

## Owner transaction proof

`CategoryService::create` writes the category owner row and its localized
translation, then calls the production Forum projection invalidation owner
inside the same database transaction.

The target requires the committed result to contain:

1. exactly one category and one `en` translation;
2. exactly one Forum owner-ledger row at revision `1`;
3. exactly one legacy `index.reindex_requested` outbox envelope;
4. exactly one caused
   `forum.search_projection.invalidation_issued` outbox envelope.

The exact identity chain must be:

```text
forum_projection_revision_ledger.event_id
  == legacy root sys_events.id
  == typed ContractEventEnvelope.causation_id
```

The typed payload must carry owner revision `1`, target type `forum` and no
target ID. The owner command must complete even though all four Search-owned
tables are unavailable.

The disabled Search storage must remain empty. No Search inbox row, document,
checkpoint or scan cursor may be written by the Forum owner transaction.

## Re-enable and bounded recovery

After the owner commit, the target restores the Search table names and enables:

```text
rustok.search.enabled=true
```

It then uses the production server composition facade with the real
`IndexModule`, `ForumModule` and `SearchModule`. From those runtime extensions
the target resolves:

- the production Forum `SearchProjectionSource`;
- the production server Forum owner-revision adapter;
- the existing `ForumProjectionReconciler`.

One bounded call to:

```text
ForumProjectionReconciler::sweep_due(1, 8)
```

must discover one owner tenant, execute one current-state rebuild and advance
exactly one owner revision.

The recovered Search state must contain one public Forum category document with
the exact category ID, localized title and slug committed while Search was
disabled. The Search-owned checkpoint must be:

```text
owner_revision = 1
event_id = exact Forum ledger/root event ID
outcome = rebuild_repaired
```

The owner-ledger repair must not synthesize a row in
`search_projection_inbox`; that would create a second execution identity rather
than using the existing owner reconciliation lane.

A second caught-up sweep must perform no rebuild and no checkpoint advancement.

## Compatibility

This slice changes no production Rust source, dependency manifest, migration,
event schema, digest, runtime flag, consumer group, broker topic, Search query
or public API.

It adds no second inbox, projector, reconciler or ordering clock. Forum
`owner_revision` remains independent from Search `ingest_sequence`; the latter
is intentionally absent from this recovery because no transport delivery was
fabricated.

The proof uses no broker and makes no claim about Iggy, acknowledgement,
poison/DLQ or the D8 worker lifecycle.

## Deliberate limits

D9 does not prove:

- successful PostgreSQL execution;
- deletion, richer ACL or storefront visibility ordering;
- out-of-order or duplicate transport delivery;
- D8 retry/backoff or graceful-shutdown execution;
- arbitrary multi-process contention beyond merged D7;
- completion of `FORUM-23B2G2B3D`;
- closure of `LINK-FORUM-03`.

Deletion/ACL ordering remains the final separate D0 runtime scenario after the
host-worker and Search-disabled slices are resolved.

## Maintainer verification

```bash
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test --locked -p rustok-server \
  --features mod-forum \
  --test forum_versioned_invalidation_search_disabled_recovery \
  -- --nocapture --test-threads=1

node scripts/verify/verify-forum-search-versioned-invalidation-d9-search-disabled.mjs
cargo check --locked -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
git diff --check
```

No command above was run by the implementation agent, per maintainer request.
