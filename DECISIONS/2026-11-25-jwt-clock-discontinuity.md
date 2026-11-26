# JWT clock discontinuity protection

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Extends: [JWT emergency revocation](2026-11-25-jwt-emergency-revocation.md), [JWT cluster lease barrier](2026-11-25-jwt-cluster-lease.md), [JWT rolling admission](2026-11-25-jwt-rolling-admission.md)
- Supersedes: None
- Superseded by: None

## Context

A monotonic clock may stop during suspend. A discontinuous DB wall clock
may falsely expire grants, drain horizons or staged verification windows.
A check on the next supervisor tick does not protect resumed request threads.

## Decision

Every managed local JWT lease has both monotonic and wall-clock deadlines,
computed from the same sample before the authority read. Both must remain
strictly in the future before and after crypto. Once observed expired, the
same grant cannot reopen after a small backward wall-clock step. The runtime and authority
each anchor wall time to monotonic elapsed time for their lifetime. A
backward reading relative to the anchor or divergence exceeding one second
latches a fault. A lease also retains the authority's shared fault latch.
No publication, renewal or later valid clock reading clears a local fault.
A restart after operator clock remediation is required. Unmanaged fixed
and process-local non-cluster embedders retain their existing contract.

All lease-grant, admission and ACK reads bracket the database timestamp
with host samples. The read must finish within one monotonic second; the
database time must lie within the host wall interval plus/minus 500 ms.
An out-of-window timestamp latches the authority fault; an inconclusive
slow sample refuses the operation without claiming a detected clock jump.
Both host samples must pass the lifetime clock guard. The same rules apply
to operational observation. A grant response is checked again before local
installation. CAS predicates, registry and conservative drain semantics
remain unchanged: a delayed mutation based on a previously valid sample
cannot advance the sampled expiry time. Never retry with an unverified
timestamp. A clock fault learned by a scrape also closes the local gate
through the shared authority latch.

Use a two-second conservative skew budget when deciding that a historical
verifier has expired or that a staged verifier is still eligible for
promotion. The existing five-second overlap margin remains. The ten-second
local and fifteen-second DB lease lengths are unchanged.

Emergency revocation remains a monotonic security-reducing mutation even
during a clock incident: it neither grants nor expires authority. It may
record the positive DB timestamp from the raw row; the receipt's wall-clock
values must not be interpreted as trustworthy elapsed time during an
incident. Do not use a clock fault as a reason to remove outstanding grants.

## Credential admission extension

The live runtime serializes credential-admission clock samples and latches any
observed backward wall step, even within the lease's one-second tolerance.
OAuth token exchange uses the shared gate at initial admission, after async
reads, before one-shot mutations and commit, and at response creation. Refresh
recovery and replay classification use the same samples. Access extraction
samples before signature verification and after asynchronous policy resolution.
The gate checks the managed lease and emergency/shutdown fences without holding
a synchronous lock across async work. Fixed embedders retain their contract;
explicit security-reducing revocations do not require this admission gate.
This does not establish a durable application DB clock or cross-restart high
water mark. Browser confirmation MAC issuance and verification and all code
issuance paths use the live credential clock. The verified confirmation expiry
survives into the locked mutation, which checks it before consent/code writes
and commit. Code publication rechecks the gate and code expiry after commit.
Application DB corroboration uses the same bounded ClockGuard observation
rules in a separate process-lifetime application latch shared by retention,
OAuth and common access extraction. The clock read uses fresh database wall
time (not PostgreSQL transaction-start time). OAuth checks before expiry-based
admission/classification, on the locked mutation connection and before commit;
standalone consent uses the corroborated host time. Access extraction verifies
the DB clock after policy IO and rejects future-dated OAuth registrations.
Post-commit code/token publication rechecks the local application latch, not
another fallible DB read. Explicit security-reducing mutations are unchanged.
A database fault does not mutate or fence the independent JWT authority.
This assumes one trusted application primary per process. It does not order
DB observations across independent processes or rule out a rollback within
the allowed skew or between observations. Direct registration/login and
refresh now use the same clock checks at admission, locked transitions and
pre-commit, plus local post-commit checks. Direct access expiry is bounded by
the persisted session deadline before signing, and response TTLs account for
commit latency. Failed login bookkeeping uses corroborated application time
without requiring signing authority. Future refresh rotation evidence is
inconclusive and cannot revoke a lineage. Other recovery mutation boundaries
remain follow-up work.

