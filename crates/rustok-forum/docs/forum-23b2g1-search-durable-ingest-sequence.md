# FORUM-23B2G1 durable Forum Search inbox ingest sequence

## Status

`source_complete_execution_pending`

This slice replaces Forum Search inbox execution ordering based on producer
wall-clock timestamps and event UUID tie-breaking with a Search-owned immutable
PostgreSQL ingest sequence. Runtime, migration and restart evidence remain
maintainer-owned and are not claimed here.

## Ordering boundary

`search_projection_inbox.ingest_sequence` is assigned by PostgreSQL when a new
inbox row is inserted. It is positive, unique and immutable. Sequence gaps are
allowed because a duplicate `event_id` may consume a sequence value before
`ON CONFLICT DO NOTHING` discards the duplicate row.

Existing rows are backfilled deterministically by:

```text
created_at ASC, revision_at ASC, event_id ASC
```

The database sequence is then advanced beyond the largest retained value. New
rows therefore cannot sort before rows that already existed at migration time.

## Claim and watermark semantics

The PostgreSQL reconciler:

1. claims the lowest due `ingest_sequence` for a tenant;
2. blocks later rows while that row is retry-delayed or being processed;
3. records the completed sequence in `search_projection_watermarks`;
4. skips a non-redaction row only when its sequence is not greater than the
   effective scope watermark.

Full-scope watermarks continue to dominate category-scope watermarks. Profile
privacy and account-deletion author scopes remain redaction barriers and are
never skipped from an unrelated watermark.

`revision_at` and `event_id` remain stored and validated against the serialized
envelope. They are retained for diagnostics and event identity, but no longer
determine execution or watermark order.

## Compatibility and degraded mode

No Forum owner write, root event schema, reindex target, projection document,
storefront API, dependency or `Cargo.lock` change is introduced. Retry limits,
backoff, dead-letter behavior, advisory locking and current-source rebuild
semantics remain unchanged.

The new migration is PostgreSQL-only. SQLite can still accept the existing inbox
schema used by domain fixtures, but background Forum projection reconciliation
remains unsupported there exactly as before.

This is deliberately a Search-issued ingest sequence, **not** the final
Forum-owner-issued projection revision. A later versioned owner event must carry
a monotonic Forum revision and reconcile it with this durable delivery sequence.

## Maintainer verification

The implementation agent did not run these commands:

```bash
cargo test -p rustok-search forum_inbox -- --nocapture
cargo test -p rustok-search --test forum_projection_sweeper_contract -- --nocapture
node scripts/verify/verify-forum-search-durable-ingest-sequence.mjs
cargo check -p rustok-search --all-targets
cargo xtask module validate search
cargo xtask module validate forum
```

PostgreSQL evidence should retain migration output for pre-existing rows,
concurrent insert ordering, duplicate-event gaps, retry blocking, restart,
full/category watermark interaction, redaction barriers and a clock-skew case
whose execution order follows `ingest_sequence` rather than `revision_at`.
