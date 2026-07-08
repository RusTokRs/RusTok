# Implementation plan for `rustok-cli-core`

## Execution checkpoint

- Current phase: typed provider execution
- Last checkpoint: Created the CLI core contract crate with command descriptors, requests, outcomes, provider trait, shared error type and default provider execution contract; introduced the separate `rustok-cli` binary crate with list/help behavior, namespace dispatch and an explicit duplicate-checking command registry outside the server dependency graph.
- Next step: connect the first generated selected-distribution provider or move the first legacy task/seed/migration flow out of `apps/server` into a typed `rustok-cli` command.
- Open blockers: none
- Hand-off notes for next agent: Do not add module-specific commands to this crate; module commands belong in module-local `cli/` adapter packages and are aggregated through a generated registry.
- Last updated at (UTC): 2026-07-08T07:40:00Z

## Quality backlog

- Extend command argument decoding tests as the first external command provider is wired.
- Add generated registry contract tests before distribution-aware builds.
- Update task migration docs as each command moves to the platform CLI.
