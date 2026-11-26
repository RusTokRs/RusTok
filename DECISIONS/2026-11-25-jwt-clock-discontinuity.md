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
application store and coherent false time at first initialization remain
separate limits. The independent authority extension below covers witnessed
DB observations, not all request-path host clock observations.
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

## Durable independent authority clock (2026-11-26)

Time-sensitive witnessed reads serialize with authority mutations, obtain a
fresh authority DB timestamp after both singleton locks, validate it against
the existing bounded host observation and compare it to the authenticated
witness clock floor. A lower DB timestamp trips the shared authority clock
latch. A higher verified timestamp is durably written to the existing
witness checkpoint before returning the observation for an expiry decision,
including read-only decisions and refusals. The checkpoint field belongs
to the independent witness, not to the restored authority row. Clock-only
writes retain the ledger revision; domain CAS mutations still advance it.
Time-sensitive mutations recheck time after both locks and carry the
nondecreasing floor into the next authenticated checkpoint. An unconfirmed
witness clock write uses the existing cancellation/commit outcome guard.
Raw emergency revocation neither checks expiry nor changes the floor;
offline ledger recovery preserves it exactly.

This is an additive authenticated checkpoint format extension, with no
authority or application DDL. Stop every older authority/witness writer
and wait out all prior leases/drains on a reliable external clock before
deploying. A valid pre-extension checkpoint without the field has no
historical clock evidence; its first checked read enrolls the verified
current DB time. Confirm clock correctness before cutover. Old readers
reject the added field and must not be restarted. Never strip the field,
reset the witness, or restore an older witness to regain availability.
A clock incident requires independently correcting the clocks and reaching
the retained floor, then starting a fresh process. Existing witness
recovery remains required for ledger mismatch or uncertain split commits.

The DB floor has millisecond precision and does not prove the accuracy of
the UTC source; local request-path JWT samples, application late observations,
unwitnessed development fences, simultaneous witness rollback and exact
commit-time expiry remain outside this extension. Checked reads now write
the authenticated witness payload and force synchronous commit, adding
WAL, latency and contention; measure these costs before production.
Application admission writes also force PostgreSQL synchronous_commit=on;
this does not by itself establish synchronous standby failover durability.

## Independently committed application observations (2026-11-26)

The server host configures a dedicated single-connection clock writer pool
after application migrations and before accepting traffic; the session
cleanup CLI configures the same mechanism. PostgreSQL uses the existing
application primary URI and the existing `auth_application_clock` singleton.
The domain connection is sampled first, before taking the clock writer lock.
The writer samples again under the singleton lock, requires the same primary
database/schema/server/relation identity and no recovery replica, rejects
backward database evidence or a lower floor, and commits using
`synchronous_commit = 'on'`. No domain connection is acquired while holding
the clock writer lock. This avoids exhausting the domain pool and permits
late observations during a domain transaction without rolling them back with
that transaction. The writer's fresh millisecond host timestamp is the
decision timestamp, not the prior domain sample or a later unpersisted wall
reading. A bounded commit interval and the live credential gate are still
required. A failed or cancelled clock commit trips the process-local sticky
clock guard; no unknown commit outcome authorizes a domain decision.

The PostgreSQL primary identity comparison is a routing sanity check, not a
cryptographic identity or independent backup witness. All hosts must connect
to the same trusted primary, use the same schema, and reserve one additional
database connection per host/CLI process. Restart only after clock remediation;
do not lower the floor or replace the primary with stale state.

SQLite has a single writer per file. A second pool on the application file
would deadlock when a domain write transaction is held. For durable late
observations use a separately provisioned persistent SQLite file and
`RUSTOK_AUTH_SQLITE_CLOCK_DATABASE_URL=sqlite:///absolute/path/clock.db?mode=rw`.
This is optional development/embedded deployment configuration; ordinary
SQLite without a sidecar and unmanaged embedders retain admission-only
durability. Never silently fall back after a sidecar has been bound.
Provision offline with all old binaries and writers stopped, after applying
the application clock migration. Back up both files, read the current
`host_ms` and `database_ms` from the application singleton, and create an
empty separate clock file with the same singleton schema and exact copied
values. Create the following table in both files and insert the same newly
generated canonical, non-nil UUID into each:

```sql
CREATE TABLE auth_application_clock_sidecar_binding (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    binding_id TEXT NOT NULL
);
-- Insert id=1 and the same operator-generated UUID in both files.
```

Use FULL or EXTRA synchronous SQLite commits. The host refuses the same
canonical file, Unix hard-link aliases, absent/incorrect bindings, missing
clock rows, sidecar floors below the original application floors, or a
sidecar containing application user/session tables. Provisioning is manual,
not a runtime repair or a migration that creates another file. Keep the
sidecar authoritative after cutover; never copy an older application floor
over it, and never restore it from stale backup. Missing configuration on
a bound application file fails closed. A restored pre-binding application
or rolled-back sidecar is not an independently witnessed state and cannot be
detected reliably by this scheme.

Existing verified-clock paths in session and recovery mutations, OAuth and
retention now use the independently committed writer when configured.
Selected previously unchecked session/registration expiry predicates,
password and email change checks, and OAuth family/recovery decisions also
use it. Access extraction explicitly persists a new sample if the deadline
crosses between its final durable sample and live credential check. The
persisted sample is a bounded decision point, not the exact domain commit
or response instant. In-memory JWT validation, all post-commit publication
observations, WebSocket timers, unsupported/unmanaged callers, rollback of
the application clock store itself, and total power-loss/failover evidence
remain outside the complete expiry non-resurrection claim. Global singleton
contention and the cost of synchronous clock writes still require benchmarks.

Source regressions cover rollback after a sidecar write, restart against a
future floor, binding mismatch, forbidden legacy fallback, and uncertain
commit fencing. Rust compilation, tests, PostgreSQL pool/failover and
SQLite real-file deployment evidence are pending.

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
