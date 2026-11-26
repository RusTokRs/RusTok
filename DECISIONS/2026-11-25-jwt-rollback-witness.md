# JWT authority rollback witness

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition; Auth operator CLI
- Extends: [Independent JWT authority](2026-11-25-jwt-independent-authority.md)
- Supersedes: None
- Superseded by: None

## Context

An older authority snapshot can contain a validly authenticated ledger but
omit revocations, replica grants or drain barriers. A generation-only
checkpoint misses same-generation revocations and lease mutations. A
checkpoint stored in the same restore domain cannot detect its own rollback.

## Decision

Require a third, independently administered PostgreSQL store for the
production JWT authority. Configure `RUSTOK_JWT_WITNESS_DATABASE_URL`
alongside the existing authority URI and pinned UUID. Both URIs require
an explicit TCP endpoint and exactly one `sslmode=verify-full`; only TLS
certificate options are additionally accepted. No partial configuration,
automatic provisioning or production fallback. Different URI strings and
schema checks are not proof of separate physical/administrative failure domains.

The authority owns the runtime ledger. The witness holds its latest exact
metadata snapshot, authenticated with a purpose-separated HKDF/HMAC key
bound to the authority UUID. It includes a nonnegative monotonic revision,
generation, envelope/tag, all lease/drain/untracked horizons and the complete
replica registry. It holds no signing material. Checked authority DB time
is now also authenticated as a witness-owned monotonic clock floor. Clock-only
updates keep the ledger revision; every conditional ledger mutation, including grant, ACK, drain,
admission and revocation, advances the revision even when the generation
does not change. All reads compare both revision and exact metadata.

Lock the authority singleton first and witness singleton second, using
PostgreSQL row locks. Validate the current checkpoint, recheck the expected
revision and original CAS predicate, apply the authority mutation in its
uncommitted transaction, advance its revision, then update the witness.
Force synchronous commit on write transactions. Commit the witness before
committing the authority. Return success or install a local grant only
after both commits. An interrupted commit can leave the witness ahead
but cannot authorize a grant on a mismatched pair. A stale revision returns
CAS contention. Existing time checks and bounded local lease remain in force.

A detected mismatch, missing/invalid checkpoint or uncertain write commit
(including cancellation of an in-flight commit future before both outcomes
are confirmed) latches a local witness fault; an installed lease checks the same latch
before and after JWT crypto. The supervisor exits on authority errors.
Other disconnected holders retain at most the existing bounded lease.
Connection failures before a checkpoint comparison may close the host
through existing supervisor/lease behavior. No cross-process instantaneous
revocation or network calls on the JWT request path are promised.

## Clock-floor extension

Checked reads take both existing locks, resample authority DB time under
those locks, corroborate it against host samples, reject a timestamp below
the authenticated floor, and commit any advance to the witness before
returning. Time-sensitive ledger mutations repeat the check and carry the
floor forward; emergency revocation does not interpret or advance time.
The same cancellation guard fences uncertain clock-only writes. Ledger
comparison/recovery excludes the witness-only field from the authority-row
projection but authenticates and retains it in the witness itself.

Stop old writers and drain leases before cutover. An authenticated legacy
checkpoint lacking the field is enrolled at the first checked observation
of a trusted clock; no earlier history can be reconstructed. Old binaries
reject the new payload; never downgrade or remove the field. A rollback
of the witness itself remains outside the trust model. See the
[clock ADR](2026-11-25-jwt-clock-discontinuity.md).

## Offline initialization and recovery

Stop every old host and operator writer and wait for the conservative lease
window and any active drain. Provision an empty witness DB in a third
restore domain with durable WAL and appropriately restricted credentials.
For a fresh deployment import the independent authority first with
`rustok-cli auth jwt-authority-init --confirm`. Existing authority deployments
do not reimport their application row. Initialize once with
`rustok-cli auth jwt-witness-init --confirm`; it creates the authority revision
column and witness table, authenticates the source, and refuses live grants
or drains and existing schemas. The witness is committed first; an interrupted
initialization is fenced, not automatically reseeded.

On rollback or a split commit, stop all writers and wait for leases on an
independent reliable clock. Verify that the witness itself has not been
restored and the configured UUID/root are correct. Preserve evidence and
backups. Run `rustok-cli auth jwt-witness-recover --confirm` against the
pinned authority and witness. Recovery restores only the exact authenticated
checkpoint (including all grants), never lowers an authority revision and
never changes the witness. It can restore the revision column on a pre-witness
PostgreSQL snapshot with the original authority identity. Missing authority
identity or invalid witness is not repaired automatically. Start fresh
processes after recovery; a running process's latch cannot be cleared.
Never initialize a replacement witness from a restored authority.

## Trust and verification boundary

The witness must not share the application or authority restore domain.
Protect its current acknowledged state through independent durable backups,
WAL and synchronous failover where required. A rollback of the witness
together with the authority, a rollback of all stores, compromise of the
root/configuration or unauthorized old binaries is outside this contract.
If the witness is lost or suspected stale, do not resume JWT traffic until
its current state is independently established. Session, refresh-token,
OAuth and RBAC application-data restoration remain separate work.

The write path now serializes across two stores and copies bounded metadata
on every mutation. Benchmark independent-pool PostgreSQL latency/contention
and fault-inject network failures, both commit boundaries, WAL failover,
restore and TLS verification before deploying. Source regressions cover
same-generation rollback, same-revision grant corruption, stale CAS,
recovery preserving revocation and grants, refusal to lower revision and
TLS policy. Compilation, regressions and promtool remain unrun.
