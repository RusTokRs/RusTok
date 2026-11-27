# Deployment readiness is independent of tenant serving policy

- Date: 2026-11-27
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-modules; deployment topology and node-agent adapters; server runtime composition
- Extends: [Durable module node reconciliation ledger](./2026-08-14-module-node-reconciliation-ledger.md), [Effective module policy decision](./2026-07-22-effective-module-policy-decision.md)
- Supersedes: the implicit equality between the deployment ledger revision and every tenant effective-policy revision, not the ledger or policy owner
- Superseded by: None

## Context

The node ledger has one global desired/observed head. Effective module policy is
tenant-scoped and includes runtime and tenant-specific intent. Requiring the
same digest for these independently varying authorities makes simultaneous
tenant serving impossible. Simply ignoring the readiness revision would accept
unattributed or stale observations.

## Decision and sources of truth

Keep one deployment ledger. The authenticated topology operator supplies a
deployment intent digest, not tenant authorization. In the owner transaction,
after locking the current state and loading exact admitted installation
identities, derive a domain-separated SHA-256 digest from the intent, topology
digest and canonical ordered assignment identities. The existing ledger and
agent wire field `policy_revision` carries this owner-derived **deployment
readiness revision**. The operator request field of the same name carries
deployment intent; it is not echoed as authority. The owner persists intent
separately and emits the derived revision in existing events.

Serving requires both authorities: freshly evaluated tenant effective policy
and a converged owner-issued readiness set for the same node and every exact
artifact in the permitted dependency/co-requisite closure. Tenant admission
tokens remain bound to the exact tenant effective-policy revision. Agents
neither receive nor derive tenant permissions. The owner reconstitutes and
checks the deployment digest from the complete stored assignment set and
requires exact live identity and observed assignment evidence.

## Invariants and concurrency

The desired and observed heads must agree and be converged; an incomplete,
legacy, ambiguous, altered or superseded set denies serving. All assignments
must echo the derived readiness revision; a deployment revision is never a
tenant capability or a substitute for current tenant selection, security
state or permission checks. The runtime compares the state revision and both head IDs before and after
the closure read. Serving leases now retain the converged head/state
revision, owner-derived digest, node identity and exact observation revisions
for every required artifact, captured in one readiness read snapshot.
SQL effects hold the shared reconciliation state row lock after the mixed
selection fences and before the composition lock (SQLite already owns a
write reservation). The effect reconstitutes the digest and validates the
complete required live/observed assignments under the same lock; requests,
claims, heartbeats and reports hold the exclusive state row lock before
ledger mutations. A restored head with a different state revision or changed
assignment observation denies the old lease. Fresh preflight compares the
same evidence; no additional schema or agent wire change is required. The producer writes intent,
derived revision, assignments, receipt, state and outbox in the existing owner
transaction. Existing idempotency semantics remain tied to the original
request; replay does not renew readiness.

## Migration and cutover

Add a nullable constrained deployment intent digest to the reconciliation
aggregate. NULL identifies legacy evidence and never authorizes the new serving
path. Preserve historical rows; stop old writers, apply migration, deploy
producer and consumers together, then submit a fresh topology request with a
new idempotency key and wait for convergence. Old exact request replays remain
historical receipts. No conversion can attest old observations to the new
contract. Downgrade is denied. Existing agent framing is unchanged; coordinate
rollout of owner and transport components.

## Context dimensions

Tenant, actor, installation selection and current effective-policy revision
remain serving authority. Node ID, exact immutable artifact identities, topology
and deployment intent define readiness. Channel and maintenance belong to
tenant serving policy, not deployment readiness. Locale, currency and timezone
do not determine readiness.

## Events, failure semantics and non-goals

Existing reconciliation events carry the derived deployment revision; tenant
policy events retain their own revision semantics. Storage and invalid
evidence errors do not grant authority. A pending replacement deployment
temporarily denies artifact serving until convergence. This is intentionally
conservative; zero-downtime overlapping deployment generations, ongoing worker
liveness, request context wiring, non-SQL effect commit fencing and continuous
runtime liveness remain separate work. Native Auth/RBAC
availability is unchanged.

## Alternatives and verification

A tenant ledger per tenant duplicates infrastructure observations, multiplies
agent work and fails to model shared platform installations efficiently.
A revision-free gate cannot establish provenance. The selected design retains
one owner-issued readiness identity and independently evaluates tenant policy.
Required verification: SQLite and PostgreSQL migration, legacy denial,
digest recomputation and tamper detection, desired/observed divergence,
cross-tenant selection and differing policy revisions, transport compatibility,
real concurrent reconciliation and cutover. Source tests are authored; runtime
verification and full multi-tenant integration evidence remain outstanding.
