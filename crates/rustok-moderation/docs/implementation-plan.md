---
id: doc://crates/rustok-moderation/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
---

# Moderation implementation plan

## Boundary

`rustok-moderation` owns reports, cases, policies, decisions, durable decision application, appeals, and moderation audit history. Domain modules remain authoritative for their own content and apply decisions through typed ports.

## Completed

- owner crate and module metadata;
- typed subject and scope references;
- owner-neutral command/read ports and subject-owner decision application port;
- module-owned schema for reports, cases, report links, immutable decisions, receipts, and events;
- workspace and composed migration registration with locked build and migration-plan evidence;
- repository-backed report, case, assignment, and decision services;
- receipt-first replay and deterministic request hashes derived from `PortContext`;
- active-case identity using `ON CONFLICT DO NOTHING` rather than recover-after-unique-error transactions;
- revision compare-and-set for mutable case transitions;
- tenant-scoped read and queue projections;
- SQLite contract coverage for replay, conflicts, deduplication, revisions, and isolation.

## Next priorities

1. Add PostgreSQL concurrent active-case and revision-CAS evidence.
2. Add moderation-specific RBAC resources and tenant permission registration.
3. Publish transactional outbox contracts for report, case, and decision lifecycle events.
4. Add durable decision-application operations and recovery.
5. Integrate forum, blog, comments, pages/groups, reviews, marketplace listings, sellers, media, and messaging through owner adapters.
6. Add versioned policies, premoderation, automated assessment providers, appeals, and capability-scoped account sanctions.
7. Publish admin queue and case surfaces only after the owner runtime is composed.

## Invariants

- no cross-domain foreign keys;
- every decision references the exact subject revision that was reviewed;
- the moderation owner never writes domain-owned tables;
- receipts replay before provider or subject reads;
- idempotency keys and actor identity are accepted only through `PortContext`;
- active-case deduplication never relies on continuing a PostgreSQL transaction after a unique violation;
- immutable decisions are not rewritten after application;
- domain owners validate whether a decision is applicable to their subject;
- automated providers return assessments, never direct destructive actions.

## Verification required before promotion

- PostgreSQL duplicate-report and active-case contention tests;
- concurrent case revision CAS tests;
- decision application crash/retry recovery;
- cross-tenant and local-scope authorization evidence;
- owner adapter contract tests for each integrated module;
- composed runtime, RBAC, outbox, and transport evidence.
