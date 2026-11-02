# Selected Inventory → generic Index consumption

Status: **source authored, uncompiled/unrun; recovery and readiness gates remain open**.

## Ownership and selection

The actual Inventory module contributes `InventoryRuntimeSelected`. Only that selected marker
allows Distribution to register the nonlocalized `rustok-inventory/stock` schema, native
`inventory-stock-primary` source and `inventory.source.changed` refresh route. The existing
Index source-factory catalog contributes the real sealed `inventory_stock_index` listener
through `IndexModule::register_event_listeners`, after immutable capabilities exist.

Inventory owns baselines, positive per-Variant revisions, original change identities, live
records and retained tombstones. Auth owns current service contexts. Distribution owns only
native-port conversion/registration and event adaptation; it performs no Inventory SQL. Index
owns durable schema/inbox/record/job/checkpoint mutations and remains domain-neutral. The server
composes existing owner capabilities; it contains no Inventory source query or conversion.

## Current authenticated reads

The selected source resolves `WorkerServiceContextRuntime` afresh for each bounded source/fact
invocation. The server provider calls the canonical current access-token resolver, validates a
real tenant OAuth `client_credentials` service principal and filters current grants through
current module availability. Actor conversion delegates to canonical `AuthContext::port_actor()`:
OAuth service actors carry the genuine authenticated `client_id`, distinct from the application
UUID/JWT subject that authentication also verifies. This is intentional native-port semantics,
not an identity bug or a reason to invent another actor mapping. The source needs current
`inventory:read` (or an actually granted applicable owner permission), never an invented system
principal. Baseline creators, event actors, stored jobs, receipts, journals and worker invocation
labels grant no read authority. No service identity, token or claim set is serialized into jobs.

Deployment binds exact tenant/consumer pairs using `RUSTOK_WORKER_SERVICE_TOKENS_JSON`:

```json
[
  {
    "tenant_id": "11111111-1111-1111-1111-111111111111",
    "consumer": "inventory-stock-primary",
    "access_token": {
      "resolver": "env",
      "key": "RUSTOK_INVENTORY_INDEX_SERVICE_ACCESS_TOKEN"
    }
  }
]
```

The tenant is an illustrative UUID, not a provisioned account or permission grant. Issue/manage
the actual service through the existing Auth/OAuth owner; this integration does not mint tokens
or add grants. Inline tokens, duplicate bindings, nested unknown credential fields, cross-tenant
reference aliases and unbounded input refuse. Configuration is at most 256 KiB / 256 bindings;
a token is at most 16 KiB. `mounted_file` uses `RUSTOK_WORKER_SERVICE_SECRET_MOUNT_ROOT`; an
already composed canonical secret registry is reused instead of installing another one.
Each invocation uses canonical `resolve_fresh_for_tenant`: tenant policy is enforced, while the
registry cache is neither consulted nor populated. Invalidate-then-resolve alone would race a
concurrent stale cache fill. Effective authorization is also resolved afresh. This is not a
linearizable snapshot across an external secret rotation and a later owner read.
Missing/expired/revoked credentials, unavailable policy or denied reads withhold success rather
than falling back to an actor from the event or pretending stock is zero. Bindings change at
startup; rotate the credential through the referenced secret and existing OAuth lifecycle.

## Projection and bounded reads

The selected production source/replay worker requires PostgreSQL. SQLite appears only in
focused test fixtures for this bridge/job boundary; it is not a production fallback. Inventory's
separate native SQLite owner-baseline support does not certify this selected Index composition.

- Original Variant UUID is the entity key. No locale or inferred Product link is added.
- UUID fields: `id`, `product_id`, `inventory_item_id`, `baseline_id`.
- Signed integer fields: `stocked`, `reserved`, `incoming`, `available`.
- Availability is checked `stocked - reserved`; negative availability is preserved.
- The native owner must return a positive i64-bounded revision and original non-nil change UUID.
- A retained owner delete becomes a retained Index delete, not a missing-row/zero-stock inference.
- Reads use only `InventorySourceHistory::baseline_status/load/scan/changes`, at most 128 records
  per invocation and a 30-second total source-read budget.
