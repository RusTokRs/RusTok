# M6 locale-optional drift-finding entity scope

Status: `source_complete_owner_execution_pending`

## Purpose

The bounded drift digest producer accepts every valid `EntityKey`, including schemas whose locale
mode is `None`. The previous PostgreSQL finding contract could persist only locale-bearing entity
scopes. This slice closes that mismatch without changing any existing locale-bearing finding key.

## Typed scope

`IndexDriftFindingScope` retains the existing locale-bearing variant:

```rust
Entity { schema, entity_id, locale }
```

and adds an explicit locale-free variant:

```rust
EntityWithoutLocale { schema, entity_id }
```

The variants are intentionally distinct. A caller cannot represent locale absence with an empty,
invented, or sentinel `LocaleKey`. Both entity variants require a positive schema version and a
non-nil entity UUID.

The inspector decodes an entity row with `locale_key = NULL` as `EntityWithoutLocale`. A non-null
locale must still parse to a canonical `LocaleKey` whose canonical bytes equal the stored bytes.
Global and schema findings continue to reject every entity or locale column.

## Finding-key compatibility

The finding-key domain remains `index_drift_finding_key_v1`.

For locale-bearing entity scopes, the exact old component sequence is unchanged:

1. tenant UUID;
2. check name;
3. `entity` scope tag;
4. schema module, entity, and version;
5. entity UUID;
6. canonical locale bytes.

The source-only compatibility test independently reconstructs that legacy sequence and compares it
with the current request key.

A locale-free entity appends one length-prefixed NUL byte as its final component. Canonical locale
identifiers cannot contain NUL, so the new scope cannot collide with any valid locale-bearing scope.
Expected digest, actual digest, severity, lifecycle state, timestamps, and details remain excluded
from the key.

## Database migration

`m20260804_000005_relax_index_finding_locale_scope` is a forward migration after the existing M3 and
reconciliation-recovery migrations. The historical M3 migration remains unchanged.

On PostgreSQL the migration discovers the one existing table CHECK that binds `scope_kind`,
`entity_id`, and `locale_key`, drops it by its actual catalog name, and installs a named relaxed
constraint. Entity rows still require module, entity, schema version, and entity UUID; locale becomes
optional. All independent locale length/canonical-shape, digest, state, and closure constraints remain.

SQLite is test-only for this adapter. Its migration rebuilds the table, copies every column, and
recreates the unique finding-key and open-finding indexes with the same relaxed scope rule.

## Producer adapter

`PostgresIndexDriftFindingWriter` remains the recorder implementation used by
`IndexDriftDigestProducer`:

- `EntityKey { locale: Some(locale) }` maps to `Entity`;
- `EntityKey { locale: None }` maps to `EntityWithoutLocale`;
- both use the same transactional create/refresh/reopen/suppress lifecycle;
- raw records, SQL, database errors, and transport context do not cross the recorder DTO.

## Evidence ownership

The environment-gated PostgreSQL harness applies every real Index migration, writes one locale and
one locale-free finding for the same entity identity, proves distinct keys and `NULL` persistence,
reads the locale-free scope through the real inspector, and refreshes the same durable finding.

It remains retained execution evidence until the repository owner runs it.

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test drift_finding_locale_scope_postgres_test \
  -- --nocapture --test-threads=1

cargo test -p rustok-index --test drift_finding_locale_key_contract
node scripts/verify/verify-index-drift-finding-locale-scope.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo commands, PostgreSQL scenarios, workflows, or CI were executed
by the implementation agent.

## Still open

- one production source/materialized snapshot reader under a truthful owner boundary;
- entity discovery and bounded missing/stale/orphan diagnosis;
- automatic convergence handling and finding resolution;
- resolve/ignore commands with actor/reason audit;
- targeted/full/shadow repair and before/after evidence;
- scheduler, transport, and graceful-shutdown composition;
- retained execution evidence.
