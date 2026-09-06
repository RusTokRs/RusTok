# Implementation plan for `rustok-build`

## Current state

The capability owns build persistence, immutable execution-plan contracts,
queued execution, command construction, manifest-snapshot materialization, and
the process runner. `BuildControl` is read-only and returns canonical
framework-neutral build snapshots. The production release table, build-to-release
field, active-release head, deployment backends, activation hook, rollback
command/event, GraphQL/native mutations, and both admin rollback controls have
been removed atomically.

## Module Release Safety Boundary

The accepted
[module release safety decision](../../../DECISIONS/2026-08-06-module-release-rollback-safety.md)
makes `rustok-modules` the sole operator-level owner of static release
selection, predecessor eligibility, rollback, incident outcome, and
desired-versus-observed rollout. `rustok-build` remains the plan/validation
owner for canonical role builds and shared non-operator build primitives.
`rustok-static-distribution-worker` is the sole static role-bundle
executor/publisher and returns one canonical digest-bound role-bundle receipt
covering every automated server/worker role and embedded Leptos/browser
artifact. `rustok-build` must not retain a second static publisher.

Production admission, activation, rollout, recovery, and retention are outside
this crate and owned by `rustok-modules`. Future build changes must not
reintroduce a release head, direct deployment, or rollback fallback here.

## Verification

- `cargo check -p rustok-build`
- `cargo check -p rustok-server --lib --no-default-features`
- `cargo check -p rustok-admin --lib --no-default-features --features ssr`
- API surface contract guard after server call sites move.
