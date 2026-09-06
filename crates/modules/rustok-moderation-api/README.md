# rustok-moderation-api

Neutral contracts shared by the moderation owner and authoritative domain owners.

## Ownership

This crate owns no reports, cases, decisions, persistence, migrations, queues, appeals,
or domain enforcement state. It contains only:

- moderation subject and scope references;
- reason and decision kinds;
- typed, versioned decision effects;
- decision-application command/result contracts;
- `ModerationSubjectCommandPort`;
- host-composed subject-adapter and factory registries.

`rustok-moderation` owns moderation workflow persistence. Domain modules such as Groups,
Forum, Blog, Pages, Marketplace, Media, and Profiles depend on this crate when exposing an
authoritative decision-application adapter. They must not depend on moderation entities or
services.

## Adapter registry

Adapters are keyed by `(subject_module, subject_kind)`. Duplicate keys fail registration.
Factories are materialized only after `HostRuntimeContext` exists. A missing adapter is not
success and must remain retryable in moderation orchestration.

## Effect contract

`ModerationDecisionEffect` carries an explicit schema version and a typed action. The effect
is validated against `ModerationDecisionKind`, included in immutable decision hashing, and
persisted separately from policy snapshots. Capability sets are bounded, canonical, and
must not use arbitrary owner JSON as an enforcement contract.

Historical decisions created before effect persistence are exposed by the moderation owner
with `effect: None`; they cannot be dispatched to domain adapters without explicit re-review
or a truthful migration.
