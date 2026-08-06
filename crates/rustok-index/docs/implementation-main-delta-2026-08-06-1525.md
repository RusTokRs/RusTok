# `rustok-index` main delta — 2026-08-06 15:25 UTC

Active branch: `agent/index-m6-repair-execution-evidence-20260806`.

Stack parent: `agent/index-m6-orphan-link-repair-20260806@8c36960608ef387b5e36f2b5904c6ea83ceda752` / PR #3075.

Parent PR dependency: `agent/index-m6-prepared-repair-recovery-20260806@7ababf39e26de9d2039c864d5092147d52d50d1a` / PR #3067.

Latest rechecked main: `main@bfc517186f2a0f56ffea4887ead2b69779c210a4`.

## Intervening main delta

The complete `main@b564f764fc28b12da55b2a0db0d8b4e94c97b0d4..main@bfc517186f2a0f56ffea4887ead2b69779c210a4`
delta contains six commits. Changed paths are confined to Commerce and Customer diagnostic safety,
Forum/Search canonical route cutover and its source-ready PostgreSQL reindex harness, and Page Builder
inline-session DOM hardening.

No `crates/rustok-index` source, migration, documentation, integration-test, or Index verifier path
changed. There is no path overlap with the prepared-repair recovery slice, orphan-link repair slice,
or this repair execution harness.

## Stacked PR boundary

This branch deliberately starts from PR #3075 because the executable scenarios exercise the concrete
orphan-link owner introduced there and the recovery-aware boundary from #3067. A pull request from
this branch to `main` therefore includes both parent slices until they land. After #3067 and #3075
merge, the remaining diff is the repair execution evidence packet only.

## Review scope

The new slice adds only:

- two env-gated `rustok-index` PostgreSQL integration targets;
- their shared isolated-schema fixture;
- one architecture/runbook document;
- live plan and documentation index updates;
- one static source guard and aggregate registration.

No production runtime, public transport, migration, domain, application, or PostgreSQL adapter source
is modified.

## Validation boundary

This is a source-level overlap and concurrency review only. Tests, Node verifiers, formatting, Cargo
checks, migrations, PostgreSQL scenarios, workflows, and CI were not executed by the implementation
agent.
