# FORUM-20AM Forum and Notifications plan synchronization

Status: source-ready / unvalidated

## Scope

`FORUM-20AM` closes documentation drift accumulated while `FORUM-20H` through
`FORUM-20AL` were delivered as conflict-safe runtime slices. It updates the canonical Forum
plan, Notifications owner-local ledger, owner README, and live contract without changing
runtime code, migrations, dependencies, public envelopes, or task completion status.

## Recheck

The merged chain was re-read from its machine contracts and execution records:

- topic-local audience narrowing and richer exact visibility (`20H–20K`);
- exact recipient context, host authority, target-open and audience filtering (`20L–20P`);
- Groups membership facts (`20Q`);
- owner inbox open/list/state/count/reconciliation and bounded bulk/group operations
  (`20R–20AF`);
- authenticated storefront port, native adapter, grouped UI, navigation badge, GraphQL
  grouped reads, and GraphQL fresh-open authorization (`20AG–20AL`).

The latest `FORUM-20AL` handoff contract now names this synchronization contract instead of
claiming that the four documents remain pending.

## Preserved boundaries

The overall program remains `in_progress`. This task does not claim maintainer-run tests or
runtime evidence and does not close trust/channel facts, write audiences, remaining
search/index/SEO/deep-link migration, automatic auth-reactive bootstrap refresh, GraphQL
group-state writes, scheduled reconciliation/redaction, delivery transports, delivery-time
authorization, or PostgreSQL cross-consumer evidence.

## Evidence

- machine contract: `crates/rustok-forum/contracts/forum-notification-plan-sync.json`;
- source verifier: `scripts/verify/verify-forum-notification-plan-sync.mjs`;
- synchronized documents are named by the machine contract.

Suggested maintainer validation commands are recorded in the machine contract. None were run
by the implementation agent.
