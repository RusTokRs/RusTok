# Drift candidate confirmation recheck — 2026-08-06

Status: `source_reviewed_unvalidated`.

## Reviewed scope

The review covered:

- the application confirmation boundary;
- the PostgreSQL materialized observer;
- frozen source/absence registry composition;
- public crate exports;
- documentation and static guards.

No server, GraphQL, HTTP, CLI, MCP, native-admin, scheduler, lifecycle, or repair file is changed by
this slice.

## Confirmed source properties

The confirmer processes one typed candidate and:

- checks exact materialized state before owner calls;
- uses one-key targeted loads only;
- never infers absence from an empty ordinary load;
- accepts only authoritative delete or explicit retained absence evidence;
- double-reads stale absence evidence;
- double-reads orphan source-link and target-absence evidence;
- checks exact materialized state again before returning a confirmed outcome;
- returns only typed confirmed identity/version evidence or a closed not-candidate reason;
- maps dependencies to bounded machine codes without propagating owner/provider/database causes.

## Stale conclusion

A stale entity is confirmed only while the same live materialized key and indexed source version
remain present, and the source twice reports the same absence version through delete or admitted
watermark. The absence version must not be below the indexed version.

An owner upsert returns `SourcePresent`. Changed materialized state returns `MaterializedChanged`.
Missing absence capability or watermark fails closed.

## Orphan conclusion

An orphan link is confirmed only while:

- PostgreSQL still contains the same live source entity/version and exact link name/ordinal/target;
- the current target row remains absent or a positive-version tombstone;
- the owner source twice exposes the same source version and exact link target at the same ordinal;
- the target owner twice supplies the same positive delete/absence version.

A changed source version/link, present target, absent source authority, or changed materialized state
returns a typed not-candidate outcome rather than a finding.

## Composition conclusion

`materialize_postgres_index_drift_candidate_confirmer` reads frozen registries and constructs the
observer/confirmer. It performs no SQL and never calls `extensions.insert`. The capability remains
unmounted.

## Concurrency conclusion

Owner sources and PostgreSQL do not share one transaction. Double observation bounds the race window
but cannot make the result cross-owner atomic or retained. Any persistence adapter must revalidate
its write-time assumptions and remain idempotent.

## Open boundaries

Still open are:

- deterministic confirmed-candidate finding projection and persistence;
- write-transaction revalidation;
- lifecycle commands and actor audit;
- public transport;
- scheduling and restart ownership;
- repair;
- retained PostgreSQL and owner-source execution evidence.

## Validation disclosure

No tests, verifiers, formatting, Cargo commands, PostgreSQL scenarios, workflows, or CI were run.
Compilation and runtime behavior are not claimed.
