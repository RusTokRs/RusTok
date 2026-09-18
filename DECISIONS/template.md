# Title

- Date: YYYY-MM-DD
- Decision status: Proposed | Accepted | Superseded | Rejected
- Implementation status: Not started | In progress | Implemented | Not applicable
- Owners: owning module/team
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

Describe the problem, current state, constraints, and why an architectural decision is required.

## Decision

State the accepted architecture precisely. Use one canonical vocabulary and make ownership explicit.

## Sources of truth and ownership

Identify:
- the canonical owner;
- authoritative persisted/domain state;
- derived projections, caches, indexes, overlays, and transport representations;
- dependency direction across modules.

## Invariants

List the states that must remain true in every implementation.

### Allowed states

Describe supported states and transitions.

### Forbidden states

Describe states that must be impossible, rejected, or fail closed.

## Non-goals

List nearby concerns that this decision intentionally does not own.

## Data, transaction, and concurrency boundary

Define:
- database-enforced invariants;
- transaction boundary;
- revision/optimistic-concurrency rules;
- idempotency/retry requirements;
- destructive-operation semantics.

Use `Not applicable` explicitly when the decision has no persisted or transactional state.

## Context dimensions

State how tenant, channel, locale, principal/auth, policy, trace, timezone, currency, or other result-affecting context participates. Mark non-applicable dimensions explicitly.

## Events and projections

Define event/outbox ownership, projection/index invalidation, rebuild/reconciliation behavior, and whether derived state participates in correctness.

## Failure semantics

Define reject/retry/degrade/reconcile behavior. Security- and policy-sensitive paths must fail closed unless an accepted degraded mode says otherwise.

## Migration and cutover

Describe the canonical cutover, data transformation policy, zero-legacy implications, external compatibility constraints, and supersession links.

## Alternatives considered

Record materially different designs and why they were rejected.

## Verification

State the observable evidence required to prove the decision: database constraints, integration/runtime behavior, transport contracts, projection/rebuild evidence, or other appropriate gates.

## Consequences

List trade-offs, operational impact, and follow-up implementation work.
