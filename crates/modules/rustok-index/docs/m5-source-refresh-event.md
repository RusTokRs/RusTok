# M5 exact source refresh event worker

Status: `source_only_selected_inventory_consumer_recovery_and_runtime_evidence_pending`.

## Purpose

Thin owner notifications request an authoritative exact-key load instead of transporting copied
Index records. `persist_index_source_refresh` owns route/load/minimum-version/durable-mutation
ordering independently of transport acknowledgement. `IndexSourceRefreshEventWorker` awaits that
same boundary and only then acknowledges the original opaque token. A sealed module listener
awaits persistence and returns success through the canonical required-module delivery gate;
it does not manufacture an acknowledger.

These application boundaries are domain-neutral and contain no SQL, broker SDK, worker spawning,
credential resolution or source-specific permission logic.

## Delivery and identity contract

`IndexSourceRefreshEventDelivery<T>` carries one registered event domain, a non-nil notification
UUID, an exact tenant/schema/entity/optional-locale key, a positive minimum owner source version
and an opaque acknowledgement token. The registered source owns fields, links, deletion state,
positive revision and its immutable mutation UUID.

**The loaded owner mutation UUID is the durable Index inbox identity.** The notification UUID
remains the delivery/ACK identity. Never rebind a newer loaded payload to an older notification
UUID: after the first successful apply, a redelivery observing a newer owner revision would
otherwise conflict with the immutable inbox payload hash. Nil loaded identities refuse.

## Processing order

1. Resolve the immutable mutation-event route and exact schema.
2. Resolve the immutable schema's source and verify its registered source name.
3. Perform one bounded `IndexSourceLoadRequest` for exactly one key.
4. Require exactly one authoritative upsert or retained tombstone.
5. Require the owner revision to be at least the notification's minimum source version.
6. Validate and preserve the loaded immutable owner mutation UUID.
7. Await durable `IndexReplayMutationSink` persistence.
8. Only the transport wrapper acknowledges the original opaque token; a module listener returns
   success only after the same persistence boundary.

A missing result or a revision below the event fence withholds both persistence and ACK.
Applied, duplicate and stale results permit ACK only after the durable sink has returned.
An ACK failure after apply leaves redelivery safe through the loaded owner's stable identity and
monotonic per-key revision fence. A live load is not a snapshot, absence certificate or global
commit watermark.

## Selected owner composition

Distribution selects `inventory-stock-primary` / `rustok-inventory/stock` when the actual Inventory
module contributes its selection marker. The native adapter resolves a fresh authenticated service
context, reads only Inventory-owned history and keeps conversion outside Index/host code.
Changed verifies the exact committed tenant/baseline/Variant/version/change identity and awaits
this generic persistence boundary. BaselineActivated verifies the original envelope/baseline UUID
and commits an original-ID pending replay request before handler success, including empty sources.
The Index-owned runtime materializer registers `index_replay` through ordinary ModuleWork lifecycle;
its 128-record / 8-page / 300-second-lease invocations resume the exact original job/checkpoint.

See the [Distribution boundary](../../rustok-distribution/docs/inventory-index.md) and
[Inventory source packet](../../rustok-inventory/contracts/evidence/source-index-consumer-source.json).
Existing Product/Variant refresh routes and remote-only Product consumers are not replaced.
No new stock/public/admin query surface or business-readiness claim is introduced.

## Known Limitations / Pending Implementation

- The runner currently terminalizes page failures, including retryable source failures.
  Automatic backoff and operator recovery of an original failed/cancelled job remain unwired.
  Crash/expired lease and successful bounded yield resume; a retained failed job parks, not
  falsely completes or receives a replacement UUID. Lease attempts include normal yields and
  are not a consecutive-failure budget.
- Service bindings, canonical event digest admission, tenant schema installation, backend SQL,
  concurrency, broker delivery/restart and full native/HTTP/GraphQL/Next/Leptos operator recovery
  are not validated by source registration or a successful enqueue.
- Older already-active baselines may lack historical canonical publication. Never fabricate
  missing facts or treat live scan as a repeatable snapshot. Explicit reconciliation/cutover
  remains necessary before live baseline/consumer readiness.
- No tests, compilation, SQL, migrations, broker scenarios or application runtime were executed.
  Read-only source guards, syntax and formatting checks are not behavioral evidence.

The primary implementation cursor remains `M6 - execute and admit concrete repair evidence`.

## Maintainer validation (not executed by the agent)

```bash
cargo test -p rustok-index source_refresh_event --lib
cargo test -p rustok-index source_replay_job --lib
cargo test -p rustok-index source_replay_scheduler --lib
cargo test -p rustok-distribution --features mod-inventory inventory_index --lib
node --test scripts/verify/verify-inventory-index-consumer.test.mjs
```

The ignored PostgreSQL empty-activation definition additionally requires an explicitly isolated
`RUSTOK_INVENTORY_INDEX_DATABASE_URL` and a manual `--ignored` invocation. SQLite fixture definitions
are not PostgreSQL concurrency, real OAuth/secret rotation or broker ACK evidence.
