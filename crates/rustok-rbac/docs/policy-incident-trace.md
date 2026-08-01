# RBAC policy incident trace packet

## Purpose

This packet is a dedicated integration scenario for one stale authorization incident.
It connects the evaluator decision, authoritative relation state, process-local
permission cache, durable invalidation generation, watchdog recovery action, and the
post-recovery evaluator decision without introducing a second authorization or cache
implementation.

The fixture uses the existing `setup_test_db_with_migrations` helper and therefore runs
against a fresh single-connection in-memory SQLite database. It is not PostgreSQL
concurrency evidence, multi-replica evidence, or Redis transport evidence. Those P0
gates remain open.

## Incident scenario

The source-ready test is
`apps/server/tests/rbac_policy_incident_trace.rs`.

1. A tenant user receives the canonical Admin role and the normal RBAC resolver warms
   a permission snapshot for `settings:manage`.
2. One transaction deletes the user's role relation and reserves the next durable RBAC
   invalidation generation.
3. The transaction commits, but the normal local/Redis publisher is intentionally not
   called. This represents a missed post-commit publication rather than a rollback or
   an invalid relation write.
4. Before the five-second durable-generation watchdog poll, the canonical evaluator
   observes the stale cache hit and still allows `settings:manage`.
5. The test reads the authoritative relation-backed permission set and records that it
   no longer grants the required permission.
6. The durable generation remains ahead of the process-applied generation.
7. The existing watchdog observes `generation_advanced`, performs the existing
   process-wide permission snapshot clear, advances the applied checkpoint, and
   increments the bounded recovery and full-clear counters.
8. The canonical evaluator resolves again and denies the permission from authoritative
   relations.

## Packet fields

The emitted `rbac policy incident packet` event contains only bounded diagnostic facts:

- a per-run incident UUID;
- tenant and user identifiers already used by RBAC decision diagnostics;
- required permission identifier;
- evaluator result before and after recovery;
- authoritative assigned-role and permission counts plus a required-permission boolean;
- cache-hit boolean and cached permission count, never the complete permission list;
- durable and applied generation values;
- the closed recovery action `generation_advanced_full_clear`;
- recovery and full-clear counter deltas.

Passwords, bearer tokens, OAuth secrets, session identifiers, raw cache keys, and full
permission or relation lists are not written to the packet.

## Ownership and boundaries

- `rustok-rbac` remains the relation and generation-store owner.
- `apps/server` continues to compose the canonical resolver, Moka cache, and watchdog.
- `rustok-telemetry` continues to own the bounded generation/recovery metrics.
- The test does not call the invalidation publisher, manually clear the cache, create a
  second evaluator, or normalize the durable/applied generation gap.
- The only recovery actor is the existing durable-generation watchdog.
- The SQLite packet does not replace retained real-PostgreSQL concurrency or
  two-replica Redis outage/restart/missed-publication evidence.

## Source evidence

Machine-readable source evidence is recorded at
`crates/rustok-rbac/contracts/evidence/rbac-policy-incident-trace-source.json`.
The focused source guard is
`scripts/verify/verify-rbac-policy-incident-trace.mjs`.

## Execution

Targeted maintainer commands:

```bash
cargo test -p rustok-server --test rbac_policy_incident_trace -- --nocapture
node scripts/verify/verify-rbac-policy-incident-trace.mjs
```

The test should be retained with its `rbac policy incident packet` log output on the
same revision. The source file and evidence JSON do not claim that SQLite, PostgreSQL,
Rust, Node, formatting, workflows, or CI have run. The broader `core/rbac` cursor
remains `in_progress` until its compile, PostgreSQL concurrency, multi-replica Redis,
management-flow, and FFA/FBA gates are satisfied.
