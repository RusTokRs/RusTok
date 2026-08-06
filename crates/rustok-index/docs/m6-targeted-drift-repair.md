# M6 targeted drift repair boundary

Status: `source_complete_recovery_aware_orphan_pending`.

## Purpose

`IndexDriftRepairService` defines one internal, authorization-gated repair attempt for one exact open
confirmed drift finding. The generic boundary admits the two deterministic finding identities emitted
by `PostgresIndexDriftConfirmedCandidateWriter`:

- `index.confirmed_missing_entity`;
- `index.confirmed_orphan_link.<sha256>`.

A separate concrete composition supports only the missing-entity kind and now includes a fail-closed
prepared-command recovery policy. Orphan-link repair remains unsupported. The service does not scan
findings, choose arbitrary SQL, accept a record payload, run a background loop, mount a transport, or
automatically resolve the finding.

## Exact command

`IndexDriftRepairCommand` requires:

- one non-nil tenant UUID;
- one non-nil finding UUID;
- one non-nil idempotency command UUID;
- one typed `IndexDriftRepairTarget`;
- one bounded lifecycle actor identity;
- one trimmed nonempty reason bounded to 512 bytes.

The target is a typed preimage, not mutation input.

A missing-entity target carries the exact entity key, indexed source version, and authoritative
absence version. An orphan-link target carries the exact source key/version, link name, ordinal,
complete typed target identity, and authoritative target-absence version.

The PostgreSQL store accepts the target only when it reproduces the persisted finding contract:

- exact entity scope;
- exact check name, including the orphan identity SHA-256 suffix;
- exact deterministic finding key;
- exact expected and actual evidence digests;
- the established `index_drift_digest_finding_v1` details marker.

This cryptographic preimage check prevents an authorized caller from substituting another link,
target, ordinal, locale, source version, or absence version. The store performs no broad link scan and
does not deserialize raw repair JSON.

## Authorization capability

`IndexDriftRepairAuthorizer` runs before finding storage is read. A denied request returns `Denied`
and does not reveal whether the finding exists.

Only `IndexDriftRepairService` can construct `IndexDriftAuthorizedRepairCommand`. The repair store,
evidence reader, and repair owner receive that capability rather than an ordinary command.

No default allow-all authorizer is provided.

## Durable reservation fence

`PostgresIndexDriftRepairStore::reserve` uses one PostgreSQL `SERIALIZABLE READ WRITE` transaction.
It:

1. advisory-locks the exact tenant and command UUID;
2. validates exact replay or rejects command UUID payload reuse;
3. locks and validates the exact open finding;
4. verifies the complete typed target commitment;
5. rejects another active command for the same finding;
6. inserts one durable `prepared` reservation.

Migration `m20260806_000007_add_index_finding_repair_commands` adds
`index_consistency_finding_repair_commands` and a partial unique index allowing at most one
`prepared` command per tenant/finding.

A completed retry returns its existing terminal receipt. A different active command receives
`FindingBusy`. Exact retry of `prepared` state is additionally subject to the recovery policy below.

## Before, owner, and after boundaries

After reservation, the service requires:

1. one admitted `capture_before` result;
2. exactly one repair owner selected by target kind;
3. at most one owner `repair` call;
4. one admitted `capture_after` result;
5. one terminal receipt write.

The evidence reader returns only a bounded state and lowercase SHA-256 digest:

- `Repairable`;
- `Converged`;
- `Changed`.

A successful receipt requires:

- repairable before evidence;
- an owner `Applied` result with a bounded receipt digest;
- converged after evidence.

Any other admitted result becomes a typed `NotRepaired` receipt. Dependency failures stay bounded
retryable/permanent machine-code failures and leave the durable reservation available for an exact,
recovery-admitted retry.

## Prepared-command recovery policy

`IndexDriftRepairRecoveryService` defines independently authorized `Resume`, `Pause`, and `Abandon`
decisions over one exact durable repair identity and expected recovery revision. The original repair
payload digest commits the typed target, original actor, and original reason.

Migration `m20260806_000008_add_index_finding_repair_recovery` adds an append-only decision ledger with
these states:

- unclassified legacy command;
- active;
- paused;
- terminally abandoned.

New reservations receive immutable revision `0` state `active`. A legacy or crash-stranded prepared
row with no ledger decision fails closed with `index_drift_repair_recovery_required`; it never resumes
from wall-clock age or process-liveness inference.

The concrete missing-entity composition wraps both the durable store and owner:

