# Implementation plan for `rustok-runtime`

## Execution checkpoint

- Current phase: host-neutral composition
- Last checkpoint: Added `RuntimeComposition`, which carries optional DB/typed host handles and a JSON settings snapshot without depending on `apps/server` configuration types.
- Next step: pass host-created `RuntimeComposition` into the first DB-backed module CLI provider.
- Open blockers: none
- Hand-off notes for next agent: Do not move domain services, Axum response mapping, CLI contracts, FBA metadata, or UI transport code into this crate.
- Last updated at (UTC): 2026-07-10T00:00:00Z

## Quality backlog

- Add focused tests when the first non-trivial runtime helper is introduced.
- Add source guardrails after the first production consumer migrates.
- Revisit `rustok-api::runtime` ownership after server bootstrap no longer depends on Loco.
