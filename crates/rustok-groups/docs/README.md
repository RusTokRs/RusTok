# Groups module runtime contract

## Purpose

`rustok-groups` provides the social-container boundary for communities inside a
RusToK tenant. It combines phpFox-style modular groups with RusToK owner-module,
FFA, FBA, multilingual storage, tenant isolation, and headless transport rules.

## Responsibility zone

Groups owns group identity, localized presentation, visibility, join policy,
memberships, local roles, invitations, membership applications, ordered questions
and rules, policy revision history, feature bindings, command receipts, audit, and
Groups semantic source events.

Groups does not own auth users/sessions, profile presentation, media binaries, Forum,
Blog, Pages, Marketplace, comments, notification inbox/delivery, search projections,
feed entries, checkout, payment, orders, fulfillment, or analytics.

No Groups table has a foreign key to another optional domain module. Cross-domain
references are logical typed identifiers resolved through public ports.

## Multilingual database contract

Language-neutral state belongs to base tables. Localized business copy belongs to
parallel exact-locale rows:

- `groups` stores language-neutral identity and policy state;
- `group_translations` stores title, summary, and body;
- `group_membership_policies` stores current language-neutral application-policy
  revision/enabled state;
- `group_membership_policy_translations` stores ordered questions and rules by
  normalized `locale VARCHAR(32)`;
- `group_membership_policy_revisions` stores append-only exact-locale snapshots of
  successful policy writes;
- `group_membership_applications` stores the exact policy ID, revision, locale, and
  immutable question/rule snapshot seen by the candidate.

The host supplies the already-resolved effective locale through `PortContext`.
Groups normalizes it and selects only that row. It never injects an English fallback,
arbitrary first row, or another stored locale. Missing copy is explicit unavailable
or not-found state.

Application `policy_snapshot` and policy revision rows are immutable evidence, not
shadow localization stores. Current canonical policy copy remains the exact-locale
translation row.

## Access and privacy

- `public`: localized shell and public features are readable;
- `closed`: summary shell is discoverable, while body, members, features, and provider
  content require active membership or platform authority;
- `secret`: non-members receive not-found semantics, including application attempts.

The access contract separates discovery, summary-shell access, and private-content
access. Provider failure never grants private access, and a transport error never
causes implicit retry through another path.

Join policies are `open`, `request`, and `invite_only`. Local roles are `owner`,
`admin`, `moderator`, and `member`. Platform RBAC protects operator surfaces while
Groups policy protects one group; owner services enforce both.

## Invitation contract

`GroupInvitationReadPort`, `GroupInvitationCommandPort`, and
`GroupTargetedInvitationCommandPort` own manager listing, create/revoke/token
acceptance, and authenticated targeted accept-by-ID.

Targeted invitations are single-use. Shareable links permit at most 100 uses and
expire within 300 seconds to 30 days. Plaintext is returned only by the first create
response; persistence, audit, receipts, and semantic events contain no recoverable
plaintext. Redemption, membership activation, member count, group version, audit,
and receipt commit in one owner transaction.

Targeted insert appends `groups.invitation.targeted_created` to append-only
`group_domain_events` through a database trigger. The event carries only typed
invitation/group/recipient/actor identifiers. Notifications inbox, preferences,
fan-out, email/push, retry, and cleanup remain Notifications-owner responsibilities.

## Membership-application contract

### Current policy and history

`GroupApplicationReadPort::read_group_application_policy` exposes the current policy
for the host-resolved exact locale.

- one current language-neutral policy exists per group;
- each exact-locale row contains at most 20 questions and 20 rules;
- ordered stable normalized keys identify questions and rules;
- prompt/help/rule copy and answer limits are bounded;
- management requires active owner/admin or platform `groups:manage`;
- no module-local locale fallback exists.

Every successful application-policy translation INSERT/UPDATE is captured into
`group_membership_policy_revisions` by PostgreSQL or SQLite trigger. Capture occurs in
the same database transaction as the current policy write. Existing rows are
backfilled during migration, and history rows reject UPDATE and DELETE.

`GroupApplicationPolicyHistoryReadPort::list_group_application_policy_revisions`
provides manager-only history ordered by revision descending and locale ascending.
It reuses the application-review authorization boundary: active owner/admin/moderator
or platform manage authority. Candidates cannot enumerate policy history.

### Atomic policy preconditions

Interactive policy save and candidate submit use `GroupApplicationCasCommandPort`:

- `upsert_group_application_policy_if_current`;
- `submit_group_membership_application_if_current`.

Both requests carry `GroupApplicationPolicyPrecondition` containing the policy ID,
positive revision, and exact locale rendered by the client. The owner:

1. validates deadline, idempotency, tenant, actor, locale, and request bounds;
2. returns an identical committed receipt before checking the policy again;
3. locks the group row where supported;
4. repeats authorization and group-state checks;
5. reloads current policy state and compares ID, revision, and locale;
6. returns `groups.application_policy_changed` on mismatch before any owner-state
   write;
