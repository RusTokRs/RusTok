# forum-20ad-group-key-population

Status: source-ready / unvalidated.

This owner note records the Forum/Notifications source boundary only; runtime execution and compiled verification remain maintainer-owned.

Tests, Cargo, formatting, and CI were not run by the implementation agent.

## Source contract

- # FORUM-20AD notification group-key population
- g1:{target_owner}:{target_id}
- BEFORE INSERT
- AFTER INSERT
- explicit non-`NULL` group keys are preserved
- tests/group_key_population_sqlite.rs
