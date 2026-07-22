# rustok-moderation

`rustok-moderation` is the cross-domain owner for moderation reports, cases,
decisions, application orchestration, appeals, receipts, and auditable enforcement.

## Ownership

The module owns:

- user, domain, automated, and system reports;
- moderation cases and report deduplication;
- queue assignment and case revision;
- immutable moderation decisions and policy snapshots;
- durable decision-application intent, retry, and applied evidence;
- idempotency receipts;
- moderation events, appeals, and cross-domain audit history.

The module does not own forum topics, blog posts, comments, groups, group memberships,
reviews, products, marketplace listings, seller profiles, messages, media assets, user
profiles, or the current enforcement state stored by those owners.

A domain owner keeps authority over its own object and applies a validated decision
through `ModerationSubjectCommandPort`. The moderation owner must never update another
module's tables directly and must not treat a decided case as an applied domain mutation.

## Neutral contract

Cross-domain subject and decision-application contracts are being extracted to
`rustok-moderation-api` so domain modules do not depend on the moderation persistence
owner.

The neutral API will own:

- subject and scope identity;
- reason and decision kinds;
- typed/versioned decision effects, including expiry and bounded capability restrictions;
- `ApplyModerationDecisionCommand` and `ModerationDecisionApplication`;
- `ModerationSubjectCommandPort`;
- the host-composed subject-adapter registry.

`rustok-moderation` may temporarily re-export moved contracts for Rust source
compatibility. New domain adapters must depend only on `rustok-moderation-api`.

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
             neutral typed subject adapter registry
                              |
                              v
                    authoritative owner
```

Cross-domain references are stored as immutable logical references:

- `subject_module`;
- `subject_kind`;
- `subject_id`;
- `subject_revision`;
- typed scope kind and optional scope ID.

There are no foreign keys from moderation tables to domain-owned tables.

## Groups compatibility

Groups is the reference membership-scoped integration:

- moderation owns the case and decision workflow;
- Groups owns membership/access enforcement, expiry evaluation, membership revision,
  domain receipts, and domain audit;
- membership decisions use subject kind `group_membership`, the membership UUID, and a
  monotonic membership revision;
- group-local scope carries the group UUID;
- Groups stores bounded decision provenance, not copied case notes or policy snapshots;
- the moderation admin queue belongs to moderation, while Groups UI may expose current
  local enforcement state and authorized domain actions.

## Current slice

The current owner slice provides:

- typed domain contracts, currently in the owner crate pending neutral API extraction;
- command/read FBA ports;
- tables for reports, cases, case-report links, decisions, receipts, and events;
- repository-backed report/case/decision services;
- module-owned migration source.

Neutral API extraction, host adapter registration, RBAC resources, durable decision
application, admin UI, automated assessment providers, appeals, and account sanctions
remain subsequent slices. See `docs/implementation-plan.md` for the canonical order and
evidence gates.
