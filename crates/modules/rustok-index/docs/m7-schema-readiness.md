# M7 tenant schema readiness gate

Status: `source_complete_owner_execution_pending`.

## Purpose

The selected Product vertical publishes one current Product contract, one current ProductVariant
contract, and one current SalesChannel contract. Persisted tenant `index_schemas` state must match that
exact runtime-selected set before authoritative graph use.

`PostgresIndexSchemaReadinessStore` is a bounded fail-closed observation gate. It does not register or
mutate schemas; `PostgresSchemaRegistrationStore` remains the Index-owned registration API.

## Request contract

`IndexSchemaReadinessRequest` requires a non-nil tenant UUID, a non-empty explicit schema set of at
most 64 exact `SchemaRef` values, and no duplicates. References are sorted deterministically and must
exist in the immutable runtime `SchemaRegistry` before storage is touched.

## Persisted readiness contract

For every requested current reference, readiness requires:

1. one persisted tenant `index_schemas` row;
2. `status = active`;
3. exact runtime `schema_fingerprint`;
4. exact runtime `schema_json`.

Any missing, inactive, fingerprint-mismatched, or contract-mismatched row rejects the whole request.
There is no partial-success receipt.

The generic `SchemaRef` contains a positive numeric schema key because Index storage uses it in entity,
link, inbox, and checkpoint identities. The Product distribution no longer publishes multiple Product
or ProductVariant compatibility contracts behind that key.

## M7 use

Before authoritative Product graph use, the caller requires the exact three runtime-selected schema
references for:

- Product;
- ProductVariant;
- SalesChannel.

The gate itself remains domain-neutral and accepts no Product or Channel types.

## Deliberate limits

This gate does not register missing schemas, apply secondary indexes, persist a separate readiness
boolean, start workers, bypass event-contract admission, prove relation freshness, or authorize
Storefront cutover by itself.

Concrete PostgreSQL readiness/equivalence/freshness evidence remains maintainer-executed.

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