## Destructive retention extension

Auth retention is a separate time-sensitive operation, not JWT signing. The
worker and session-only CLI use one owner implementation with a process-lifetime
application ClockGuard and bounded application DB clock observations inside the purge
transaction before deletion and before commit. The cutoff is fixed from the
earlier of the first host/DB observations minus two seconds. A later observation
cannot move the cutoff forward; a detected rollback or inconclusive sample
aborts the transaction. The CLI retains the same 30-day session history as the
worker; the OAuth family and code windows remain unchanged. This needs no JWT
signing material. A process restart with coherently incorrect host and DB time
remains outside the guarantee; do not run manual cleanup during a clock incident.

## Durable application admission extension (2026-11-26)

The Auth-owned application primary contains a singleton
`auth_application_clock` row with independent host and database UTC
millisecond high-water marks. Before selected credential admission or
retention operations, a short independent transaction serializes on this
row (PostgreSQL row lock, SQLite writer reservation), then samples and
corroborates both clocks using the existing ClockGuard. A conditional
monotonic update is committed before the domain transaction begins. A
rejected credential or rolled-back domain operation cannot erase that
committed sample. Missing or invalid state denies admission; no runtime
re-enrollment. A database reading below its persisted floor trips the
local sticky guard. A host below the persisted host floor is refused
without permanently latching a merely slower replica. Strict ordering
may reduce availability with skewed replicas; never lower either floor
to recover availability.

In-domain checks read the floor before corroboration without taking the
global writer lock. They must not open a nested pooled transaction or
hold the clock writer lock while acquiring business locks. Access
extraction persists a second admission observation after policy IO.
Recovery confirmations use the checked clock after mutation locks and
before commit. Explicit revocation paths remain independent of admission.
This is deliberately the first durable application-clock boundary, not
a universal time service or proof of expiry non-resurrection: observations
inside domain transactions and after the last independently committed
sample are not yet independently durable, and a process crash in that
window can lose their history. Millisecond precision, rollback of the
application store, coherent false time at first initialization, and
independent JWT authority clock continuity remain separate limits.
No claim is made about commit-time deadlines, WebSocket revalidation,
or complete coverage of all credential consumers. The singleton is a
write-contention point; production capacity and multi-replica tests are
required before readiness promotion.

Stop older binaries, apply the Auth application migration (not the
independent JWT authority schema), confirm synchronized host and primary
time, then deploy current readers/writers. Never reset or restore the
floor for incident recovery. Correct clocks and wait until both floors
are reached; restart a locally latched process only after remediation.
Schema down is only for disposable verification. Source tests for
persisted samples, business rollback, guard recreation, and missing
state are authored, not executed.

## Recovery and limits

The existing host supervisor terminates on authority read errors; local
crypto and readiness fail closed independently before supervisor shutdown.
Metrics expose separate local and authority clock-fenced gauges; a
content-free warning is emitted on first fault per latch. A missing target
still needs external scrape monitoring. Stop older binaries before use;
the original lease protection needs no schema migration; the durable application admission extension requires its Auth migration.

This is detection with bounded assumptions, not a trusted distributed time
service. All participating hosts must be independently synchronized to UTC
(with absolute error at most 500 ms), have bounded clock-rate error, and
use the trusted primary. A coherent jump of all clocks, a wall clock and
monotonic clock both stopping while external time advances, manipulation
of the host/DB, a jump and reversal between samples, or a freshly restarted
host with a false time source cannot be ruled out by these local checks.
Never restart through an unresolved time incident or restore an old database.
During recovery correct independent clocks first and allow at least the
conservative lease window to elapse on a reliable external clock before
restarting an affected cluster. The [independent JWT authority](2026-11-25-jwt-independent-authority.md) removes the application DB from the production JWT authority path. Rollback of that independent store remains separate.

## Verification

Source regressions cover forward/backward DB skew, excessive query interval,
host wall rollback, stopped monotonic time, sticky fault after normalization,
both lease deadlines, post-crypto fencing and refusal of subsequent
installation. Run scoped Auth/server checks, promtool and independent-pool
PostgreSQL/VM clock/suspend fault-injection tests before deployment.
