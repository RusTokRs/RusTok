# Groups membership enforcement GraphQL contract

Status: **source-ready with executable SQLite/PostgreSQL parity sources / maintainer execution pending**

## Scope

`graphql_membership_enforcement::GroupsMembershipEnforcementMutation` adds exactly two direct Groups enforcement mutations to the existing final mutation chain:

- `suspendGroupMembership`;
- `revokeGroupMembershipSuspension`.

The existing `graphql_application_cas::GroupsMutationRoot` remains the stable module entrypoint. Its `MergedObject` now includes `GroupsMembershipEnforcementMutation` as an additive member, so application, invitation, governance, localization and core Groups mutations remain present. The module manifest query and mutation entrypoints do not change.

## Transport boundary

The GraphQL layer establishes only transport facts:

- authenticated `AuthContext` is required;
- authenticated tenant must equal `TenantContext`;
- actor is the authenticated user UUID;
- every effective permission is copied into `PortContext.claims`;
- the host/request locale and optional channel are preserved;
- write deadline is five seconds;
- a non-empty caller idempotency key is required and forwarded unchanged;
- each request receives a fresh transport correlation ID.

The transport does **not** require a global `GROUPS_MODERATE` permission before calling the owner. A local group owner/admin/moderator may be authorized by Groups even without platform-wide moderation permission. Conversely, forwarding a platform claim never bypasses the Groups owner's hard owner-target, self-target, revision, source and lifecycle checks.

## Owner-only business semantics

Both mutations construct `GroupMembershipEnforcementCommandService` from `HostRuntimeContext` and invoke only `GroupMembershipEnforcementCommandPort`.

GraphQL does not:

- query or write `groups`, `group_memberships`, `group_membership_enforcements`, receipts, audit or event tables directly;
- recompute owner/admin/moderator hierarchy;
- infer current membership revision;
- rewrite `expected_membership_revision`;
- synthesize replay results;
- alter lifecycle member-count semantics;
- revoke moderation-decision enforcement through a transport shortcut;
- fall back to another transport or service.

The existing owner command remains authoritative for receipt-first replay, deterministic locking, effective actor authorization, owner/self protection, expected-revision CAS, suspension/revocation mutation, group-version/member-count behavior, audit/events and receipt commit.

## Mutation inputs

`suspendGroupMembership` accepts:

- `idempotencyKey`;
- `groupId`;
- `targetUserId`;
- `expectedMembershipRevision`;
- canonical `reasonCode`;
- optional `effectiveUntil`.

`revokeGroupMembershipSuspension` accepts the same identity/CAS inputs without `effectiveUntil`.

Input normalization and semantic validation remain owner responsibilities. The GraphQL transport intentionally does not create a second reason/expiry validator.

## Result

Both mutations return the owner result:

- group, membership and target user IDs;
- post-mutation membership revision;
- post-mutation group version;
- unchanged stored-lifecycle `member_count` value;
- effective membership status;
- enforcement-row revision;
- optional expiry;
- optional revocation timestamp;
- `replayed` marker.

The result does not expose Moderation case/decision/report data or internal receipt/audit rows.

## Errors

Neutral `PortError` kinds retain the common GraphQL transport classification:

- validation/conflict -> GraphQL `BAD_USER_INPUT`;
- not found -> `NOT_FOUND`;
- forbidden -> `PERMISSION_DENIED`;
- unavailable/timeout -> generic `INTERNAL_ERROR` with the public temporary-unavailable message;
- invariant violation -> generic `INTERNAL_ERROR` with the public requires-review message.

The enforcement transport additionally preserves the stable owner error identity in GraphQL extensions:

- `code` remains the platform-wide GraphQL transport classification;
- `domainCode` is the exact stable `PortError.code` returned by the Groups owner;
- `retryable` is the exact neutral `PortError.retryable` flag.

This avoids forcing clients to parse messages or collapse distinct owner conflicts such as `groups.membership_enforcement_revision_conflict`, while keeping unavailable/invariant diagnostics sanitized by the neutral port contract. The transport does not replace the common GraphQL `code` field with a domain-specific value.

Source-level tests retain conflict and unavailable extension mapping.

## Executable SQLite native/GraphQL parity source

`apps/server/tests/groups_membership_enforcement_graphql_sqlite.rs` retains an executable parity packet over a real temporary SQLite file, all production Groups migrations, the production native command service and the stable final Groups GraphQL root.

The packet seeds one local owner and two equivalent lifecycle-active member targets. One target is mutated through `GroupMembershipEnforcementCommandPort`; the other is mutated through `graphql_application_cas::GroupsMutationRoot`. It requires semantic parity for:

- suspend result membership revision, lifecycle member count, effective status and enforcement revision;
- GraphQL receipt replay with the same idempotency key and immutable owner result, changing only `replayed=true`;
- stale expected-revision failure with GraphQL `code=BAD_USER_INPUT`, exact owner `domainCode=groups.membership_enforcement_revision_conflict`, and `retryable=false`;
- revoke result membership revision, lifecycle member count, effective status, enforcement revision and revocation presence;
- monotonic owner `groupVersion` across the sequential native/GraphQL commands rather than pretending two sequential mutations share the same aggregate version.

No synthetic owner result or direct enforcement-table mutation is used. The GraphQL request carries the exact tenant-bound local owner principal with no platform moderation permission, proving that local owner hierarchy remains an owner-domain decision.

This packet is **execution pending**. Its source does not populate `membership_enforcement_command_transport_parity` or promote any runtime gate until a maintainer runs it.

## Executable PostgreSQL native/GraphQL parity source

`apps/server/tests/groups_membership_enforcement_graphql_postgres.rs` mirrors the SQLite packet against PostgreSQL using a unique schema per execution. Every SeaORM pool connection receives the schema through startup options:

```text
options=-csearch_path=<schema>,public
```

The packet intentionally does not rely on session-local `SET search_path`. It applies the same production Groups migrations and repeats native-vs-final-GraphQL suspend, immutable replay, stale revision error extensions and revoke semantics for a local owner with no platform moderation permission.

The PostgreSQL test is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured. Its source is **execution pending** and does not populate transport parity evidence before a maintainer runs it.

## No fallback

The stable module manifest entrypoint remains `graphql_application_cas::GroupsMutationRoot`; the new enforcement mutation is composed inside that root. There is no implicit GraphQL-to-native, native-to-GraphQL, admin, remote, or legacy fallback path.

## Verification

Intentionally not run while preparing this slice:

```bash
cargo check -p rustok-groups --features graphql
cargo test -p rustok-groups --features graphql graphql_conflict_preserves_transport_and_owner_codes
cargo test -p rustok-groups --features graphql graphql_unavailable_keeps_owner_code_and_retryability
cargo test -p rustok-server --features mod-groups --test groups_membership_enforcement_graphql_sqlite -- --nocapture
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' cargo test -p rustok-server --features mod-groups --test groups_membership_enforcement_graphql_postgres -- --ignored --nocapture
node scripts/verify/verify-groups-membership-enforcement-graphql.mjs
```

No Cargo command, test, Node verifier, browser/schema execution, formatter, workflow or CI job was executed. Executed SQLite/PostgreSQL replay/CAS/authorization/schema/error parity remains open.
