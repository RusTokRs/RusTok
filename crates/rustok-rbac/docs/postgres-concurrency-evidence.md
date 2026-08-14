# RBAC PostgreSQL concurrency evidence harness

## Purpose

`apps/server/tests/rbac_postgres_concurrency.rs` is the dedicated source-ready
PostgreSQL harness for the remaining `core/rbac` database-concurrency gate. It
uses the real workspace migrations, relation tables, committed role mutation
entry point and durable invalidation-generation allocator. It does not create a
parallel role writer, a test-only lock, or a second generation authority.

The harness is ignored by default because it creates and drops isolated
PostgreSQL databases through an administrative connection. Source presence does
not count as retained runtime evidence.

## Scenarios

### Concurrent replacement of one user role

Two independent PostgreSQL connections call
`RbacService::replace_user_role_committed` concurrently for the same tenant user,
one requesting `admin` and the other `manager`.

The retained execution must prove:

- both committed mutations complete through the production entry point;
- the target-user row lock serializes the writes;
- exactly one tenant role assignment remains;
- the final role is one of the two requested roles;
- the durable generation advances exactly twice.

### Last-active-super-admin serialization

Two active super administrators are demoted concurrently on independent
connections through the same committed production entry point.

The retained execution must prove:

- exactly one demotion commits;
- exactly one demotion is rejected with the canonical last-active-super-admin
  continuity error;
- exactly one super-admin assignment remains;
- the durable generation advances only for the successful mutation.

### Unique monotonic generation allocation

Eight independent transactions reserve and commit RBAC invalidation generations
concurrently through
`rustok_rbac::reserve_permission_invalidation_generation`.

The retained execution must prove that the returned values are unique,
contiguous and equal to the durable committed range immediately following the
starting generation.

## Fixture ownership

Each test:

1. reads `RUSTOK_MIGRATION_SMOKE_ADMIN_URL`, falling back to the repository's
   conventional local PostgreSQL admin URL;
2. creates a process-unique database with `rustok-test-utils`;
3. applies `rustok_migrations::Migrator`;
4. opens two independent target connections;
5. exercises only production RBAC mutation/generation APIs;
6. closes the target connections and drops the isolated database.

There is no SQLite fallback. The tests do not update the generation row with raw
SQL and do not issue manual role-lock SQL.

The three top-level harness cases must run serially at the libtest layer. Each
case creates and fully migrates its own database, so running all three fixture
setups concurrently can exhaust the PostgreSQL service's shared lock-memory
budget before the RBAC assertions execute. Serial top-level execution does not
weaken the evidence: the role-replacement, super-admin and generation scenarios
retain their internal synchronized concurrency of two, two and eight operations,
respectively.

## Execution

Run the ignored PostgreSQL harness explicitly with debug info disabled and the
migration-heavy top-level cases serialized:

```bash
CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p rustok-server --test rbac_postgres_concurrency -- --ignored --nocapture --test-threads=1
```

Run the focused source guard separately:

```bash
node scripts/verify/verify-rbac-postgres-concurrency-source.mjs
```

Retain the test output and exact commit SHA before marking PostgreSQL concurrency
verified. Until then, the evidence remains `source_ready_unvalidated`.

This harness does not prove Redis delivery, two-replica recovery, missed PubSub
catch-up, CLI repair propagation, formatting, compilation, Clippy, or broader
RBAC runtime behavior. Those gates remain open.