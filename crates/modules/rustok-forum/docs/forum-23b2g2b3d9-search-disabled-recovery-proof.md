# FORUM-23B2G2B3D9 Search-disabled recovery proof

## Status

`source_ready_maintainer_execution_pending`

This slice adds the `search_disabled_profile` subset of the frozen
`FORUM-23B2G2B3D` runtime-evidence matrix. It proves that Forum owner writes do
not synchronously depend on Search storage or workers and that the durable Forum
owner ledger can recover projection state after Search is enabled later.

The machine-readable contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-search-disabled-recovery-proof.json
```

The executable PostgreSQL integration test is:

```text
apps/server/tests/forum_versioned_invalidation_search_disabled_recovery.rs
```

Successful execution writes:

```text
target/forum-search-versioned-invalidation-search-disabled-recovery-evidence.json
```

The artifact is written only after the proof and isolated PostgreSQL schema
cleanup succeed. It records the exact Git source commit and must not be
hand-edited.

## Disabled runtime profile

The test starts an isolated PostgreSQL schema with only:

- Outbox migrations;
- Taxonomy migrations;
- Forum migrations.

Search migrations are deliberately not applied. The test requires all three
Search-owned tables below to be absent both before and after the Forum owner
commands:

```text
search_projection_inbox
search_projection_owner_checkpoints
search_documents
```

This is a stronger storage boundary than merely pausing a consumer: Forum is
executed while the Search persistence model does not exist in the schema.
No Search worker, projector, contract ingress, inbox row or checkpoint can
participate in the owner transactions.

## Forum owner commands

While Search is absent, real Forum services commit:

1. one public category;
2. one public topic containing `d9searchdisabledtopicone`;
3. a second public topic containing `d9searchdisabledtopictwo`.

The topic authors are intentionally anonymous owner values (`author_id = NULL`)
so this bounded proof does not require Profiles migrations. Forum permissions,
validation, category counters, public discovery state and transactional event
publication remain production behavior.

The expected Forum owner revision ledger is exact and contiguous:

```text
revision 1: forum / null
revision 2: forum_category / category ID
revision 3: forum_category / category ID
```

For every revision the test requires:

- one committed legacy `index.reindex_requested` root envelope whose ID equals
  `forum_projection_revision_ledger.event_id`;
- one committed `forum.search_projection.invalidation_issued` envelope whose
  `causation_id` equals that same root event ID.

The category row, both topic rows, translations, roots, typed events and ledger
rows therefore commit without any synchronous Search call.

## Late Search enable

Search is enabled in the same isolated owner database only after the Forum
commands have committed. The test then applies the production Search migrations
and installs an isolated checkpoint audit trigger.

At the enable boundary the test requires:

```text
Search inbox rows:       0
Search owner checkpoints: 0
Forum Search documents:  0
```

No historical typed or legacy delivery is injected. Recovery must come only
from the Forum-owned revision ledger.

The test composes:

- `ForumSearchProjectionSourceFactory` for current Forum owner state;
- a real `ForumEventService`-backed owner revision source matching the server
  adapter contract;
- `ForumProjectionReconciler::with_owner_revision_source`.

One bounded sweep must discover the tenant, read revisions `1..3`, classify all
three event UUIDs as missing from the Search inbox, run exactly one current-state
Forum rebuild and advance the checkpoint through revisions `1`, `2` and `3`.

## Rebuild-before-checkpoint ordering

The isolated PostgreSQL trigger records every insert or update to
`search_projection_owner_checkpoints`. Each audit row must observe all three
current Forum documents already committed:

```text
revision 1 / rebuild_repaired / 3 documents
revision 2 / rebuild_repaired / 3 documents
revision 3 / rebuild_repaired / 3 documents
```

The final checkpoint must reference the exact revision-3 root event UUID.
The recovery path must not synthesize `search_projection_inbox` deliveries.

## Owner-state preservation

The test takes a complete bounded snapshot before Search enable containing:

- category ID, name, slug and description;
- both topic IDs, category IDs, statuses, titles, slugs and bodies;
- all three owner revision rows and event identities.

After projection recovery the same snapshot must compare equal. Search enable
and reconciliation may materialize a projection and checkpoint, but they cannot
rewrite or lose Forum owner state or its committed event history.

The resulting Search projection must contain exactly:

- one `forum_category` document;
- two `forum_topic` documents with the original marker titles and bodies.

A second caught-up sweep must perform zero rebuilds and zero checkpoint
advances while retaining the same three documents and three audit rows.

## Static dependency boundary

`crates/rustok-forum/Cargo.toml` has no `rustok-search` dependency.
`projection_invalidation.rs` writes only owner revision tables and Outbox-owned
events. It does not reference `search_documents`, `search_projection_inbox` or
Search-owned checkpoint tables.

Search integration remains a downstream host composition concern through the
production projection source, owner revision port and reconciler.

## Deliberate boundary

D9 does not start a long-running host process, exercise worker polling cadence,
restart a deployment or use Iggy acknowledgement, poison or DLQ behavior. It
models Search-disabled operation by the complete absence of Search migrations
and workers during owner commands, followed by explicit late enable in the same
durable PostgreSQL database.

This slice does not close aggregate `FORUM-23B2G2B3D` evidence or
`LINK-FORUM-03`.

## Maintainer verification

```bash
node scripts/verify/verify-forum-search-versioned-invalidation-search-disabled-recovery-proof.mjs
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-server \
  --test forum_versioned_invalidation_search_disabled_recovery \
  -- --nocapture --test-threads=1
```

No command above was run by the implementation agent.
