# Groups membership enforcement expiry/revoke PostgreSQL evidence contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_membership_enforcement_expiry_postgres.rs` is the PostgreSQL counterpart to the SQLite expiry/revoke evidence packet. It retains executable owner-state evidence that temporary Groups membership suspension expires according to the Groups owner clock without cleanup and that canonical direct revoke preserves the current enforcement projection in place.

The packet uses a unique PostgreSQL schema, all production Groups migrations, `GroupMembershipEnforcementCommandPort`, and `GroupMembershipEnforcementReadPort`. It adds no crate dependency, manifest change, lockfile change, or alternate enforcement implementation.

The canonical Groups plan already lists expiry/revoke and lifecycle-count invariance as open GROUPS-07 evidence. This packet supplies executable source for that existing gate; it does not promote the gate to completed runtime evidence.

## PostgreSQL isolation

Every execution creates a unique schema and connects through a schema-scoped PostgreSQL startup `search_path`:

```text
options=-csearch_path=<schema>,public
```

Using connection startup options matters because SeaORM is pooled: a one-shot `SET search_path` on one session would not isolate later calls that run on another pooled PostgreSQL connection.

The test applies every migration returned by `rustok_groups::migrations::migrations()` inside the isolated schema and drops the schema after the assertions.

## Expiry contract

One lifecycle-active member is suspended through the production direct command with a short future `effective_until`.

Immediately after the command the evidence requires:

- effective status `Suspended`;
- `active_member=false`;
- current projection `is_effective=true`;
- `source_kind=direct_local`;
- membership revision advanced exactly once;
- stored group `member_count` unchanged.

After the Groups owner clock passes `effective_until`, the test performs **no cleanup mutation**. A fresh production read must resolve:

- effective status `Active`;
- `active_member=true`;
- the same current enforcement projection still present;
- `is_effective=false`;
- original expiry still present;
- `revoked_at` still null;
- `source_kind=direct_local` unchanged;
- membership revision unchanged since the original suspension;
- group `member_count` unchanged.

The persistence diagnostic reads only whether `effective_until` and `revoked_at` are present. Effective truth remains owned by the typed Groups read port.

## Direct revoke contract

A second lifecycle-active member receives a non-expiring direct-local suspension through the production command. The owner then calls the production revoke command using the post-suspend membership revision as the CAS precondition.

The evidence requires:

- revoke result effective status `Active`;
- revoke timestamp present;
- membership revision advances exactly once for suspend and once for revoke;
- enforcement projection revision advances in place;
- the projection remains stored with `source_kind=direct_local`;
- `revoked_at` becomes non-null;
- a fresh production read reports the projection as non-effective and membership as active;
- stored lifecycle `member_count` never changes.

The packet intentionally does not test revocation of `source_kind=moderation_decision`; production source forbids that through the direct-local revoke command.

## Timing

The expiry test uses a two-second future expiry and waits slightly beyond that boundary before the second owner read. If the PostgreSQL runtime cannot observe the expected owner-clock transition, the test should fail rather than introducing cleanup, polling, or fallback behavior.

## Execution status

This file is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing the slice. PostgreSQL expiry/revoke runtime evidence therefore remains **maintainer execution pending**.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_membership_enforcement_expiry_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-membership-enforcement-expiry-postgres.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this evidence source.
