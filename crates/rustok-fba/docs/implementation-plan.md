# Implementation plan for `rustok-fba`

## Execution checkpoint

- Current phase: foundation scaffold
- Last checkpoint: Created shared FBA metadata types for backend topology, transport profiles, provider descriptors, consumer dependencies, and call context over `rustok-api::ports`.
- Next step: migrate repeated provider/consumer registry structs only after two or more module registries need the same typed shape.
- Open blockers: none
- Hand-off notes for next agent: Do not move module service traits, gRPC adapters, HTTP handlers, or event transport code into this crate.
- Last updated at (UTC): 2026-07-08T07:40:00Z

## Quality backlog

- Add JSON fixture tests when the first module registry consumes these types.
- Add verifier coverage that new FBA registries use `rustok-fba` for shared metadata.
- Keep `rustok-api::ports` as the canonical call-context/error layer until a separate ports crate is justified.
