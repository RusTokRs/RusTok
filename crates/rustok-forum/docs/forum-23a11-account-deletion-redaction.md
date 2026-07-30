# FORUM-23A11: account deletion author redaction

Date: 2026-07-30

Status: `source_complete_execution_pending`

## Purpose

Forum Search embeds a bounded public Profiles summary into topic and approved-reply documents. The
canonical Auth admin `delete_user` operation previously deactivated the account and revoked sessions
without producing durable evidence that Profiles public presentation had already been hidden.

`DomainEvent::UserDeleted` already existed, but an event by itself was not sufficient: a Search
rebuild could still read a public Profiles row and reproduce the stale author summary.

A11 makes the owner state and invalidation evidence atomic.

## Canonical owner transaction

`ServerAuthAdminMutationProvider::delete_user` retains its authorization, tenant lock, role hierarchy,
and active-super-admin continuity checks. Inside the existing database transaction it now performs:

1. `AuthLifecycleService::deactivate_user_in_tx`;
2. `redact_profile_for_account_deactivation_in_tx`;
3. tenant-scoped active-session revocation;
4. durable RBAC invalidation-generation reservation;
5. `TransactionalEventBus::publish_in_tx` for `DomainEvent::UserDeleted`;
6. transaction commit.

The event envelope records the authenticated administrator as `actor_id`.

The Profiles helper marks an existing profile `Hidden` and updates its owner timestamp. If no profile
exists, it returns a successful `false` result: absence is already a valid redacted owner state, but
`UserDeleted` is still emitted so stale Search documents are rebuilt.

If event insertion fails, the transaction is explicitly rolled back. Auth deactivation, Profiles
hiding, session revocation, and RBAC generation reservation therefore cannot commit without durable
author-redaction evidence. The existing post-commit RBAC cache/fan-out path remains unchanged.

## Search redaction path

The Forum projection inbox maps both `ProfileUpdated` and `UserDeleted` to
`forum_author:<user_id>`. Author scope remains a redaction barrier and does not inherit an unrelated
full-Forum wall-clock watermark.

`SearchIngestionHandler` handles `UserDeleted` only when a Forum projection source is composed. The
consumer rebuilds the tenant's Forum projection from current owner state. Because the profile is now
hidden or absent, `ProfilePresentationService` denies anonymous presentation and Search replaces the
embedded author with `null`.

Projection errors retain the existing retry/backoff and dead-letter behavior.

## Scope boundary

This slice covers the canonical admin `delete_user` path. It does not claim equivalent Profiles
redaction for arbitrary `update_user(status = inactive|banned)` operations. Those paths require an
explicit product policy because temporary suspension and deletion may have different presentation
semantics.

The operation is deactivation, not hard erasure: account and profile rows remain for referential and
audit continuity. `UserDeleted` is sufficient only because owner redaction and event persistence
share one transaction; an ungrounded `UserDeleted` publisher would not satisfy this contract.

## Compatibility

No event variant, database migration, dependency, or `Cargo.lock` change is introduced. The Auth
admin port signature, Forum GraphQL/REST API, Search query API, Search document schema, and Profiles
public read API remain unchanged.

## Remaining scope

- define presentation behavior for non-delete user status disabling paths;
- replace remaining Forum wall-clock ordering with owner-issued monotonic revisions;
- remove or compile-time restrict deprecated `ProfileService` mutation methods;
- collect maintainer-executed PostgreSQL account-redaction and Search rebuild evidence.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
node scripts/verify/verify-forum-search-account-deletion-redaction.mjs
node scripts/verify/verify-forum-search-public-author-summary.mjs
cargo check -p rustok-profiles --all-targets
cargo check -p rustok-search --all-targets
cargo check -p rustok-server --all-targets
cargo test -p rustok-profiles account_redaction -- --nocapture
cargo test -p rustok-search forum_inbox -- --nocapture
cargo test -p rustok-search ingestion -- --nocapture
```
