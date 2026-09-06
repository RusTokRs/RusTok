# FORUM-23A10: Legacy ProfileService mutation deprecation

Date: 2026-07-30

Status: `source_complete_execution_pending`

## Purpose

`FORUM-23A9` introduced `ProfileMutationService` as the public Profiles mutation facade whose
constructor requires both `DatabaseConnection` and `TransactionalEventBus`. GraphQL self-service
and Profiles CLI backfill already use that facade.

The older mutation methods on `ProfileService` remained public for source compatibility. Their
names do not communicate that they omit durable `ProfileUpdated` publication, so an external caller
could select the unsafe API accidentally.

`FORUM-23A10` marks all seven legacy mutation methods with Rust `#[deprecated]` diagnostics and
names the exact event-aware replacement method.

The machine-readable contract is
`crates/rustok-forum/contracts/forum-search-profile-legacy-mutation-deprecation.json`.

## Deprecated methods

- `ProfileService::upsert_profile` → `ProfileMutationService::upsert_profile_with_event`;
- `ProfileService::update_profile_handle` → `ProfileMutationService::update_profile_handle_with_event`;
- `ProfileService::update_profile_content` → `ProfileMutationService::update_profile_content_with_event`;
- `ProfileService::update_profile_locale` → `ProfileMutationService::update_profile_locale_with_event`;
- `ProfileService::update_profile_visibility` → `ProfileMutationService::update_profile_visibility_with_event`;
- `ProfileService::update_profile_media` → `ProfileMutationService::update_profile_media_with_event`;
- `ProfileService::backfill_profile` → `ProfileMutationService::backfill_profile_with_event`.

Each diagnostic explains that the replacement couples the owner write to durable
`ProfileUpdated` publication atomically.

## Boundary composition

The compiler deprecation warning is the downstream API signal. Inside this repository, the stronger
`FORUM-23A8` source gate still rejects production Rust call sites for the legacy methods. The active
GraphQL and CLI paths remain bound to `ProfileMutationService` by the `FORUM-23A9` verifier.

The old `backfill_profile` implementation still calls old `upsert_profile` internally. A local
`allow(deprecated)` is attached to that compatibility method so the implementation can remain
source-compatible without hiding warnings from external callers.

Read, normalization, locale-candidate, and backfill-planning methods on `ProfileService` are not
deprecated.

## Compatibility

This slice does not remove or change any legacy method signature. It does not change method bodies,
storage behavior, GraphQL/REST schemas, Search query or document schemas, migrations, dependencies,
or `Cargo.lock`.

Existing callers can still compile unless their own build treats deprecation warnings as errors.
New code must use `ProfileMutationService`.

## Claim boundary

This slice does not claim that:

- legacy methods are compile-time private;
- legacy methods themselves publish durable events;
- external downstream crates cannot call them;
- the workspace denies all deprecation warnings;
- account deletion redaction or general producer ordering is complete;
- runtime verification was executed by the implementation agent.

The eventual breaking-window follow-up is to remove or make the deprecated mutation methods
crate-private after intentional legacy test callers have migrated.

## Remaining FORUM-23 scope

- remove or compile-time restrict deprecated `ProfileService` mutation methods;
- define owner-ordered profile or account deletion invalidation;
- replace remaining Forum wall-clock ordering with owner-issued monotonic revisions;
- add the remaining bounded filters and member projections;
- capture maintainer-executed PostgreSQL rebuild, redaction, and query evidence.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
node scripts/verify/verify-forum-search-profile-legacy-mutation-deprecation.mjs
node scripts/verify/verify-forum-search-profile-event-aware-mutation-api.mjs
node scripts/verify/verify-forum-search-profile-service-mutation-boundary.mjs
node scripts/verify/verify-forum-search-public-author-summary.mjs
cargo check -p rustok-profiles --all-targets
cargo check -p rustok-profiles-cli --all-targets
cargo xtask module validate profiles
cargo xtask module validate forum
```
