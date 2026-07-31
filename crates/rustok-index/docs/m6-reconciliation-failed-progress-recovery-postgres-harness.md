# M6 Reconciliation Failed Progress Recovery PostgreSQL Harness

Status: executable target retained, not run.

This harness retains the durable boundary where a reconciliation run has already published one safe page cursor and then fails while applying a mutation from the next page.

It complements the source-failure diagnostics harness and the mutation-failure diagnostics harness by proving recovery after non-zero durable progress.

## First attempt

The source owns two pages with stable event UUIDs and source versions.

Page one returns one valid mutation and cursor `{ "offset": 1 }`. The canonical `PostgresIndexReconciliationRunner` must:

- apply the mutation;
- persist one entity and one inbox row;
- publish durable reconciliation cursor progress with `pages_processed = 1`;
- retain `completed_passes = 0`;
- heartbeat before scanning page two because `heartbeat_every_pages = 1`.

Page two remains inside the exact tenant and schema scope, but its record omits the required `id` field. Source-page scope validation therefore succeeds, while `PostgresMutationStore` schema validation returns permanent `mutation_rejected`.

The runner must return `IndexReconciliationRunError::MutationFailed` at position zero and terminalize attempt one as `failed`.

## Durable failed state

The failed row must retain the previous safe page boundary:

- state `failed`;
- attempt count 1;
- `completed_passes = 0`;
- `pages_processed = 1`;
- source cursor `{ "offset": 1 }`;
- cleared lease owner and expiry;
- non-null completion timestamp.

Its diagnostic must be exact three-field JSON:

```json
{
  "contract": "index_reconciliation_run_failure_v1",
  "dependency_code": "mutation_rejected",
  "retryable": false
}
```

The diagnostic exposes no mutation payload, tenant, actor, worker, job, database, SQL, transport, stack, or owner-detail field.

The second-page invalid mutation must create no entity or inbox row. PostgreSQL must retain exactly one entity, one inbox row, and one failed reconciliation job.

A later exact-tenant cancellation request must return `AlreadyTerminal(Failed)`.

## Duplicate-safe recovery

Terminal failed reconciliation jobs are excluded from active acquisition under the current runner. A new explicit invocation therefore creates a new job and starts from the initial source cursor.

The recovery source returns:

1. the same valid first-page mutation with the same event UUID and source version;
2. a corrected valid second-page mutation using the same second-page event UUID and source version that never reached mutation storage during the failed run.

The recovery invocation must:

- use a different job UUID;
- complete as attempt one;
- process two pages and one completed pass;
- execute one page-boundary heartbeat;
- report one duplicate and one newly applied mutation;
- report no stale mutation;
- preserve exactly two entity rows and two inbox rows.

The succeeded job must retain `completed_passes = 1`, `pages_processed = 2`, a null source cursor, cleared lease ownership, and no failure diagnostic.

A later invocation must return `AlreadyComplete` for the succeeded recovery job without processing another page.

## PostgreSQL isolation

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with a PostgreSQL `DATABASE_URL` fallback. Without a PostgreSQL URL it reports a skip and succeeds.

Each invocation creates a unique schema, creates the tenant owner fixture, applies every real `IndexModule` migration, persists one active schema, uses one-connection pools with schema-local `search_path`, reads durable evidence, and drops the schema.

No sleep, polling delay, or elapsed-time race is used.

## Scope boundaries

This harness changes no production code, migration, reconciliation SQL/state machine, source contract, cursor shape, mutation identity, schema, diagnostic, or public API.

It does not provide:

- automatic retry, backoff, scheduling, or attempt exhaustion;
- failed-scope dead-letter admission or authorized requeue;
- cancellation, lease-loss, heartbeat-takeover, or restart races;
- source-call timeout or pending-future preemption;
- digest comparison, orphan cleanup, targeted/full/shadow repair, locale/partition dimensions, or complete drift repair.

The canonical M6 reconciliation and drift-repair item therefore remains open.
