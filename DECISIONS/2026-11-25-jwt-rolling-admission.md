# JWT rolling replica admission

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Extends: [JWT cluster lease barrier](2026-11-25-jwt-cluster-lease.md)
- Supersedes: None
- Superseded by: None

## Context

Draining every generation prevents incompatible concurrent crypto but also
interrupts additive verification staging and planned signer promotion.
The host already owns a short database-backed crypto lease. Record the
actual prepared metadata for each lease grant, rather than counting
processes from a heartbeat or acknowledging the file before applying it.

## Decision and invariants

A publisher has a random process-incarnation UUID. The Auth singleton
records outstanding grants by incarnation and generation, including the
canonical four-purpose verification metadata and DB deadline. An outstanding
grant is a conservative acknowledgement of the maximum crypto capability
that can be enabled locally; a grant lost before local installation delays
promotion but cannot make an unsafe promotion possible. A new generation grant is acquired before atomically installing both keys
and deadline; the old local gate remains intact if acquisition fails.
Only after the local write fence has drained old crypto and installed the
new generation does the same incarnation acknowledge replacement and
remove its older grants via CAS. Renewal retries a contested ACK. Shutdown
or lost connection alone cannot remove a grant before its DB deadline.

A rolling transition must preserve every still-verifiable key and legacy
mapping of the preceding snapshot, with no shortened historical cutoff.
Both snapshots must accept each other's active signer, using the exact
algorithm, ID and canonical verification identity, until the conservative
overlap horizon plus five seconds. For the outgoing active signer,
the operator must additionally retain verification for the maximum remaining
token lifetime; the overlap predicate alone does not establish that TTL.
New verification-only material may be added without changing the signer.
Promotion of a staged signer must pass the same predicate against every
outstanding lease, including older generations. If the immediate durable
predecessor is compatible but a still-live older grant is not, return
AwaitingReplicas without closing renewal of the current generation.
Old-generation grants cannot renew and eventually expire unless their
own incarnation acknowledges a newer applied generation. Incompatible
changes use the existing conservative drain barrier. No assumption of
a predetermined replica count or healthy disconnected replica is made.

## Persistence and concurrency

The registry is a bounded JSON map in the same singleton row. Generation,
tag, horizon, drain deadline and exact previous registry bytes are compared
by CAS for grants and publication. This avoids a separate membership race.
The aggregate lease horizon is preserved across rolling publications.
The migration records any pre-registry anonymous horizon separately;
rolling publication waits until it expires. Registry entries are bounded
to 256 outstanding incarnation/generation grants and 4 MiB; capacity
denies renewal without removing live entries. Expired records are pruned
on successful writes. This deliberately small-cluster control-plane
implementation trades write amplification for one serialization point;
larger clusters require a separately verified normalized store.
The DB primary is trusted, as in the prior lease ADR; no secrets or
raw signing material are recorded. Static generation zero still uses drain.

## Rollout and failure

Stop binaries that do not register grants and apply the Auth migration
before starting this version. Use atomic file distribution in order:
enroll the old signer, add the next verification key, distribute that
generation, then publish the next signer with the former signer retained
as history. Every still-authorized holder must be compatible before
promotion. An unreachable host is fenced after its lease; it may not
renew until receiving the current document. Retry unchanged waiting
candidates. On ambiguous database failure the host exits. Candidate
grant/install failure does not discard an existing valid local gate.

The earlier DB clock/monotonic clock assumptions remain mandatory.
This is a rolling compatible path, not a universal zero-downtime
guarantee: delayed file distribution can fence individual replicas,
capacity/contention can deny grants, incompatible removal drains,
and an operator must provision a sufficient historical JWT TTL window.
[Emergency key revocation](2026-11-25-jwt-emergency-revocation.md) extends this contract.
Clock discontinuity protection, database rollback protection and root/algorithm migration remain open.

## Verification

Source regressions cover additive staging, wait for an old grant, promotion
while staged replicas continue crypto, atomic local activation, ACK only
after local installation, disconnected holder expiry, stale renewal refusal,
incompatible removal and legacy drain. Scoped compilation
and independent-pool PostgreSQL/network-partition runtime evidence remain
pending.
