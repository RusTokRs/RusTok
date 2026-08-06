# `rustok-index` main delta — 2026-08-06 13:55 UTC

Active branch: `agent/index-m6-orphan-link-repair-20260806`.

Stack parent: `agent/index-m6-prepared-repair-recovery-20260806@7ababf39e26de9d2039c864d5092147d52d50d1a` / PR #3067.

Parent merge base with main: `main@6a5fb20d67ea20e726a4a782b79d591ec6a6ba72`.

Latest rechecked main: `main@809a557dd37d636ef1ad7fa1df06ab4c4daa03b4`.

## Intervening main delta

The complete parent-merge-base-to-main delta contains seven commits. Their changed paths are confined
to release workflows and Pages build composition, Commerce error-envelope and fulfillment-diagnostic
hardening, Forum category storefront routing, server evidence/test support, storefront feature
gating, `rustok-api` port error traits, and Forum admin recursion configuration.

No `crates/rustok-index` source, migration, documentation, contract, or Index verifier path changed.
There is no source overlap with either the prepared-repair recovery parent or this orphan-link repair
slice.

## Stacked PR boundary

This branch deliberately starts from PR #3067 because the orphan-link composition requires the
recovery-aware store, owner fence, and completion trigger introduced there. A PR from this branch to
`main` therefore includes the parent until #3067 lands. After #3067 is merged, the remaining diff is
the orphan-link repair slice only.

## Branch review scope

The new slice changes only `rustok-index` PostgreSQL repair composition, crate exports, architecture
documentation, the live implementation plan, and Index static verifier guards. It does not modify any
intervening main path.

## Validation boundary

This is a source-level concurrency and overlap recheck only. No tests, Node verifiers, formatting,
Cargo checks, migrations, PostgreSQL scenarios, workflows, or CI were executed by the implementation
agent.
