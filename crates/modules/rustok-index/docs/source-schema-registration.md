# Persisted source schema registration

`PostgresSchemaRegistrationStore` is the Index-owned persistence boundary for
owner-published `IndexSchema` contracts. Source modules declare generic schemas but
must not write `index_schemas` directly.

## Registration flow

1. Validate the non-nil tenant and complete `IndexSchema` contract.
2. Calculate the canonical schema fingerprint and semantic JSON payload.
3. Begin one database transaction.
4. On PostgreSQL, serialize the tenant/module/entity identity with a transaction
   advisory lock. SQLite remains contract-test support.
5. Resolve an exact existing version before considering a new version.
6. Return `Unchanged` only when the active stored fingerprint and semantic JSON
   match exactly.
7. Reject same-version contract drift, retired state, and an unregistered version
   lower than the latest persisted version.
8. Insert a new active schema with `ON CONFLICT DO NOTHING`, then verify any race
   winner before returning.
9. Commit before an `IndexMutation` may rely on the schema foreign key.

## Consumer contract

A durable source consumer keeps its in-memory `SchemaRegistry` for validation and
uses `PostgresSchemaRegistrationStore` for tenant persistence. For each relevant
source delivery it must:

1. convert the sealed owner event into a generic mutation;
2. persist or exactly recognize the tenant schema;
3. apply or terminally recognize the mutation through `PostgresMutationStore`;
4. acknowledge the broker delivery only after those durable results commit.

Registration or mutation failure must leave the broker delivery replayable. An
owner projection never becomes the source of authorization or normalized domain
state.

## Failure behavior

- nil tenant: `SchemaRegistrationError::NilTenantId`;
- malformed contract: `InvalidSchema`;
- same version with another contract: `VersionConflict`;
- missing lower version after a newer version: `NonMonotonicVersion`;
- retired exact schema: `SchemaRetired`;
- unsupported backend, malformed stored values, tenant FK failure, or database
  failure: generic `Storage`.

Error displays do not publish schema JSON or storage causes. Operators use owner
logs and retained database evidence for diagnosis.

## Verification

```bash
cargo check -p rustok-index --all-targets
cargo test -p rustok-index schema_registration --lib -- --nocapture
node scripts/verify/verify-index-schema-registration.mjs
```

These commands remain maintainer-run for this slice.
