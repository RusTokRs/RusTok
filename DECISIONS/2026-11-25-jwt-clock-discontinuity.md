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
strictly in the future before and after crypto. The runtime and authority
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

## Recovery and limits

The existing host supervisor terminates on authority read errors; local
crypto and readiness fail closed independently before supervisor shutdown.
Metrics expose separate local and authority clock-fenced gauges; a
content-free warning is emitted on first fault per latch. A missing target
still needs external scrape monitoring. Stop older binaries before use;
no schema migration is required.

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
