# Groups membership enforcement GraphQL contract

Status: **source-ready / maintainer execution pending**

## Scope

`graphql_membership_enforcement::GroupsMutationRoot` adds exactly two direct Groups enforcement mutations to the existing final mutation chain:

- `suspendGroupMembership`;
- `revokeGroupMembershipSuspension`.

The wrapper composes the previous `graphql_application_cas::GroupsMutationRoot` through `MergedObject`, so application, invitation, governance, localization and core Groups mutations remain present. The module manifest points only the final mutation entrypoint at the new wrapper; the query root is unchanged.

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

Neutral `PortError` kinds map consistently with existing Groups transports:

- validation/conflict -> GraphQL bad user input;
- not found -> not found;
- forbidden -> permission denied;
- unavailable/timeout -> generic temporary-unavailable internal error;
- invariant violation -> generic requires-review internal error.

Owner stable error codes remain authoritative internally. Runtime schema/error-extension parity still requires executable evidence; source presence alone does not promote that gate.

## No fallback

The module manifest composes this source into the final Groups mutation root. There is no implicit GraphQL-to-native, native-to-GraphQL, admin, remote, or legacy fallback path.

## Verification

Intentionally not run while preparing this slice:

```bash
cargo check -p rustok-groups --features graphql
cargo test -p rustok-groups
node scripts/verify/verify-groups-membership-enforcement-graphql.mjs
```

No Cargo command, test, Node verifier, browser/schema execution, formatter, workflow or CI job was executed. Runtime replay/CAS/authorization/schema/error parity remains open.
