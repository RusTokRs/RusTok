# Groups membership enforcement GraphQL contract

Status: **source-ready with consolidated SQLite/PostgreSQL parity sources / maintainer execution pending**

## Scope

`graphql_membership_enforcement::GroupsMembershipEnforcementMutation` adds exactly two direct Groups enforcement mutations to the existing final mutation chain:

- `suspendGroupMembership`;
- `revokeGroupMembershipSuspension`.

The existing `graphql_application_cas::GroupsMutationRoot` remains the stable module entrypoint. Its `MergedObject` includes `GroupsMembershipEnforcementMutation` additively, so application, invitation, governance, localization and core Groups mutations remain present. The module manifest query/mutation entrypoints do not change.

## Transport boundary

The GraphQL layer establishes only transport facts:

- authenticated `AuthContext` is required;
- authenticated tenant must equal `TenantContext`;
- actor is the authenticated user UUID;
- every effective permission is copied into `PortContext.claims`;
- host/request locale and optional channel are preserved;
- write deadline is five seconds;
- a non-empty caller idempotency key is required and forwarded unchanged;
- each request receives a fresh transport correlation ID.

The transport does **not** require a global `GROUPS_MODERATE` permission before calling the owner. Local owner/admin/moderator hierarchy remains a Groups owner-domain decision. Forwarding a platform claim never bypasses hard owner-target, self-target, revision, source or lifecycle checks.

## Owner-only business semantics

Both mutations construct `GroupMembershipEnforcementCommandService` from `HostRuntimeContext` and invoke only `GroupMembershipEnforcementCommandPort`.

GraphQL does not query/write Groups owner tables directly, infer or rewrite membership revisions, recompute hierarchy, synthesize replay results, alter lifecycle member-count semantics, revoke moderation-decision enforcement through a shortcut, or fall back to another service/transport.

The owner command remains authoritative for receipt-first replay, deterministic locking, effective actor authorization, owner/self protection, expected-revision CAS, suspension/revocation, group-version/member-count behavior, audit/events and receipt commit.

## Mutation inputs

`suspendGroupMembership` accepts `idempotencyKey`, `groupId`, `targetUserId`, `expectedMembershipRevision`, canonical `reasonCode` and optional `effectiveUntil`.

`revokeGroupMembershipSuspension` accepts the same identity/CAS inputs without `effectiveUntil`.

Input normalization and semantic validation remain owner responsibilities.

## Result

Both mutations return the owner result: group/membership/target IDs, post-mutation membership revision, post-mutation group version, unchanged stored-lifecycle `member_count`, effective status, enforcement revision, optional expiry/revocation timestamp and `replayed`.

The result exposes no Moderation case/decision/report data or internal receipt/audit rows.

## Errors

Neutral `PortError` kinds retain the common GraphQL transport classification:

- validation/conflict -> `BAD_USER_INPUT`;
- not found -> `NOT_FOUND`;
- forbidden -> `PERMISSION_DENIED`;
- unavailable/timeout -> sanitized `INTERNAL_ERROR`;
- invariant violation -> sanitized `INTERNAL_ERROR`.

The enforcement transport additionally preserves:

- `code`: platform-wide GraphQL transport classification;
- `domainCode`: exact stable Groups `PortError.code`;
- `retryable`: exact neutral retryability flag.

This keeps `groups.membership_enforcement_revision_conflict` machine-readable without replacing the common transport code. Source-level tests retain conflict/unavailable extension mapping.

## Consolidated parity contract

The canonical parity files are:

- `apps/server/tests/groups_membership_enforcement_graphql_sqlite.rs`;
- `apps/server/tests/groups_membership_enforcement_graphql_postgres.rs`.

Both use **two equivalent Groups**, not two sequential targets in one aggregate. Each group starts with one local owner, one ordinary lifecycle-active target, the same initial group version and the same stored member count. One group is mutated through native `GroupMembershipEnforcementCommandPort`; the other through the stable final GraphQL root.

This makes native/GraphQL `groupVersion`, membership revision, enforcement revision and member-count comparisons exact rather than merely monotonic across unrelated sequential mutations.