7. commits successful policy/application state, group version, audit, and receipt in
   one transaction.

For a new policy, `expected_policy = null` is accepted only while no current policy
exists. Updating an existing policy requires a matching precondition.

The older unconditional save and submit methods on `GroupApplicationCommandPort`
remain public for source compatibility. Module-owned FFA does not use them. Their
removal or versioned deprecation remains an API migration gate.

### Submission

Only active `request` groups accept applications. Secret groups return not-found
semantics; banned users and active members are rejected. Pending or approved
applications cannot be resubmitted, while rejected applications may receive a fresh
snapshot.

Unknown answer keys and rule acknowledgements are rejected. Every required question
must contain a bounded non-empty answer, and every required rule must be acknowledged.
A stale form returns `groups.application_policy_changed` without changing membership,
application, group version, audit, or receipt state.

A successful submit stores pending membership, application snapshot, group version,
audit, and receipt together.

### Review

Listing/review through `GroupApplicationReadPort` and
`GroupApplicationCommandPort::review_group_membership_application` requires active
owner/admin/moderator or platform manage authority. Only pending applications may be
reviewed. Approve activates membership and increments member count; reject moves
membership to `left`. Review note is optional and bounded to 2,000 characters.
Application, membership, group version, audit, and receipt commit together.

## FBA contract

Published ports include summary, membership, access, localization, invitation,
targeted invitation, application read, application policy history, application CAS,
group command, and governance boundaries. All use `PortContext`, `PortCallPolicy`,
and `PortError`. Reads require a deadline; writes require deadline plus idempotency
key. Consumers never import Groups entities or query Groups tables directly.

The final GraphQL composition is
`graphql_application_cas::GroupsQueryRoot` / `GroupsMutationRoot`. It retains core,
localization, governance, invitation, targeted invitation, application, and policy
history fields and adds the two CAS mutations. Native server functions and GraphQL
adapters call the same owner ports.

## FFA contract

Admin and storefront packages retain `core → transport → UI` separation. UI imports
only the transport facade, never raw adapters. Selected native or GraphQL transport
never falls back implicitly.

The admin policy editor captures the loaded policy identity and sends it directly to
the CAS mutation. On `groups.application_policy_changed`, it keeps the stale identity
and requires explicit reload before another save. The locale is read-only and comes
from host route context. Revision history remains manager-only.

The storefront uses `apply=<group_uuid>` to load the exact-locale policy and render
dynamic questions/rules. Submit carries the loaded policy identity. A stale conflict
preserves the route, disables repeated submission, and exposes explicit reload that
clears old answers and acknowledgements before loading the current policy. The query
key is removed only after success.

## Degraded modes

- Groups provider unavailable: deny private content.
- Exact-locale policy unavailable: disable application/editor; never choose another
  locale.
- Policy CAS conflict: write no owner state, preserve the selected transport error,
  and require explicit reload before retry.
- Policy history unavailable: current owner policy remains authoritative; hide
  history rather than synthesizing revisions.
- Native or GraphQL transport failure: surface selected-path error; never retry
  through another transport.
- Profiles unavailable: display stable UUID/placeholder, never copy canonical profile
  state.
- Notifications unavailable: owner commands commit and remain authoritative.
- Search/index unavailable: owner writes commit; future projections may catch up.

## Open gates

The following remain source or evidence work:

- remove or version-deprecate legacy unconditional application command methods;
- explicit multi-locale policy management contract and picker;
- candidate cancellation/reopen/resubmit policy;
- bounded bulk review and per-item audit/result handling;
- ProfilesReader summaries and application semantic events;
- migration execution/backfill/immutability evidence;
- native/GraphQL stable-code parity, replay, stale races, concurrency, lock ordering,
  accessibility, security, retry, and recovery evidence.

## Verification

Expected commands before readiness promotion:

```bash
cargo xtask module validate groups
cargo check -p rustok-groups --features graphql
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
cargo test -p rustok-groups
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-groups-localization-boundary.mjs
node scripts/verify/verify-groups-invitations-boundary.mjs
node scripts/verify/verify-groups-targeted-invitation-delivery.mjs
node scripts/verify/verify-groups-membership-applications.mjs
node scripts/verify/verify-groups-membership-policy-revisions.mjs
node scripts/verify/verify-groups-application-policy-cas.mjs
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

No build, test, migration, verifier, parity, replay, stale-race, concurrency,
accessibility, security, retry, or recovery command was executed for this source
slice. FFA, FBA, GROUPS-06, and GROUPS-19 remain `in_progress`; runtime evidence keys
remain `null`.

## Related documents

- [Canonical implementation plan](implementation-plan.md)
- [Root module README](../README.md)
- [FBA registry](../contracts/groups-fba-registry.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)
- [Database contract](../../../docs/architecture/database.md)
- [FFA package architecture](../../../docs/UI/module-package-architecture.md)
- [FBA architecture](../../../docs/backend/module-backend-architecture.md)
