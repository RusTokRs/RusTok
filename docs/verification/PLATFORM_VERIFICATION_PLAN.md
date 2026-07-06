---
id: doc://docs/verification/PLATFORM_VERIFICATION_PLAN.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# RusToK — Main Platform Verification Plan

- **Structure update date:** 2026-04-08
- **Status:** Ready for new periodic run
- **Mode:** Master-plan for repeatable verification sessions
- **Goal:** Run regular platform verification by consolidated phases without accumulating historical noise in a single document

---

## How the Verification Plan Set is Now Organized

The main document no longer stores the entire detailed checklist and history of fixes in one file.
It is used as an orchestration layer for periodic runs, while detailed checks are extracted into specialized documents inside `docs/verification/`.

### Master / orchestration

- [Main Platform Verification Plan](./PLATFORM_VERIFICATION_PLAN.md) — this file, reset-friendly master-checklist for a new run.

### Detailed platform plans

- [Foundation Verification Plan](./platform-foundation-verification-plan.md) — workspace baseline, module composition, foundation crates, auth/RBAC/tenant foundation.
- [Events, Domains and Integrations Verification Plan](./platform-domain-events-integrations-verification-plan.md) — event runtime, domain modules, integration boundaries.
- [API Surfaces Verification Plan](./platform-api-surfaces-verification-plan.md) — GraphQL, REST, `#[server]`, operational endpoints.
- [Frontend Surfaces Verification Plan](./platform-frontend-surfaces-verification-plan.md) — host apps, module-owned UI, shared libraries, i18n/routes.
- [Quality and Operational Readiness Verification Plan](./platform-quality-operations-verification-plan.md) — local quality checks, observability, security/dependency hygiene, documentation sync and release-readiness.

### Specialized companion plans

- [RBAC, Server and Runtime Module Verification Plan](./rbac-server-modules-verification-plan.md) — targeted pass on live authorization contract and capability boundaries.
- [Leptos Libraries Verification Plan](./leptos-libraries-verification-plan.md) — companion plan for shared Leptos/UI library layer.
- [Platform Core Integrity Verification Plan](./platform-core-integrity-verification-plan.md) — server + admin surfaces + core crates as a unified runtime baseline.

---

## Periodic Run Rules

- This master-plan only stores the clean checklist of the current/next run.
- Historical `[x]`, `[!]` and detailed fix descriptions are not accumulated here.
- Check details are maintained in specialized plans, and issue history is in a separate registry.
- If a new problem is found during a run, it should be reflected directly in the relevant detailed plan and closed within the same verification cycle.
- After changing architecture, API, modules, UI contracts, observability or the verification process, synchronize [docs/index.md](../index.md) and the [verification catalog README](./README.md).

## Order of Execution

1. First go through the foundation block.
2. Then check events, domain modules and integrations.
3. After that, check API and frontend surfaces.
4. Complete the run with the quality/operations/release-readiness block.
5. Separately verify targeted companion plans for RBAC and Leptos libraries if the relevant circuits are affected.

---

## Master-Checklist for New Run

### Phase 0. Compilation and Build

- [ ] Pass the build baseline from the [Foundation Verification Plan](./platform-foundation-verification-plan.md).
- [ ] Record environment blockers separately from product defects.

### Phase 1. Architectural Compliance

- [ ] Verify registry, taxonomy and dependency graph via the [Foundation Verification Plan](./platform-foundation-verification-plan.md).

### Phase 2. Platform Core

- [ ] Check `rustok-core`, `rustok-outbox`, `rustok-events`, `rustok-telemetry` against the [Foundation Verification Plan](./platform-foundation-verification-plan.md).

### Phase 3. Authorization and Authentication

- [ ] Pass the auth surface from the [Foundation Verification Plan](./platform-foundation-verification-plan.md).

### Phase 4. RBAC