GraphQL carries the tenant-bound local owner with an empty effective permission list. No platform moderation claim is needed for the local owner path.

### Suspend and same-key replay

Both surfaces suspend their equivalent target at expected membership revision one. The packet compares group/membership/user identity relative to each aggregate, membership revision, group version, member count, effective status, enforcement revision, expiry/revocation presence and replay flag.

The exact request is repeated with the same idempotency key while the target is already suspended. Native and GraphQL must return the immutable stored suspension result with `replayed=true`, rather than fail current-state `already_suspended` validation.

### Fresh stale-CAS parity

A different idempotency key then retries expected membership revision one after the committed suspension advanced the target to revision two.

Native must return non-retryable `PortErrorKind::Conflict` with stable owner code `groups.membership_enforcement_revision_conflict`. GraphQL must retain the **same owner-safe message** and expose:

- `code=BAD_USER_INPUT`;
- `domainCode=groups.membership_enforcement_revision_conflict`;
- `retryable=false`.

The stale attempt must not mutate owner state.

### Revoke and same-key replay

Both surfaces revoke the direct-local suspension at expected membership revision two. The result must advance membership revision to three and group version exactly once while lifecycle member count remains unchanged. The bounded enforcement row remains and gains a revocation timestamp/enforcement revision update.

The exact revoke request is repeated with the same idempotency key after the suspension is no longer active. Both surfaces must return the stored revoke result with `replayed=true`.

### Historical suspension replay after revoke

After the later revoke commit, the original suspension idempotency key/request is replayed again. Both native and GraphQL must still return the historical **suspended** result with its earlier group version, original enforcement revision and `revoked_at=None`.

This is stronger than current-projection comparison: it proves completed receipt replay is resolved before current authorization/CAS/lifecycle validation and is never reconstructed from the later revoked projection.

### Final owner state

Backend reads occur only after production owner mutations for evidence. Both equivalent groups must finish with:

- target stored role `member`;
- lifecycle status `active`;
- membership revision three;
- lifecycle member count two;
- `source_kind=direct_local`;
- matching enforcement revision;
- non-null revocation marker;
- final group version equal to the successful revoke result.

No direct enforcement-table mutation stands in for owner behavior.

## SQLite source

`groups_membership_enforcement_graphql_sqlite.rs` uses a real temporary file-backed SQLite database and all production Groups migrations. It builds `graphql_application_cas::{GroupsQueryRoot, GroupsMutationRoot}` directly as the stable module schema surface.

The SQLite source is **execution pending** and does not populate `membership_enforcement_command_transport_parity` until maintainer execution.

## PostgreSQL source

`groups_membership_enforcement_graphql_postgres.rs` mirrors the same consolidated contract inside a unique PostgreSQL schema. Every SeaORM pool connection receives isolation through startup options:

```text
options=-csearch_path=<schema>,public
```

The packet intentionally never relies on session-local `SET search_path`. It is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and remains **execution pending**.

## No fallback

The stable module entrypoint remains `graphql_application_cas::GroupsMutationRoot`; the enforcement mutation is composed inside that root. There is no implicit GraphQL/native/admin/remote/legacy fallback.

## Verification

Intentionally not run while preparing this source:

```bash
cargo check -p rustok-groups --features graphql
cargo test -p rustok-groups --features graphql graphql_conflict_preserves_transport_and_owner_codes
cargo test -p rustok-groups --features graphql graphql_unavailable_keeps_owner_code_and_retryability
cargo test -p rustok-server --features mod-groups --test groups_membership_enforcement_graphql_sqlite -- --nocapture
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' cargo test -p rustok-server --features mod-groups --test groups_membership_enforcement_graphql_postgres -- --ignored --nocapture
node scripts/verify/verify-groups-membership-enforcement-graphql.mjs
```

No Cargo command, test, Node verifier, browser/schema execution, formatter, migration execution, workflow or CI job was executed. Executed SQLite/PostgreSQL replay/CAS/authorization/schema/error parity remains open.
