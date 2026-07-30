# FORUM-23A9: event-aware Profiles mutation API

Date: 2026-07-30

Status: `source_complete_execution_pending`

## Purpose

`FORUM-23A1` through `FORUM-23A7` made the active GraphQL self-service and CLI backfill writes
atomically publish `ProfileUpdated`. `FORUM-23A8` then added a repository source gate preventing
production code from calling the older direct `ProfileService` mutation methods.

This slice adds the stronger positive API boundary: a public `ProfileMutationService` whose
constructor requires both a `DatabaseConnection` and a `TransactionalEventBus`. Every exposed
mutation is explicitly event-named and delegates to the existing Profiles-owned atomic helper.

The machine-readable contract is
`crates/rustok-forum/contracts/forum-search-profile-event-aware-mutation-api.json`.

## Public mutation facade

`crates/rustok-profiles/src/mutations.rs` exports:

- `upsert_profile_with_event`;
- `update_profile_handle_with_event`;
- `update_profile_content_with_event`;
- `update_profile_locale_with_event`;
- `update_profile_visibility_with_event`;
- `update_profile_media_with_event`;
- `backfill_profile_with_event`.

The facade stores borrowed references to the database and durable event bus. Callers therefore
cannot construct the preferred mutation surface without providing the outbox-capable runtime
component. The facade does not duplicate owner logic: every method delegates to the already
reviewed transaction that commits the profile owner rows only after the `ProfileUpdated` envelope
has been persisted.

## Runtime adoption

GraphQL self-service now constructs `ProfileMutationService` from the request-scoped database and
event bus. It still passes the authenticated human user as actor and target user. Avatar/banner
ownership and access validation remains before the facade write, preserving the existing media
boundary.

Profiles CLI backfill constructs one facade for the command and uses its
`backfill_profile_with_event` method. The underlying helper continues to publish with a system actor,
recheck profile absence inside the transaction, and skip a concurrently created profile rather
than overwriting it.

The older public free `backfill_profile_with_event` helper remains exported as a compatibility
surface. It is already event-aware and cannot perform a silent owner write.

## Legacy boundary

The older direct `ProfileService` mutation methods remain public for compatibility and tests. This
slice does not claim that external downstream crates are compile-time prevented from invoking
them. Repository production code remains protected by the `FORUM-23A8` source gate.

The follow-up is to deprecate, remove, or compile-time restrict those legacy methods after callers
have migrated to the facade. Until then, the claim is narrower: RusToK production GraphQL and CLI
mutation entry points use the event-aware public API, while legacy bypass methods remain explicit
debt rather than hidden risk.

## Compatibility

No legacy `ProfileService` signature, GraphQL schema, Forum REST contract, Search query or document
schema, database migration, dependency, or `Cargo.lock` change is introduced.

## Remaining FORUM-23 scope

- deprecate, remove, or compile-time restrict legacy direct `ProfileService` mutations;
- define owner-ordered profile or account deletion invalidation;
- replace remaining Forum wall-clock ordering with owner-issued monotonic revisions;
- add the remaining bounded filters and member projections;
- capture maintainer-executed PostgreSQL rebuild, redaction, and query evidence.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
node scripts/verify/verify-forum-search-profile-event-aware-mutation-api.mjs
node scripts/verify/verify-forum-search-profile-service-mutation-boundary.mjs
node scripts/verify/verify-forum-search-public-author-summary.mjs
cargo check -p rustok-profiles --all-targets
cargo check -p rustok-profiles-cli --all-targets
cargo xtask module validate profiles
cargo xtask module validate forum
```
