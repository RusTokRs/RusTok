# Implementation plan for `rustok-web`

## Execution checkpoint

- Current phase: foundation scaffold
- Last checkpoint: Created the Axum HTTP boundary crate with shared `HttpError`, `HttpResult`, and JSON error body mapping.
- Next step: migrate repeated Loco controller replacement helpers from server/module controllers during Phase 2 of the Loco exit plan.
- Open blockers: none
- Hand-off notes for next agent: Do not move domain services or module-specific error semantics into this crate; it is only a web boundary mapping layer.
- Last updated at (UTC): 2026-07-08T07:40:00Z

## Quality backlog

- Add mapper helpers for `rustok-api::PortError` after the first controller cutover needs it.
- Add source guardrails once controller imports start using `rustok-web`.
- Add focused tests for status/body mapping before broad controller migration.
