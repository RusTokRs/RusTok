---
id: doc://docs/standards/patterns-vs-antipatterns.md
kind: project_overview
language: markdown
source_language: markdown
status: verified
---

# RusToK — Engineering Standards Navigation

The [logic hygiene catalog](logic-hygiene-patterns.md) is the canonical home
for observed defects, anti-patterns, remediation techniques, and Doctor
coverage. Its evidence inventory distinguishes verified RusToK cases from
review guidance and scanner candidates.

This page is a navigation index for existing links; the documents below own
their respective contracts. Do not add a parallel catalog of rules here.

## 1. Architecture

[Backend module architecture](../backend/module-backend-architecture.md) and
[platform principles](../architecture/principles.md).

## 2. Code Quality (Rust)

[Coding standards](coding.md), [error handling](errors.md), and the
[logic hygiene catalog](logic-hygiene-patterns.md).

## 3. Data and DB

[Safe database migrations](../guides/safe-database-migrations-and-squashing.md)
and [security standards](security.md).

## 4. Event System

[Event flow contract](../architecture/event-flow-contract.md).

## 5. Auth and RBAC

[RBAC module](../../crates/modules/rustok-rbac/README.md) and
[security standards](security.md).

## 6. Multi-Tenancy

[Tenant module documentation](../../crates/modules/rustok-tenant/docs/README.md).

## 7. API

[API architecture](../architecture/api.md).

## 8. Frontend

[Frontend module architecture](../UI/module-package-architecture.md).

## 9. Testing

[Testing guide](../guides/testing.md).

## 10. Observability

[Logging standards](logging.md).

## 11. Security

[Security standards](security.md).

## 12. DevOps and CI/CD

[Verification scripts](../../scripts/verify/README.md) and
[contributor contract](../../AGENTS.md).

## Related Documents

- [Documentation index](../index.md)
- [RusToK Doctor](../../crates/utils/rustok-doctor/README.md)
