# leptos-auth — implementation plan

_No planned tasks._

## Execution checkpoint

- Current phase: transport_module_alignment
- Last checkpoint: Legacy `src/api.rs` file removed; auth native/server-function + GraphQL fallback implementation now lives in `src/transport.rs`, while `leptos_auth::api::*` remains as a compatibility re-export for existing callers.
- Next step: Synchronize the plan with the current code and select the first incomplete item.
- Open blockers: None.
- Hand-off notes for next agent: Update this block after each increment.
- Last updated at (UTC): 2026-06-29T00:00:00Z



## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and relevance of `README.md` and local docs.
- [ ] Lock/update verification gates for the current module state.