- Live keyset continuation binds exact tenant, active baseline and last Variant. It is not a
  repeatable snapshot, complete absence certificate or globally ordered commit watermark.
  Canonical live facts cover writes occurring behind a replay cursor.

## Persistence and acknowledgement

Changed requires the original committed `(tenant, baseline, Variant, source_version, change UUID)`
journal identity and current active baseline. It persists the tenant schema and awaits generic
`persist_index_source_refresh`: exact registered key, authoritative load at least the event's
version, preserved current owner mutation UUID and durable inbox/upsert/tombstone. Only then does
the listener return success to the canonical module delivery gate. There is no dummy acknowledger.
An older redelivery loading a newer owner revision must not rebind that payload to its old envelope
UUID; the immutable inbox hash remains stable per owner change.

BaselineActivated additionally requires envelope UUID equal to the original baseline UUID. It
verifies the current owner baseline and commits a pending rebuild under that same original job
UUID before success, including empty baselines. Duplicate activation retains original job,
attempt/checkpoint and completed null-cursor proof; cancellation, failed state or identity drift
refuses. A two-row retained scope-history bound suffices to detect another original job identity.
A retained checkpoint without its original job also refuses a fresh enqueue; it must never let
an unrelated completed cursor certify a new initial replay. Already-complete acquisition checks
the exact durable checkpoint before returning success. No new UUID or checkpoint reset repairs
a refused original job.

The canonical Index replay materializer registers `index_replay` through the existing ModuleWork
lifecycle. A server role with background workers enabled discovers only selected source/schema
bindings and invokes the single `SharedIndexReplayRuntime`: 128 records/page, 8 pages/invocation,
300-second renewable lease and per-page heartbeat. Discovery's fresh invocation token is only a
worker/lease-owner label, not source authority; the canonical runner acquires/fences the exact
original durable job before mutations. Busy is held, a bounded yield is resumable, and completion
checks the retained job/checkpoint rather than adding an independent ACK ledger.

## Known Limitations / Pending Implementation

- **Failed-job retry/recovery is not wired.** The current runner terminalizes page failures,
  including retryable owner failures. Crash/expired lease and successful yield resume the original
  checkpoint; a failed/cancelled original parks and activation redelivery refuses replacement.
  Lease attempts include normal yielded acquisitions, not only failures. Do not call the retry
  store after terminalization or claim automatic backoff/operator recovery is complete.
- Do not start an unrelated manual Full replay for this new scope before its original activation
  job is enqueued: an independently created job identity conflicts and must not be substituted.
- Missing/corrupt retained selected job metadata fails closed and needs explicit owner recovery;
  it is not silently skipped, acknowledged or reset. Removed/unselected source jobs stay retained
  and are excluded from this deployment's discovery so they cannot starve selected sources.
- No compiler, tests, SQL, migrations, credentials, runtime, broker, concurrency or restart scenario
  was executed. Test fixtures/doubles and lexical guards do not establish real OAuth/secret
  rotation, PostgreSQL locking or transport ACK behavior.
- Full native/HTTP/GraphQL/Next/Leptos operator recovery remains pending. No new transport/UI
  capability or storefront/public-stock readiness is exposed by this source slice.
- Older active baselines may lack canonical facts. No historical publication/backfill/reset is
  inferred from status or the journal. Explicit installation/digest/schema/credential/worker/
  backlog/recovery preflight is required before a live baseline or consumer cutover.
- Product and other existing source/remote-only consumers remain separate. Generic Index, source
  registry presence and initial replay enqueue are not Product step-8 completion.

Tracking: [Inventory plan](../../rustok-inventory/docs/implementation-plan.md),
[Index plan](../../rustok-index/docs/implementation-plan.md),
[source evidence](../../rustok-inventory/contracts/evidence/source-index-consumer-source.json).