- reservation and completion require latest state `active`;
- the owner holds the same tenant/command PostgreSQL advisory fence while validating payload identity,
  checking active state, and performing the idempotent mutation call;
- pause or abandon therefore wins before owner admission or waits until the already admitted owner
  call finishes;
- a database trigger rejects `prepared -> completed` unless latest state is still `active`.

If an operator decision wins after the owner call but before receipt persistence, completion fails
closed. No side effect is inferred. Authorized resume preserves the original command UUID and reaches
the established mutation inbox duplicate path.

The detailed policy is documented in `m6-prepared-repair-recovery.md`.

## Concrete missing-entity composition

`materialize_postgres_index_drift_missing_entity_repair_service` composes the first concrete path. It
requires an explicit repair authorizer, frozen source and absence registries, an immutable schema
registry, and PostgreSQL.

The concrete path:

- rejects `OrphanLink` before the generic reservation store can create `prepared` state;
- brackets one exact `index_entities` identity read with two authoritative owner reads;
- admits ordinary source `Delete` or explicit retained absence watermark evidence;
- requires absence version strictly newer than the live indexed version;
- emits a typed `IndexMutation::Delete` through `PostgresMutationStore`;
- uses the durable repair command UUID as the mutation event and inbox delivery identity;
- re-reads evidence and requires an exact tombstone at the admitted absence version;
- returns a domain-separated owner receipt digest without payload or database causes;
- applies the recovery-aware store, owner fence, and completion trigger described above.

The detailed concrete contract is documented in `m6-missing-entity-repair-composition.md`.

## Terminal receipt

`complete` opens another PostgreSQL `SERIALIZABLE READ WRITE` transaction. It validates the
tenant-bound reservation ticket and performs the only allowed repair-row transition:

`prepared -> completed`

The terminal row records:

- `repaired` or `not_repaired`;
- bounded outcome code when not repaired;
- exact owner name;
- before digest;
- optional after digest;
- optional owner receipt digest;
- database-owned completion timestamp.

The original database trigger preserves command identity, target kind, actor identity, reason, and
payload digest across completion. The recovery database trigger additionally requires latest state
`active`. A completed row cannot be updated again under the trigger contract.

The current finding lifecycle row is not rewritten by repair. If the finding is no longer open at
completion, the store persists `NotRepaired(finding_not_open)` rather than claiming success.
Authorized recovery resume also rejects a finding that is no longer open.

## Crash and lifecycle boundary

A `prepared` reservation intentionally survives source, owner, evidence, serialization, or process
failure. The concrete missing-entity owner remains idempotent through the mutation inbox.

Recovery decisions are immutable and never derived from elapsed time. Pause is an admission fence,
not cancellation after an owner call has acquired the advisory lock. Abandon is a terminal recovery
decision and does not fabricate before evidence, after evidence, an owner receipt, or a repair
receipt.

Finding lifecycle rows and repair/recovery history remain separate. Recovery neither resolves nor
ignores a finding, and lifecycle closure does not delete the durable repair or recovery records.

## Privacy and transport

Repair and recovery command `Debug` output expose actor-subject and reason lengths only. Stored
receipt decoders have no derived payload-revealing `Debug`. Public failures expose bounded machine
codes only.

The crate exports internal PostgreSQL materializers, but does not insert the store, evidence reader,
owner, repair service, or recovery service into `ModuleRuntimeExtensions`. There is no GraphQL, HTTP,
CLI, MCP, native-admin, scheduler, worker, or automatic-repair surface.

## Deliberate limits

These slices do not add:

- a concrete orphan-link repair owner;
- lifecycle transition after successful repair;
- time-based lease expiry or automatic ownership inference;
- cancellation after an owner call acquires the recovery fence;
- automatic finding iteration or candidate-page consumption;
- public authorization or transport;
- retained migration, PostgreSQL/SQLite, owner, crash-window, concurrency, workflow, or CI evidence.

## Next implementation step

Compose one concrete bounded orphan-link evidence reader and idempotent mutation owner behind the
existing recovery-aware repair boundary. Preserve exact source link identity, ordinal, typed target,
target absence version, and durable command UUID.

Keep public transport, automatic finding iteration, and retained production evidence separate.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_repair -- --nocapture
cargo test -p rustok-index drift_missing_entity_repair -- --nocapture
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-missing-entity-repair-composition.mjs
node scripts/verify/verify-index-prepared-repair-recovery.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, formatting, Cargo checks, migrations, PostgreSQL/SQLite scenarios,
workflows, or CI were executed by the implementation agent.
