# rustok-moderation

`rustok-moderation` is the cross-domain owner for moderation reports, cases, decisions, receipts, and auditable enforcement orchestration.

## Ownership

The module owns:

- user and system reports;
- moderation cases and report deduplication;
- queue assignment and case revision;
- immutable moderation decisions;
- idempotency receipts;
- moderation events and audit history.

The module does not own forum topics, blog posts, comments, groups, reviews, products, marketplace listings, seller profiles, messages, media assets, or user profiles.

A domain owner keeps authority over its own object and implements `ModerationSubjectCommandPort` to apply a validated decision. The moderation owner must never update another module's tables directly.

## Boundary

```text
forum/blog/comments/groups/reviews/marketplace/media/messages
                              |
                              v
                    rustok-moderation
               reports -> cases -> decisions
                              |
                              v
                 typed domain command port
                              |
                              v
                    authoritative owner
```

Cross-domain references are stored as immutable logical references:

- `subject_module`;
- `subject_kind`;
- `subject_id`;
- `subject_revision`.

There are no foreign keys from moderation tables to domain-owned tables.

## Current slice

The first owner slice provides:

- typed domain contracts;
- command/read FBA ports;
- tables for reports, cases, case-report links, decisions, receipts, and events;
- module-owned migration source.

Host registration, RBAC resources, concrete persistence services, admin UI, automated assessment providers, appeals, and account sanctions are intentionally deferred to subsequent slices.