- [ ] Execute platform-level RBAC checks from the [Foundation Verification Plan](./platform-foundation-verification-plan.md).
- [ ] For server/runtime module changes, additionally go through the [RBAC, Server and Runtime Module Verification Plan](./rbac-server-modules-verification-plan.md).

### Phase 5. Multi-Tenancy

- [ ] Pass tenancy checks from the [Foundation Verification Plan](./platform-foundation-verification-plan.md).

### Phase 6. Event System

- [ ] Pass event/outbox checks from the [Events, Domains and Integrations Verification Plan](./platform-domain-events-integrations-verification-plan.md).

### Phase 7. Domain Modules

- [ ] Pass module checks from the [Events, Domains and Integrations Verification Plan](./platform-domain-events-integrations-verification-plan.md).

### Phase 8. GraphQL API

- [ ] Pass GraphQL contract checks from the [API Surfaces Verification Plan](./platform-api-surfaces-verification-plan.md).

### Phase 9. REST API

- [ ] Pass REST contract checks from the [API Surfaces Verification Plan](./platform-api-surfaces-verification-plan.md).

### Phase 10. Leptos Frontends

- [ ] Pass Leptos app checks from the [Frontend Surfaces Verification Plan](./platform-frontend-surfaces-verification-plan.md).

### Phase 11. Next.js Frontends

- [ ] Pass Next.js app checks from the [Frontend Surfaces Verification Plan](./platform-frontend-surfaces-verification-plan.md).

### Phase 12. Frontend Libraries

- [ ] Pass platform-level library/package checks from the [Frontend Surfaces Verification Plan](./platform-frontend-surfaces-verification-plan.md).
- [ ] For targeted library contract checks, use the [Leptos Libraries Verification Plan](./leptos-libraries-verification-plan.md).

### Phase 13. Integration Links

- [ ] Pass E2E integration checks from the [Events, Domains and Integrations Verification Plan](./platform-domain-events-integrations-verification-plan.md).

### Phase 14. Local Quality Baseline

- [ ] Pass local quality checks from the [Quality and Operational Readiness Verification Plan](./platform-quality-operations-verification-plan.md).
- [ ] For `page_builder/pages` changes, additionally run `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs` and attach the report to release evidence.

### Phase 15. Observability and Operational Readiness

- [ ] Pass observability/ops checks from the [Quality and Operational Readiness Verification Plan](./platform-quality-operations-verification-plan.md).

### Phase 16. Documentation Sync and Release-Readiness

- [ ] Pass documentation sync and release-readiness checks from the [Quality and Operational Readiness Verification Plan](./platform-quality-operations-verification-plan.md).

### Phase 17. Security and Dependency Hygiene

- [ ] Pass security/dependency hygiene checks from the [Quality and Operational Readiness Verification Plan](./platform-quality-operations-verification-plan.md).

### Phase 18. Quality Anti-Patterns and Correctness

- [ ] Pass remaining quality/correctness checks from the [Quality and Operational Readiness Verification Plan](./platform-quality-operations-verification-plan.md).

---

## Final Run Report

Fill in upon completion of the current verification cycle:

| Block | Status | Comment |
|-------|--------|---------|
| Foundation | ⬜ | |
| Events / Domains / Integrations | ⬜ | |
| API Surfaces | ⬜ | |
| Frontend Surfaces | ⬜ | |
| Quality / Operations / Release Readiness | ⬜ | |
| Targeted RBAC/server companion plan | ⬜ | |
| Targeted Leptos libraries companion plan | ⬜ | |
| **TOTAL** | ⬜ | |

---

## Related Documents

- [Verification catalog README](./README.md)
- [Documentation Map](../index.md)
- [Verification scripts README](../../scripts/verify/README.md)
- [Patterns vs Antipatterns](../standards/patterns-vs-antipatterns.md)
- [Forbidden Actions](../standards/forbidden-actions.md)
- [Known Pitfalls](../ai/KNOWN_PITFALLS.md)
