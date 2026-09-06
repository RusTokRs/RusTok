---
id: doc://crates/rustok-translation/docs/media-postgres-evidence-2026-09-02.md
kind: evidence_handoff
language: en
status: verified
verified_at: 2026-09-02
---

# Media Translation PostgreSQL evidence handoff

## Scope

This note reconciles the Media Translation target rollout evidence after PR
#3807. It records what is proven by repository CI and keeps the remaining
isolated deployment gate explicit. It does not broaden the Translation target
contract and it does not claim that GitHub-hosted PostgreSQL is a production
deployment database.

## Verified chain

- PR #3807 was based on exact `main` commit
  `fdc826f27dd5cd7d1d6889d3c0c7cff6bbf94934`.
- The final PR head was
  `52e4ae005f13e9bf303431dc5066a943096eacc8`.
- Focused workflow run `33623651814` completed successfully on that exact head.
  Exact-SHA checkout, Rust formatting, compilation, and the ignored PostgreSQL
  runtime evidence all passed.
- The PR changed only
  `.github/workflows/media-translation-target-postgres.yml` and
  `crates/rustok-media/tests/translation_target_postgres.rs`; production code
  was unchanged.
- PR #3807 was squash-merged as
  `47f700ea638ea5bd0978f05db654c725d5164576`.
- Post-merge workflow run `33635199181` completed successfully on exact
  `main@47f700ea638ea5bd0978f05db654c725d5164576`. Exact-SHA checkout,
  formatting, compilation, and PostgreSQL runtime execution all passed again.

## What the focused PostgreSQL evidence closes

The repository now has executable, exact-head PostgreSQL evidence for the Media
`media/asset` Translation target covering:

- two independent database-backed replicas using the shared owner contract;
- revision-safe exact-locale apply and stale/conflicting apply rejection;
- durable owner cursor recovery across independent connections;
- idempotent replay without duplicate owner effects;
- aggregate Translation progress convergence after owner changes; and
- the same evidence after squash merge on `main`.

This closes the missing repository CI proof for Media multi-replica apply,
checkpoint/cursor recovery, replay, and progress convergence. Future work must
reuse this test and workflow instead of creating a parallel Media PostgreSQL
evidence path.

## What remains open

The following gates are intentionally not closed by #3807:

1. **Isolated/live Media deployment evidence.** The same owner/provider path
   still needs execution in an approved deployment using deployment-owned
   database configuration and operational runtime composition. A PostgreSQL
   service container in GitHub Actions is not evidence of that deployment.
2. **Live external-provider machine-translation evidence.** The ignored
   durable structured-runtime probe remains an operator/deployment task because
   it can require provider credentials, egress approval, and billable external
   execution.
3. **Other production-database evidence tracks.** Translation Memory retention
   and AI accounting/recovery have their own focused repository evidence and
   must retain any deployment-database or live-provider gates separately. Media
   evidence must not be used to collapse those distinct readiness claims.

## Handoff rule

For subsequent Translation work, treat Media PostgreSQL multi-replica/cursor CI
as verified and move to the next repository-executable owner/onboarding gap from
the central plan. If the next requirement depends on deployment secrets,
external provider credentials, or an actual production database, keep it open
as a deployment gate rather than replacing it with another local or CI-only
simulation.
