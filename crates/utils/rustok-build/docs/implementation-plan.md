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
[module release safety decision](../../../../DECISIONS/2026-08-06-module-release-rollback-safety.md)
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

## Execution and notification recovery

New build enqueue returns the inserted model rather than a pre-insert
placeholder, preserving reason and plan in immediate projections and newly
written composition receipts. Owner receipts created before this correction are
not rewritten; exact replay retains their original (possibly incomplete) result.
SQLite owner and server receipt regression tests require execution with the
project toolchain.

Successful-build reuse is not yet a complete composition identity contract:
the existing lookup selects an arbitrary matching hash before filtering status,
while reuse across revisions can return the earlier build's revision metadata.
The composition path also emits `BuildRequested` for reused or replayed results.
Resolve reuse, owner receipts, event delivery and client reconciliation together
rather than changing only the lookup or suppressing replay delivery.

Dry-run selection now previews without claiming or mutating a queued build; it
is a non-reserving snapshot. SQLite regression tests cover selection by ID,
selection of the next build, repeated previews and invalid plans, but require
execution in a project-compatible Rust environment.

A live executor now attempts to mark a claimed build failed for observed plan,
materialization, process and other execution errors. Claim publication errors
also attempt failure recording; a committed terminal state is not overwritten.
Status writes use an expected-status predicate and report final, missing, invalid
or concurrently changed states instead of treating them as success. Tests cover
these in-process cases and post-commit event failures but require execution with
the project toolchain. An ignored PostgreSQL test now holds the build row while
both terminal writers read `running` and block on their updates; it checks that
only one transition and event wins. It has not been run here and does not cover
crash recovery, global admission, or same-status progress ordering.

Actual execution still requires durable fencing, global admission and recovery
for a lost or cancelled worker, including a process that outlives its task. Same-
status progress updates are not version-fenced. Publication after a commit can
fail, repeat or arrive out of order; transactional notification/outbox design and
subscriber reconciliation are still needed. Database or failure-publication
errors can make the result ambiguous and can leave a job stranded; no automatic
retry or rollback is implied by this change.

## Verification

- `cargo check -p rustok-build`
- `cargo test -p rustok-build --lib`
- `RUSTOK_BUILD_TEST_POSTGRES_URL=<disposable database URL> cargo test -p rustok-build --test postgres_status_contention -- --ignored --nocapture` (requires permission to create/drop an isolated schema; not yet run)
- `cargo check -p rustok-server --lib --no-default-features`
- `cargo check -p rustok-admin --lib --no-default-features --features ssr`
- API surface contract guard after server call sites move.
