# Implementation plan for `rustok-runtime`

## Execution checkpoint

- Current phase: foundation scaffold
- Last checkpoint: Created the runtime helper crate with typed shared-handle lookup and DB access helpers over the current `HostRuntimeContext`.
- Next step: migrate repeated backend adapter helper code from local modules into this crate only after each helper has at least two backend consumers.
- Open blockers: none
- Hand-off notes for next agent: Do not move domain services, Axum response mapping, CLI contracts, FBA metadata, or UI transport code into this crate.
- Last updated at (UTC): 2026-07-08T07:40:00Z

## Quality backlog

- Add focused tests when the first non-trivial runtime helper is introduced.
- Add source guardrails after the first production consumer migrates.
- Revisit `rustok-api::runtime` ownership after server bootstrap no longer depends on Loco.

