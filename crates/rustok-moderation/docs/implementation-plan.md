---
id: doc://crates/rustok-moderation/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
---

# Moderation implementation plan

## Boundary

`rustok-moderation` owns reports, cases, policies, decisions, durable decision application, appeals, and moderation audit history. Domain modules remain authoritative for their own content and apply decisions through typed ports.

## Completed in the first slice

- owner crate and module metadata;
- typed subject and scope references;
- report, case, and decision contracts;
- owner-neutral command/read ports;
- subject-owner decision application port;
- module-owned migrations for reports, cases, report links, decisions, receipts, and events;
- ownership documentation.

## Next priorities

1. Add repository-backed report and case services with receipt-first idempotency.
2. Register the owner in the composed migration graph and platform module catalog.
3. Add moderation-specific RBAC resources and tenant permission registration.
4. Implement active-case deduplication and PostgreSQL contention evidence.
5. Add durable decision-application operations and recovery.
6. Integrate forum, blog, comments, pages/groups, reviews, marketplace listings, sellers, media, and messaging through owner adapters.
7. Add policy snapshots, premoderation, automated assessment providers, appeals, and capability-scoped account sanctions.
8. Publish admin queue and case surfaces only after the owner runtime is composed.

## Invariants

- no cross-domain foreign keys;
- every decision references the exact subject revision that was reviewed;
- the moderation owner never writes domain-owned tables;
- receipts replay before provider or subject reads;
- immutable decisions are not rewritten after application;
- domain owners validate whether a decision is applicable to their subject;
- automated providers return assessments, never direct destructive actions.

## Verification required before promotion

- SQLite schema and contract tests;
- PostgreSQL duplicate-report and active-case contention tests;
- receipt replay and request-hash conflict tests;
- case revision CAS tests;
- decision application crash/retry recovery;
- cross-tenant and local-scope isolation;
- owner adapter contract tests for each integrated module;
- composed migration-plan evidence.
