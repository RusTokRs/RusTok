# Implementation plan for `rustok-cli-core`

## Execution checkpoint

- Current phase: foundation scaffold
- Last checkpoint: Created the CLI core contract crate with command descriptors, requests, outcomes, provider trait, and shared error type.
- Next step: introduce the user-facing `rustok-cli` binary crate only when the first Loco task/seed/migration flow is moved out of `apps/server`.
- Open blockers: none
- Hand-off notes for next agent: Do not add module-specific commands to this crate; module commands belong in module-local `cli/` adapter packages and are aggregated through a generated registry.
- Last updated at (UTC): 2026-07-08T07:40:00Z

## Quality backlog

- Add parser/binary integration tests when the `rustok-cli` binary appears.
- Add generated registry contract tests before distribution-aware builds.
- Update Loco task migration docs as each command moves from `cargo loco task` to the platform CLI.

