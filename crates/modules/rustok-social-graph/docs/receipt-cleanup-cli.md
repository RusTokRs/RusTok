# Social Graph receipt-cleanup CLI contract

Status: **source-ready / unvalidated**

`rustok-social-graph-cli` is the owner-local operational adapter for bounded
command-receipt retention. It exposes:

```text
rustok-cli social_graph receipt-cleanup \
  --tenant-id <uuid> \
  --retention-days <positive-integer> \
  [--limit <1..1000>] \
  [--dry-run]
```

## Safety contract

- `--tenant-id` is mandatory and scopes every owner query and delete.
- `--retention-days` is mandatory. The adapter has no deployment retention
  default and derives the owner cutoff as `now - retention_days`.
- `--limit` defaults only the batch size to `100`; it can never exceed the owner
  maximum of `1000`.
- `--dry-run` reaches the same owner selection path but deletes no rows.
- the adapter calls `SocialGraphReceiptMaintenancePort`; it does not read or
  delete `social_graph_command_receipts` directly.
- the port context uses a system actor, a bounded deadline, and deterministic
  operation identity derived from tenant, cutoff, limit, and mode.
- output is aggregate only: tenant, retention window, cutoff, mode, limit,
  matched/deleted counts, and oldest retained completion time.
- idempotency keys, receipt request/response snapshots, user identities, claims,
  roles, locale values, and channel data are not returned or logged by the
  adapter.
- no scheduler or automatic cleanup is enabled by this slice. Cadence and the
  approved retention window remain deployment/operator decisions.

## Rollout

1. Apply the receipt schema and deploy receipt-aware writers.
2. Prove replay/conflict behavior for the tenant.
3. Run the command with `--dry-run` and review the retained completion floor.
4. Run a bounded live batch only after the deployment retry horizon, clock skew,
   and incident replay allowance are documented.
5. Repeat bounded batches explicitly; do not convert the command into an
   unreviewed default-on worker.

Application rollback keeps the receipt table and rows. Do not run cleanup while
rolling back writers.

## Maintainer verification

```bash
cargo check -p rustok-social-graph-cli --all-targets
cargo test -p rustok-social-graph-cli -- --nocapture
node scripts/generate/generate-cli-registry.mjs --check
node scripts/verify/verify-social-graph-receipt-cleanup-cli.mjs
rustok-cli social_graph receipt-cleanup --tenant-id <uuid> --retention-days 30 --limit 100 --dry-run
```

These commands were intentionally not run by the implementation agent.
