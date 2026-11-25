# JWT cluster lease barrier

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Extends: [Durable JWT generation fence](2026-11-25-jwt-generation-fence.md)
- Supersedes: None
- Superseded by: None

## Context

The durable generation alone cannot stop an already-running replica using
old keys. Polling the file or the database without a request-path gate
does not fence a disconnected or paused replica.

## Decision

Use a conservative, database-serialized drain barrier, not a claim of
zero-downtime replica readiness. Every HTTP host, including static mode
(generation zero), must hold a short JWT crypto lease. No request consults
the database directly. Auth checks a local monotonic deadline before and
after each synchronous JWT encode/decode under the existing runtime read
fence. Local publication clears the lease. The host supervisor renews only
after confirming the exact canonical current generation and metadata
against the durable authority. Static mode can lease only while the authority
is empty. Standalone fixed AuthConfig constructors remain unmanaged for
embedders; they are not the HTTP host composition path.

## Authority and concurrency

The singleton row also holds the maximum granted database-clock lease
deadline and a bounded drain deadline. Lease renewal and generation
admission conditionally update that same row. Admission validates the
candidate and its lifetime bindings first. If any old lease can remain
valid, admission closes renewal for 60 seconds and returns Draining.
While draining, no new lease for the old generation can be granted.
Only after the maximum old lease deadline has elapsed may a candidate
commit; the CAS compares generation, tag, lease and drain deadlines and
clears both deadlines on success. There is no fixed replica membership
list or per-replica ACK: all holders, including unknown/disconnected ones,
are accounted for conservatively by the maximum deadline. A drain is not
a reservation for a specific candidate; the first valid candidate to
win the eventual CAS is authoritative. If no candidate completes the
transition, the drain expires and the old generation may resume leasing.

Database time is sampled before the conditional grant; the local deadline
is ten seconds from before the database read, and the DB deadline is
fifteen seconds from its clock sample. A delayed response cannot extend
local authority. This assumes bounded clock-rate error, no forward DB
clock discontinuity, and no host suspend that stops the local monotonic
clock while database time advances. The five-second safety margin is
not a proof against arbitrary clock or infrastructure failure. Use
disciplined clocks and no suspended hosts. The trusted database primary
must serialize all writes; read replicas cannot serve this authority.

## Failure and recovery

The supervisor retries a draining candidate even when the source bytes
are unchanged. A stale host with an unchanged or absent file loses crypto
authority automatically. It can converge when the committed document
arrives; same-generation canonical admission is idempotent. On database
failure the host exits; a drop guard clears local authority even on
cancellation. At startup the host does not bind a listener before admission
and initial lease succeed. A static host exits when renewal is denied.
A file-backed host continues polling but cannot perform JWT crypto without
a current lease. Already-authorized async business IO is not cancelled by
a crypto lease expiry.

## Cutover and non-goals

Apply the additional Auth migration, stop all older/unmanaged host writers,
and then start upgraded replicas. Do not mix versions that ignore the
lease columns. Every generation change can briefly interrupt JWT operations;
this is a deliberate correctness-first barrier. A non-disruptive staged
promotion with explicit replica registration/ACK and a distributed emergency
revocation policy remain follow-up work. No protection against database
restore, hostile administrator, root compromise or arbitrary clock jumps.
No new dependencies or event infrastructure.

## Verification

Source regressions cover old lease drain, renewal refusal, expired lease
crypto refusal, canonical catch-up, generation-zero/static enrollment and
drain expiry. Run scoped Auth/host checks and real independent-pool PostgreSQL
contention, network partition, restart and timing tests before deployment.
