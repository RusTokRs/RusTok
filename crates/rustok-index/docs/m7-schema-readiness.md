# M7 tenant schema readiness gate

Status: `source_complete_owner_execution_pending`.

## Purpose

The selected Product vertical already publishes immutable runtime schemas and persists exact
source-owned contracts in tenant-scoped `index_schemas`. Query execution and replay validate the
individual schema they touch, but runtime capability presence alone does not prove that the complete
Product/ProductVariant/SalesChannel set is ready for one tenant.

`PostgresIndexSchemaReadinessStore` adds one bounded fail-closed gate for that cutover boundary. It
does not create or mutate schema registrations; `PostgresSchemaRegistrationStore` remains the only
Index-owned registration API.

## Request contract

`IndexSchemaReadinessRequest` requires:

- one non-nil tenant UUID;
- one non-empty explicit schema set;
- at most 64 exact `SchemaRef` values;
- no duplicate references.

The request sorts references deterministically. Every requested reference must exist in the immutable
runtime `SchemaRegistry` before storage is touched.

## Persisted readiness contract

The store reads the entire requested set with one tenant-scoped storage statement. For every exact
reference, readiness requires all of the following:

1. one persisted `index_schemas` row exists for the tenant;
2. `status` is exactly `active`;
3. `schema_fingerprint` equals the immutable runtime registry fingerprint;
4. `schema_json` equals the immutable runtime registry contract.

Any missing, inactive, fingerprint-mismatched, or contract-mismatched schema rejects the complete
request. The gate returns no partial-success receipt.

A successful `IndexSchemaReadinessReceipt` contains only the tenant identity and deterministic exact
schema/fingerprint pairs that were checked. It is an observation of persisted readiness, not a new
durable authorization token and not a substitute for source registration or schema-application
leases.

## M7 use

Before an authoritative Product graph consumer or Storefront cutover is admitted for a tenant, the
caller can require the exact selected schema set, including the currently selected contracts:

- `rustok-product::product@2`;
- `rustok-product::product_variant@2`;
- `rustok-channel::sales_channel@1`.

The gate is generic and accepts no Product-domain types. Future selected modules can use the same
contract without Index-core changes.

## Deliberate limits

This slice does not:

- register missing schemas automatically;
- apply secondary indexes or schema-application jobs;
- persist a separate readiness flag that could become stale;
- start a task, scheduler, retry loop, or broker consumer;
- add Product Index typed wire events;
- bypass the event-contract digest admission gate;
- admit Product-to-SalesChannel relation semantics;
- claim live PostgreSQL/reference equivalence, freshness, restart, or partition evidence;
- authorize Storefront or production partition cutover by itself.

The Product typed refresh family remains blocked by the stale canonical event-contract digest artifact.
Concrete repair PostgreSQL evidence and the remaining live M7 evidence are still maintainer-owned
execution steps.

## Maintainer verification

```bash
cargo test -p rustok-index schema_readiness --lib -- --nocapture
node scripts/verify/verify-index-schema-readiness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
