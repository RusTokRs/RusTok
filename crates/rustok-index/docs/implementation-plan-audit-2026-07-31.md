# `rustok-index` implementation status audit — 2026-07-31

This audit accompanies the canonical [implementation plan](./implementation-plan.md) and prevents source-complete draft work from being mistaken for code already merged to `main`.

## Status vocabulary

- `merged_main`: present on the current default branch.
- `open_source_complete`: implemented in an open PR, with maintainer validation still pending.
- `owner_evidence_pending`: source exists, but retained runtime/PostgreSQL/CI evidence has not been executed by the implementation agent.
- `not_started`: no reviewed implementation was found.

## Rechecked M6/M7 state

| Capability | Status | Evidence |
| --- | --- | --- |
| bounded multi-pass reconciliation runner, durable cursor/pass progress, lease fencing and cancellation | `merged_main`; `owner_evidence_pending` | production runner exists on `main`; focused PostgreSQL harness PRs remain open |
| failed reconciliation scope admission | `open_source_complete`; `owner_evidence_pending` | superseded PR #2743, retained in this PR |
| bounded dead-letter inspection core adapter | `open_source_complete`; `owner_evidence_pending` | added in this PR |
| source-call timeout | `open_source_complete`; `owner_evidence_pending` | PR #2639 |
| bounded replay dry-run | `open_source_complete`; `owner_evidence_pending` | PR #2642 |
| replay retry transition store | `open_source_complete`; `owner_evidence_pending` | PR #2644 |
| replay dead-letter admission | `open_source_complete`; `owner_evidence_pending` | PR #2648 |
| cooperative replay page interruption | `open_source_complete`; `owner_evidence_pending` | PR #2649 |
| guarded reconciliation operator | `open_source_complete`; `owner_evidence_pending` | PR #2693 |
| Product/ProductVariant/SalesChannel bounded schemas and sources | `open_source_complete`; `owner_evidence_pending` | canonical M7 work retained from PR #2636 and related open slices |
| incremental event acknowledgement | `not_started` | no reviewed implementation found |
| durable Product/ProductVariant-to-SalesChannel relation revision contract | `not_started` | prior attempts correctly leave this open because channel changes can invalidate links without advancing Product revision |
| automatic retry/backoff/exhaustion plus host scheduling | `not_started` as an integrated capability | storage substrate exists in open PRs, but runner wiring and host ownership remain open |
| actor/reason audit and manual dead-letter requeue/reset | `not_started` | next production sequence after authorized inspection |
| digest comparison, orphan cleanup, targeted/full/shadow repair and complete drift admission | `not_started` | remains the final M6 consistency boundary |

## Updated execution order

1. Merge or supersede the source-complete M7 and reconciliation admission work without losing the canonical plan.
2. Add the bounded core dead-letter inspector (this PR).
3. Compose it behind the server-owned request-bound `modules:manage` reconciliation operator.
4. Add immutable actor/reason audit records and a scope-locked manual requeue or retry-epoch reset.
5. Wire bounded retry/backoff/exhaustion and explicit host scheduling/graceful shutdown ownership.
6. Add digest comparison, orphan detection, targeted/full/shadow repair, locale/partition dimensions, and retained admission evidence.
7. Continue M7 with incremental acknowledgement, durable relation revision semantics, slice completeness, and consumer cutover.

## Validation ownership

Per maintainer instruction, the implementation agent did not run tests, formatting, Cargo checks, JavaScript verifiers, PostgreSQL fixtures, or CI. All source-complete claims in this audit mean code review/repository inspection only, not executed evidence.
