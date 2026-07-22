# rustok-groups

## Purpose

`rustok-groups` owns social-group identity, exact-locale presentation, privacy,
memberships, local roles, invitations, membership applications, feature bindings,
and group access policy. Source includes application-policy CAS, append-only policy
history, candidate cancellation, manager reopen, review, governance, receipts, audit,
and native/GraphQL transports.

A group is a social container and policy owner. It is not the persistence owner for
Forum, Blog, Pages, Marketplace, Media, Comments, Notifications inbox/delivery,
search, feed, commerce, payment, order, or fulfillment state.

## Responsibilities

- Own tenant-scoped group identity, lifecycle, visibility, join policy, memberships,
  local roles, invitations, applications, and feature bindings.
- Store language-neutral state separately from exact-locale title, summary, body,
  application questions, and rules. Never select English or an arbitrary first row.
- Separate summary discovery from private-content access and preserve secret-group
  non-disclosure.
- Own bounded invitation digests, expiry, revocation, redemptions, targeted source
  events, and transactional membership activation.
- Own one current application policy per group and immutable policy snapshots on
  submitted applications.
- Capture policy revisions in append-only history in the same transaction as policy
  translation writes.
- Publish `GroupApplicationCasCommandPort` for policy save and candidate submit using
  `(policy_id, revision, locale)`. Stale input returns
  `groups.application_policy_changed` before owner-state writes.
- Publish `GroupApplicationLifecycleReadPort` for the exact candidate's current
  application.
- Publish `GroupApplicationLifecycleCommandPort`:
  - candidate cancel: `pending → cancelled`, membership `pending → left`;
  - manager reopen: `rejected/cancelled → pending`, membership `left → pending`;
  - cancellation and reopen preserve the submitted policy snapshot;
  - fresh candidate resubmit remains a separate current-policy CAS command that
    replaces the snapshot only after success.
- Check receipt replay before CAS/lifecycle state evaluation and commit application,
  membership, group version, audit, and receipt atomically where declared.
- Keep the final GraphQL root CAS-first: legacy unconditional policy-save and submit
  mutations are not exposed.
- Publish module-owned Leptos admin/storefront FFA with explicit selected transport and
  no implicit native/GraphQL fallback.
- Keep Profiles, Notifications, Search, Media, Forum, Blog, Pages, Marketplace, Events,
  and Chat persistence in their owner modules.

The older unconditional Rust policy-save and submit methods remain source-compatibility
methods only. Their removal or versioned deprecation is a separate API migration gate.

## Application lifecycle FFA

Admin:

- filters applications by pending, approved, rejected, or cancelled status;
- reviews pending applications;
- reopens only rejected/cancelled applications through the owner lifecycle port;
- displays the preserved candidate policy snapshot, answers, and acknowledgements.

Storefront:

- reads only the authenticated candidate's current application;
- shows pending status and permits candidate cancellation;
- blocks duplicate submission after approval;
- permits fresh CAS resubmit after rejection or cancellation;
- keeps `apply=<group_uuid>` after cancellation and clears it only after successful
  fresh submission;
- preserves stale-policy reload behavior and never falls back between transports.

## Entry points

- `GroupsModule`, `GroupsService`, `GroupApplicationService`
- `GroupSummaryReadPort`, `GroupMembershipReadPort`, `GroupAccessReadPort`
- `GroupLocalizationReadPort`, `GroupInvitationReadPort`
- `GroupApplicationReadPort`, `GroupApplicationPolicyHistoryReadPort`
- `GroupApplicationLifecycleReadPort`
- `GroupApplicationCasCommandPort`, `GroupApplicationLifecycleCommandPort`
- `GroupCommandPort`, `GroupLocalizationCommandPort`, `GroupInvitationCommandPort`
- `GroupTargetedInvitationCommandPort`, `GroupApplicationCommandPort`
- `GroupGovernanceCommandPort`
- `graphql_application_cas::GroupsQueryRoot`
- `graphql_application_cas::GroupsMutationRoot`
- `rustok_groups_admin::reopen_group_admin_membership_application`
- `rustok_groups_storefront::load_groups_storefront_my_application`
- `rustok_groups_storefront::cancel_groups_storefront_membership_application`

## Readiness

Source presence does not prove build, migration, GraphQL schema, parity, replay,
concurrency, lock ordering, security, accessibility, retry, or recovery behavior.
FFA, FBA, GROUPS-06, and GROUPS-19 remain `in_progress`; policy-history, policy-CAS,
and application-lifecycle runtime evidence remain `null`.

## Documentation

- [Live module contract](docs/README.md)
- [Canonical implementation plan](docs/implementation-plan.md)
- [FBA registry](contracts/groups-fba-registry.json)
- [Platform documentation map](../../docs/index.md)
