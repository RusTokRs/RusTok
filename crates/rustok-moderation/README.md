# rustok-moderation

`rustok-moderation` is the cross-domain owner for moderation reports, cases,
decisions, application orchestration, appeals, receipts, and auditable enforcement.

## Ownership

The module owns:

- user, domain, automated, and system reports;
- moderation cases and report deduplication;
- queue assignment and case revision;
- immutable moderation decisions, policy snapshots, and typed effect persistence;
- durable decision-application intent, retry, and applied evidence;
- idempotency receipts;
- moderation events, appeals, and cross-domain audit history.

The module does not own forum topics, blog posts, comments, groups, group memberships,
reviews, products, marketplace listings, seller profiles, messages, media assets, user
profiles, or the current enforcement state stored by those owners.

A domain owner keeps authority over its own object and applies a validated decision through
`ModerationSubjectCommandPort`. The moderation owner never updates another module's tables
and never treats a decided case as an applied domain mutation.

## Neutral contract

`rustok-moderation-api` owns the cross-domain contract so domain modules do not depend on the
moderation persistence owner. It contains:

- subject and scope identity;
- reason and decision kinds;
- typed/versioned decision effects, including expiry and bounded canonical capability sets;
- `ApplyModerationDecisionCommand` and `ModerationDecisionApplication`;
- `ModerationSubjectCommandPort`;
- host-composed subject-adapter and factory registries.

The neutral crate contains no SeaORM entities, migrations, owner services, queues, or
transports. `rustok-moderation` temporarily re-exports moved contracts for Rust source
compatibility. New domain adapters depend only on `rustok-moderation-api`.

Duplicate `(subject_module, subject_kind)` registrations and factory/adapter key mismatches
fail startup. Missing adapters remain unavailable/retryable and never imply successful
application.

## Decision effects

New decisions require a v1 `ModerationDecisionEffect` compatible with their decision kind.
The effect enters command identity and immutable decision hashing, and is persisted in
`moderation_decision_effects` in the same owner transaction as the decision.

Supported effect families include no mutation, visibility change, lock, interaction
restriction, edit/publication requirements, subject suspension with optional expiry,
escalation, and account-sanction recommendation. Capability sets are bounded, unique, and
canonically ordered.

Historical decisions created before effect persistence remain readable with `effect: None`.
They are not eligible for subject-adapter dispatch until explicit re-review or a truthful
migration; no permanent sanction is inferred.

## Boundary

```text
forum/blog/comments/groups/reviews/marketplace/media/messages
                              |
                 report / subject reference
                              v
                    rustok-moderation
               reports -> cases -> decisions
                              |
               durable application operation
                              v
             rustok-moderation-api registry
                              |
                              v
                    authoritative owner
```

Cross-domain references are immutable logical references:

- `subject_module`;
- `subject_kind`;
- `subject_id`;
- `subject_revision`;
- typed scope kind and optional scope ID.

There are no foreign keys from moderation tables to domain-owned tables.

## Groups compatibility

Groups is the reference membership-scoped integration:

- moderation owns case, decision, application, retry, and appeal workflow;
- Groups owns membership/access enforcement, expiry evaluation, membership revision,
  domain receipts, and domain audit;
- membership decisions use subject kind `group_membership`, membership UUID, and a
  monotonic membership revision;
- group-local scope carries the group UUID;
- Groups stores bounded decision provenance, not copied case notes or policy snapshots;
- moderation admin owns queues/cases; Groups UI may expose current local enforcement state
  and authorized direct domain actions.

## Current source slice

The current source provides:

- report/case/decision owner services and FBA read/command ports;
- neutral subject/scope/effect/application contracts;
- explicit adapter/factory registries;
- typed decision-effect validation, hash binding, persistence, and truthful legacy reads;
- tables and migrations for reports, cases, links, decisions, effects, receipts, and events;
- repository-backed receipt-first report/case/decision operations;
- source boundary guard `scripts/verify/verify-moderation-api-boundary.mjs`.

Durable decision-application operations, host materialization, RBAC, outbox, admin FFA,
Groups enforcement adapter, appeals, and automated providers remain subsequent slices. See
`docs/implementation-plan.md` for canonical order and evidence gates.
