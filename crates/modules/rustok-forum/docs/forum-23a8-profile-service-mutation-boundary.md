# FORUM-23A8: ProfileService mutation source boundary

Date: 2026-07-30

Status: `source_complete_execution_pending`

## Purpose

`FORUM-23A1` through `FORUM-23A7` made the active GraphQL self-service and Profiles CLI
backfill paths couple owner writes to durable `ProfileUpdated` publication. The remaining risk was
that repository production code could call the older direct mutation methods on `ProfileService`
and bypass those transactional helpers.

This slice adds a source-level production call-site gate for that risk. It does not change the
public Rust signatures and does not make the methods compile-time private.

The machine-readable contract is
`crates/rustok-forum/contracts/forum-search-profile-service-mutation-boundary.json`.

## Audited direct methods

The audit covers:

- `upsert_profile`;
- `update_profile_handle`;
- `update_profile_content`;
- `update_profile_locale`;
- `update_profile_visibility`;
- `update_profile_media`;
- `backfill_profile`.

At main commit `31df77f3cd7b6294af04af81bf73eadb1a0c9e72`, repository search found definitions in
`crates/rustok-profiles/src/services.rs` and non-definition call sites only in tests or verifier
text. No production Rust caller was found.

## Enforced boundary

`scripts/verify/verify-forum-search-profile-service-mutation-boundary.mjs` recursively scans
production `.rs` files under `apps/` and `crates/`. It rejects method-call syntax and
`ProfileService::method` UFCS syntax for every audited direct method.

The only production exemption is the owner definition file itself. Test, fixture, example, and
benchmark paths remain available for compatibility with existing service-level tests.

The verifier separately requires the active runtime paths to use the event-coupled helpers:

- GraphQL self-service uses `upsert_profile_with_event` and the five focused update helpers;
- CLI backfill uses `backfill_profile_with_event`.

A future production call to a direct non-event method therefore fails source verification rather
than silently bypassing Forum Search invalidation.

## Claim boundary

This is deliberately a source-level production call-site gate. It does not claim that:

- the public methods are compile-time private;
- external downstream repositories are scanned;
- an external crate cannot call the methods;
- runtime verification was executed by the implementation agent;
- account deletion redaction or general producer ordering is complete.

The stronger follow-up is to replace or restrict the direct APIs so event publication is intrinsic
to the public mutation surface rather than enforced only by repository source policy.

## Compatibility

No public Rust method signature, GraphQL/REST contract, Search query or document schema, database
migration, dependency, or `Cargo.lock` change is introduced.

## Remaining FORUM-23 scope

- replace this source gate with compile-time restricted or intrinsically event-aware mutation APIs;
- define owner-ordered profile or account deletion invalidation;
- replace remaining Forum wall-clock ordering with owner-issued monotonic revisions;
- add the remaining bounded filters and member projections;
- capture maintainer-executed PostgreSQL rebuild, redaction, and query evidence.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
node scripts/verify/verify-forum-search-profile-service-mutation-boundary.mjs
node scripts/verify/verify-forum-search-public-author-summary.mjs
cargo check -p rustok-profiles --all-targets
cargo check -p rustok-profiles-cli --all-targets
cargo xtask module validate profiles
cargo xtask module validate forum
```
