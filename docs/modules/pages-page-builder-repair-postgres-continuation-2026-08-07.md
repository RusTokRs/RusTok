# Pages / Page Builder Explicit Repair PostgreSQL Continuation

Date: 2026-08-07  
Status: source-ready / explicit-artifact-repair-postgres-harness-source-ready / execution-pending  
Scope: PostgreSQL receipt constraints, rebuild/activation transaction atomicity and activation lifecycle evidence

## Rechecked source state

Current `main` already contains the explicit append-only immutable artifact rebuild command, the separate rebuilt-artifact activation command, bounded GraphQL/HTTP/OpenAPI transports, generated transport-contract coverage and request-level tenant/permission/static-error harnesses.

The remaining database evidence gap is stronger than the existing SQLite regressions: the forward-only rebuild storage identity, rebuild receipt uniqueness, activation receipt uniqueness and event-plus-receipt transaction behavior also need a real PostgreSQL target using the production migrations.

## PostgreSQL repair harness

Marker:

```text
explicit-artifact-repair-postgres-harness-source-ready
```

`crates/rustok-pages/tests/explicit_artifact_repair_postgres.rs` is environment-gated by:

```text
RUSTOK_PAGES_TEST_DATABASE_URL
DATABASE_URL
```

Only PostgreSQL URLs are accepted. Without one, the harness reports a skip and succeeds.

Each run creates a unique PostgreSQL schema, points a one-connection SeaORM pool at that schema through `search_path`, applies the real `OutboxModule` and `PagesModule` migrations, seeds only the minimal `tenant_modules` row required by Page Builder enablement, and drops the schema at the end.

No replacement repair tables are created by the fixture.

## Rebuild packet

The harness creates and reviewed-publishes one GrapesJS page so production code creates the immutable publication artifact and retained rebuild provenance.

It then changes the mutable current body and corrupts the retained active artifact payload. The explicit rebuild must still use retained reviewed provenance rather than the mutable draft and must append one distinct operation-bound artifact:

```text
instance_key = rebuild:<operation-id>
```

The PostgreSQL assertions retain these boundaries:

- rebuilt artifact id differs from the damaged source artifact id;
- artifact/materialization hashes reproduce retained provenance;
- artifact count increases by exactly one;
- the active locale binding remains on the damaged source artifact after rebuild;
- the page version does not change;
- exact replay returns the same operation/artifact and adds no second receipt or artifact;
- the same idempotency key with another request identity returns the rebuild idempotency conflict and adds no receipt/artifact.

### Rebuild receipt constraint rollback

The harness then opens a raw PostgreSQL transaction, changes the page version only as a rollback marker and attempts a second rebuild receipt using the already-bound `(tenant, page, idempotency_key)` identity.

The real migration-owned unique index must reject that insert. The transaction is explicitly rolled back and the page marker must disappear while the original rebuild receipt count remains one.

This proves the forward-only migration constraint and PostgreSQL rollback behavior independently from the service's optimistic replay path.

## Activation packet

Before activation, a stale expected-current-artifact request must fail without changing the binding or inserting an activation receipt.

A valid activation then must:

- bind exactly the rebuilt artifact for the retained locale;
- advance the page version once;
- keep the page published;
- retain both the damaged source artifact row and rebuilt artifact row;
- persist exactly one activation receipt.

An exact replay must return the same activation operation/version and add no second receipt or page-version change.

A second activation request for the already-consumed rebuild receipt, under another idempotency key and the current page version, must be rejected while the single activation receipt remains authoritative.

## Durable lifecycle pair

The harness snapshots all pre-activation outbox ids. After successful activation it loads only newly inserted `sys_events` rows, decodes the actual durable envelopes and requires exactly:

```text
NodeUpdated(page)
NodePublished(page)
```

one of each, both for the activated page.

This is durable PostgreSQL lifecycle evidence at source level. Cache handler execution and generation rotation are intentionally not performed by this harness.

## Activation receipt conflict rollback

The harness also opens a separate PostgreSQL transaction after successful activation and deliberately performs the same ownership order that matters for atomicity:

1. update the page as a rollback marker;
2. append a `NodePublished` outbox row through `TransactionalEventBus`;
3. attempt a second activation receipt for the same rebuild operation;
4. require the migration-owned unique rebuild-receipt index to reject the insert;
5. roll back the transaction.

After rollback:

- the page version must equal the committed activation version;
- the synthetic outbox event id must be absent from `sys_events`;
- the committed activation receipt count must still be one.

This packet proves that a PostgreSQL receipt constraint failure cannot strand a preceding page mutation or durable outbox write from the same owner transaction.

## Evidence boundary

Machine source evidence:

```text
crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-postgres-source.json
```

Status remains:

```text
pages_explicit_artifact_repair_postgres_source_unvalidated
```

Execution is empty. Every validation flag remains false until maintainer execution.

Fail-closed source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-postgres.mjs
```

The guard itself is intentionally not run in this slice.

## Updated parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Explicit append-only rebuild owner command | Source-ready | Runtime evidence pending |
| Rebuild PostgreSQL real-migration harness | Source-ready | PostgreSQL execution pending |
| Rebuild exact replay/conflict on PostgreSQL | Harness-ready | Execution pending |
| Rebuild receipt unique constraint + rollback | Harness-ready | Execution pending |
| Explicit rebuilt-artifact activation | Source-ready | Runtime evidence pending |
| Activation stale-current fence on PostgreSQL | Harness-ready | Execution pending |
| Activation exact replay/rebuild reuse fence | Harness-ready | Execution pending |
| Activation receipt unique rebuild constraint | Harness-ready | Execution pending |
| Activation `NodeUpdated` + `NodePublished` durable pair | Harness-ready | Execution pending |
| Event-plus-receipt PostgreSQL rollback | Harness-ready | Execution pending |
| Cache-generation rotation after committed activation | Source-owned by lifecycle handler | Observation pending |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Run the transport schema, request-contract and PostgreSQL repair harnesses and retain accepted execution evidence.
2. Run the corresponding source guards plus the existing explicit rebuild/activation guards.
3. Add/retain failure evidence for rebuild provenance corruption and reviewed-runtime mismatch.
4. Add/retain activation stale-version, invalid replacement and unpublished-page rejection evidence beyond the stale-current/reuse cases in this PostgreSQL packet.
5. Observe the committed activation lifecycle pair through the real cache invalidation handler and prove route/page/artifact generations rotate only after commit.
6. Retain tenant/browser rollout evidence and keep automatic audit-to-repair absent before any FFA/FBA promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-postgres.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test explicit_artifact_repair_postgres -- --nocapture
cargo test -p rustok-pages --test explicit_artifact_repair_transport_contract -- --nocapture
cargo test -p rustok-pages --test explicit_artifact_repair_request_contract -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport-contract.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-request-contract.mjs
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo commands, formatting, PostgreSQL execution, lifecycle-handler/cache observation, GraphQL/HTTP requests, workflows and CI were intentionally not run.
