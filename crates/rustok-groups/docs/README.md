# Groups module runtime contract

## Purpose and ownership

`rustok-groups` is the tenant-scoped social-container boundary. It owns group identity,
exact-locale presentation, visibility, join policy, memberships, local roles,
invitations, membership applications, policy history, feature bindings, receipts,
audit, and Groups semantic source events.

It does not own auth/session state, profiles, media binaries, Forum/Blog/Pages/
Marketplace content, comments, notification inbox/delivery, search projections, feed,
commerce, payment, orders, fulfillment, or analytics. Cross-domain references use
typed identifiers and public ports; Groups tables do not foreign-key optional modules.

## Multilingual and privacy contract

Language-neutral state belongs to base tables. Localized copy belongs to exact-locale
rows. The host supplies the resolved locale through `PortContext`; Groups normalizes
and selects only that row. There is no English, arbitrary-first-row, or module fallback.

- public groups expose localized shell and public features;
- closed groups expose a summary shell but gate private content;
- secret groups return not-found to non-members, including application attempts;
- provider failure never grants private access;
- selected transport errors never trigger implicit fallback.

## Invitation contract

`GroupInvitationReadPort`, `GroupInvitationCommandPort`, and
`GroupTargetedInvitationCommandPort` own listing, create/revoke, token acceptance, and
authenticated targeted accept-by-ID. Plaintext tokens are returned once and never
persisted in owner state, audit, receipts, or events. Targeted inserts append
`groups.invitation.targeted_created` without token data. Notifications delivery remains
Notifications-owned and optional to the Groups command transaction.

## Membership-application persistence

- `group_membership_policies`: current language-neutral revision/enabled state;
- `group_membership_policy_translations`: ordered exact-locale questions/rules;
- `group_membership_policy_revisions`: append-only successful-write snapshots;
- `group_membership_applications`: exact policy identity/revision/locale, immutable
  snapshot, answers, acknowledgements, status, and review metadata.

Policy revision capture occurs in the same database transaction as policy translation
writes. Revision rows reject update/delete.

## Application policy CAS

Interactive policy save and candidate submit use `GroupApplicationCasCommandPort`:

- `upsert_group_application_policy_if_current`;
- `submit_group_membership_application_if_current`.

Requests carry `policy_id`, positive `revision`, and exact `locale`. The owner checks an
identical receipt first, locks the group row, repeats authorization/group checks,
reloads current policy, and compares the precondition before owner-state writes. A
mismatch returns `groups.application_policy_changed`. Successful state, version, audit,
and receipt commit together.

Legacy unconditional Rust save/submit methods remain compatibility-only. Final GraphQL
and module-owned FFA do not expose or use them.

## Application lifecycle

### Exact-candidate read

`GroupApplicationLifecycleReadPort::read_my_group_membership_application` returns only
the authenticated actor's application for the requested tenant/group. It never permits
cross-candidate enumeration.

### Candidate cancellation

`GroupApplicationLifecycleCommandPort::cancel_group_membership_application`:

- requires the exact candidate; another actor receives not-found semantics;
- accepts only `pending` applications with a still-pending, non-banned membership;
- moves membership to `left`, records `left_at`, and application to `cancelled`;
- clears review metadata while preserving submitted policy snapshot, answers, and
  acknowledgements;
- commits application, membership, group version, audit
  `group.membership_application_cancelled`, and receipt atomically.

### Manager reopen

`GroupApplicationLifecycleCommandPort::reopen_group_membership_application`:

- requires active owner/admin/moderator or platform authority;
- accepts only `rejected` or `cancelled` applications;
- requires an active non-secret `request` group and a `left`, non-banned, non-active
  membership;
- restores application/membership to `pending` and clears review metadata;
- preserves submitted timestamp, policy identity/revision/locale, snapshot, answers,
  and acknowledgements;
- commits version, audit `group.membership_application_reopened`, and receipt atomically.

A fresh candidate resubmit is not reopen. It uses current-policy CAS and replaces the
snapshot only after successful submission. Pending and approved applications cannot be
freshly submitted.

### Review

Manager review accepts only pending applications. Approve activates membership and
increments member count; reject moves membership to `left`. Review, cancellation, and
reopen use application-then-group lock ordering where supported.

## FBA contract

Published boundaries include summary, membership, access, localization, invitation,
application policy/history, application CAS, application lifecycle, group command, and
governance ports. All use `PortContext`, `PortCallPolicy`, and `PortError`. Reads require
a deadline; writes require deadline plus idempotency key. Consumers never import Groups
entities or query Groups tables.

Final GraphQL remains
`graphql_application_cas::GroupsQueryRoot` / `GroupsMutationRoot`. It composes core,
localization, governance, invitations, policy/history, CAS save/submit, review,
exact-candidate application read, cancellation, and reopen. Legacy unconditional
application mutations remain absent.

## FFA contract

Admin/storefront preserve `core → transport → UI` separation. UI imports only the
facade. Native and GraphQL paths never fall back.

Admin:

- filters pending, approved, rejected, and cancelled applications;
- reviews pending rows;
- exposes reopen only on rejected/cancelled rows;
- renders the preserved policy snapshot and candidate answers.

Storefront:

- loads exact-candidate current application before exposing controls;
- pending shows cancellation and hides submit;
- approved blocks duplicate submit;
- rejected/cancelled permit a fresh current-policy CAS form;
- cancellation preserves `apply=<group_uuid>` and reloads current state;
- successful fresh submit clears `apply`;
- stale-policy reload clears old answers and acknowledgements.

## Degraded modes

- Groups unavailable: deny private content.
- Exact-locale policy unavailable: disable form/editor; never choose another locale.
- Current-application read unavailable: do not guess status or expose submit controls.
- CAS conflict: write no state and require explicit reload.
- Lifecycle transport failure: preserve selected-path error and route; never fall back.
- Profiles unavailable: show UUID/placeholder without copied canonical state.
- Notifications/search unavailable: owner writes remain authoritative.

## Open gates and verification

Open work includes legacy Rust API migration, multi-locale manager picker, bounded bulk
review, ProfilesReader summaries, application semantic events, pagination/pickers,
audit/receipt history, parity, replay, CAS/lifecycle concurrency, lock ordering,
migration execution, accessibility, security, retry, and recovery.

Expected commands before readiness promotion include:

```bash
cargo xtask module validate groups
cargo check -p rustok-groups --features graphql
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
cargo test -p rustok-groups
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-groups-membership-applications.mjs
node scripts/verify/verify-groups-membership-policy-revisions.mjs
node scripts/verify/verify-groups-application-policy-cas.mjs
node scripts/verify/verify-groups-application-lifecycle.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

No build, test, migration, verifier, GraphQL schema, parity, replay, concurrency,
accessibility, security, retry, or recovery command was executed for this source slice.
FFA, FBA, GROUPS-06, and GROUPS-19 remain `in_progress`; runtime evidence remains
`null`.

## Related documents

- [Canonical implementation plan](implementation-plan.md)
- [Root module README](../README.md)
- [FBA registry](../contracts/groups-fba-registry.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)
- [Database contract](../../../docs/architecture/database.md)
- [FFA package architecture](../../../docs/UI/module-package-architecture.md)
- [FBA architecture](../../../docs/backend/module-backend-architecture.md)
