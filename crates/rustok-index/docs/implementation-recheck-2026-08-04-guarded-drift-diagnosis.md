# M6 guarded exact-entity drift diagnosis recheck — 2026-08-04

Audited baseline: `main@c6ae3db0caf64c4578cb76073e9b719e483fb953`.

## Rechecked inputs

- `IndexDriftDigestProducer` already validates one exact snapshot pair, hashes typed states, and
  delegates unequal digests only to `IndexDriftMismatchRecorder`.
- `PostgresIndexDriftSnapshotReader` already enforces a positive owner source-version fence around
  one PostgreSQL `REPEATABLE READ READ ONLY` materialized snapshot.
- `PostgresIndexDriftFindingWriter` already owns deterministic create/refresh/reopen/suppression
  lifecycle for locale-bearing and locale-free entity findings.
- `IndexReconciliationOperatorContext` already owns the server request-bound tenant/actor identity
  and `Permission::MODULES_MANAGE` authorization pattern.
- The server replay composition is the only point where PostgreSQL source factories are invoked and
  immutable source/schema registries are frozen.

## Selected composition

A separate `IndexDriftDiagnosisOperatorRuntime` is published beside the reconciliation runtime from
the same source-freezing function. This is intentionally a sibling capability rather than another
field on the runner/recovery runtime:

- both use the same `IndexReconciliationOperatorContext` and effective permission snapshot;
- diagnosis owns the snapshot reader, digest producer, and finding writer privately;
- reconciliation retains only runner, inspection, and recovery ownership;
- neither capability exposes database, registries, adapters, schedulers, or worker handles.

The public diagnosis surface is exactly `diagnose_entity(context, key)`. The key tenant is compared
with the authorized context before `IndexDriftDigestRequest` construction or dependency access.
No batch, scan, cursor, repair mode, lifecycle command, caller-selected actor, or caller-selected
separate tenant is accepted.

## Result boundary

The command returns only `IndexDriftDigestOutcome`:

- `Consistent { digest }`; or
- `MismatchRecorded { source_digest, materialized_digest, receipt }`.

It never returns raw source/materialized records, fields, links, SQL, storage causes, snapshot text,
credentials, registry handles, or finding details beyond the existing bounded receipt.

## Deliberate open work

- Empty owner targeted loads remain `index_drift_source_watermark_missing`; no absence is invented.
- No transport is added.
- No automatic resolution occurs when the pair is consistent.
- No resolve/ignore command, actor/reason audit, discovery, orphan diagnosis, or repair is added.
- PostgreSQL, authorization, and end-to-end finding evidence remain maintainer-run.

## Next cursor

Define an explicit retained absence/tombstone watermark in the targeted owner-load contract so a
truthful positive-version source `Missing` state can participate in the same fence. Keep scans,
automatic lifecycle changes, and repair outside that contract slice.

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed
by the implementation agent.
