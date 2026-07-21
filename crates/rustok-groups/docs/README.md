# Groups module runtime contract

## Purpose

`rustok-groups` provides the social-container boundary for communities inside a
RusToK tenant. It combines phpFox-style modular groups with RusToK owner-module,
FFA, FBA, multilingual storage, tenant isolation, and headless transport rules.

## Responsibility zone

### Owned state

Groups owns:

- group identity, tenant, owner, handle, lifecycle, visibility, and join policy;
- localized group title, summary, and body;
- memberships, local roles, membership state, and ownership transfer;
- invitation records, token digests, redemption state, and revocation;
- membership-application policies, exact-locale questions/rules, submissions,
  answers, acknowledgements, and review state;
- feature bindings and provider-specific versioned configuration;
- command receipts, immutable audit entries, and append-only Groups semantic source
  events.

Bans, policy revision history, bulk review, and additional moderation state remain
planned slices.

### State owned elsewhere

Groups does not own:

- auth users, credentials, or sessions;
- profile presentation;
- media binary objects;
- forum topics/replies, blog posts/comments, or Pages documents;
- products, marketplace listings, checkout, orders, payments, or fulfillment;
- notification inbox, preferences, fan-out, channels, retry, or delivery receipts;
- search projections, feed entries, or analytics.

No Groups table has a foreign key to another optional domain module. Cross-domain
references are logical typed identifiers resolved through public ports.

## Multilingual database contract

Language-neutral state belongs to base tables. Localized business copy belongs to
parallel exact-locale rows:

- `groups` stores language-neutral identity and policy state;
- `group_translations` stores title, summary, and body;
- `group_membership_policies` stores language-neutral application-policy revision
  and enabled state;
- `group_membership_policy_translations` stores bounded questions and rules by
  normalized `locale VARCHAR(32)`;
- `group_membership_applications` stores the exact policy revision, locale, and
  immutable question/rule snapshot seen by the candidate.

The host supplies the already-resolved effective locale through `PortContext`.
Groups normalizes that locale and selects only the exact row. It never injects an
English fallback, arbitrary first row, or another stored locale. Missing localized
copy is an explicit unavailable/not-found condition.

Base JSON state such as `groups.metadata`, membership metadata, and feature
configuration must remain language-agnostic. Application `policy_snapshot` is not a
shadow localization store: it is immutable evidence copied from one explicitly
selected exact-locale owner policy when the candidate submits.

### Group presentation management

`GroupLocalizationReadPort` lists exact stored translation rows.
`GroupLocalizationCommandPort` owns exact-locale upsert and delete.

- management requires active owner/admin or platform `groups:manage`;
- the owner service rechecks authorization inside the transaction;
- mutations target only `(tenant_id, group_id, locale)`;
- upsert never clones fallback copy;
- delete rejects the final translation row;
- mutation and group-version increment commit together;
- localization replay/concurrency evidence remains open.

## Access and privacy

Visibility values:

- `public`: localized shell and public feature bindings are readable;
- `closed`: summary shell is discoverable, while private body, member list,
  features, and provider content require active membership or platform authority;
- `secret`: non-members receive not-found semantics, including direct reads and
  membership-application attempts.

The access contract separates discovery, summary-shell access, and private-content
access. A provider failure never grants private access. No transport fallback may
retry an owner denial or timeout through a different path.

Join policies:

- `open`;
- `request`;
- `invite_only`.

Local roles are `owner`, `admin`, `moderator`, and `member`. Platform RBAC protects
operator surfaces; Groups local policy protects one group. Owner services enforce
both boundaries.

## Invitation contract

`GroupInvitationReadPort` owns manager-visible invitation listings.
`GroupInvitationCommandPort` owns create, revoke, and token acceptance.
`GroupTargetedInvitationCommandPort` owns authenticated targeted accept-by-ID.

- owner/admin/moderator or platform manage authority may create/list/revoke;
- targeted invitations are single-use;
- shareable links permit at most 100 uses and expire within 300 seconds to 30 days;
- plaintext token is returned only by the first successful create response;
- persistence, audit, receipts, and semantic events store only the SHA-256 digest or
  token-free identifiers;
- replayed create returns `token = null`;
- invalid, expired, exhausted, revoked, or wrong-target credentials use stable
  unavailable/not-found semantics;
- redemption, membership activation, member count, group version, audit, and receipt
  commit in one owner transaction.

### Targeted semantic delivery

Targeted invitation insert appends `groups.invitation.targeted_created` to the
append-only `group_domain_events` table through a database trigger. Invite and event
commit or roll back together. The event contains invitation, group, recipient, and
actor identifiers only—never token, digest, email, profile copy, or localized copy.

Groups registers a deferred `GroupsNotificationSourceProviderFactory` through
`rustok-notifications-api`. Once materialized by a Notifications-enabled host, it:

- recognizes only the supported targeted invitation event revision;
- resolves at most one recipient, exactly the current target user;
- suppresses revoked, expired, exhausted, redeemed, missing, or inactive-group
  invitations;
- authorizes target opening only for the exact recipient;
- returns `/modules/groups?invitation=<uuid>`.

Notifications inbox, preference, fan-out, email/push, retry, and cleanup remain
Notifications-owner responsibilities. Groups commands have no synchronous
Notifications dependency.

## Membership-application contract

### Policy

`GroupApplicationReadPort::read_group_application_policy` exposes the current
policy for the host-resolved exact locale.
`GroupApplicationCommandPort::upsert_group_application_policy` manages it.

- one current language-neutral policy exists per group;
- each exact-locale row has at most 20 questions and 20 rules;
- stable normalized keys identify questions and rules across UI/transport;
- prompts, help text, rule copy, and answer limits are bounded;
- policy management requires active owner/admin or platform manage authority;
- upsert increments policy revision and group version and stores receipt/audit in
  the same transaction;
- no module-local locale fallback exists.

### Submission

`GroupApplicationCommandPort::submit_group_membership_application` accepts an
authenticated candidate submission.

- only active `request` groups accept applications;
- secret groups return not-found semantics;
- banned users and active members are rejected;
- pending or approved applications cannot be submitted again;
- rejected applications may be resubmitted;
- unknown answer keys and rule acknowledgements are rejected;
- every required question must have a bounded non-empty answer;
- every required rule must be acknowledged;
- the current policy ID, revision, locale, questions, rules, answers, and
  acknowledgements are stored as submission evidence;
- pending membership, application snapshot, group version, audit, and receipt commit
  together.

### Review

`GroupApplicationReadPort::list_group_membership_applications` and
`GroupApplicationCommandPort::review_group_membership_application` are restricted
to active owner/admin/moderator or platform manage authority.

- only pending applications may be reviewed;
- approve activates membership, records `joined_at`, and increments member count;
- reject moves membership to `left` and records `left_at`;
- review notes are optional and bounded to 2,000 characters;
- application, membership, group version, audit, and receipt commit together;
- group/application rows use exclusive locks where supported.

Policy revision history, candidate cancellation, bulk review, stale-policy UX, and
runtime concurrency evidence remain open.

## FBA contract

Published ports include:

- `GroupSummaryReadPort`;
- `GroupMembershipReadPort`;
- `GroupAccessReadPort`;
- `GroupLocalizationReadPort` and `GroupLocalizationCommandPort`;
- `GroupInvitationReadPort`, `GroupInvitationCommandPort`, and
  `GroupTargetedInvitationCommandPort`;
- `GroupApplicationReadPort` and `GroupApplicationCommandPort`;
- `GroupCommandPort` and `GroupGovernanceCommandPort`.

All use `PortContext`, `PortCallPolicy`, and `PortError`.

Required semantics:

- tenant and actor are mandatory;
- reads require deadline;
- writes require deadline plus idempotency key;
- locale/channel/correlation context crosses transports;
- private access fails closed when Groups is unavailable;
- optional feature-provider failure affects only that feature;
- owner state, receipt, audit, membership, and group version commit atomically where
  declared;
- consumers never import Groups entities or query Groups tables directly.

The final GraphQL composition is
`graphql_applications::GroupsQueryRoot` / `GroupsMutationRoot`. It merges core,
localization, governance, invitation, targeted invitation, and application fields.
Native server functions and GraphQL adapters call the same owner ports.

## FFA contract

Admin and storefront packages retain `core → transport → UI` separation.
Application-specific files add to, rather than bypass, that structure:

```text
admin/src/application_core.rs
admin/src/application_model.rs
admin/src/transport/native_applications_adapter.rs
admin/src/transport/graphql_applications_adapter.rs
admin/src/ui/applications.rs

storefront/src/application_core.rs
storefront/src/application_model.rs
storefront/src/transport/native_applications_adapter.rs
storefront/src/transport/graphql_applications_adapter.rs
storefront/src/ui/application.rs
```

- framework-neutral cores contain no Leptos imports;
- UI imports only the transport facade, never raw adapters;
- native `#[server]` is preferred for SSR/hydrate;
- GraphQL supports CSR, mobile, Next.js, and external clients;
- selected transport never falls back implicitly;
- locale comes from host route/request context;
- authenticated GraphQL transport uses the host auth session.

The admin workspace loads pending applications and displays the stored policy
revision/locale, answers, and acknowledged rule keys before approve/reject. Manual
UUID entry remains an intermediate operator surface. A visual policy editor,
pickers, bulk review, confirmation, and audit history remain planned.

The storefront lists an “Apply to join” action for request-policy groups. The
`apply=<group_uuid>` route loads the exact-locale policy, renders dynamic bounded
questions/rules, and submits through the selected facade. The query key is removed
only after success so failed submissions remain retryable.

## Integration

Feature bindings are namespaced and versioned, for example:

- `forum.discussions`;
- `blog.posts`;
- `pages.wiki`;
- `marketplace.store`;
- `media.gallery`;
- `events.calendar`;
- `chat.room`.

A binding expresses policy/configuration only. It does not transfer persistence
ownership. The host composes provider-owned UI; Groups never embeds another
module’s business screens or persistence.

## Degraded modes

- Groups access provider unavailable: deny private content.
- Exact-locale application policy missing: hide/disable the application form; never
  select another locale.
- Native or GraphQL transport failure: surface the selected-path error; do not retry
  through the other transport.
- Profiles unavailable: display stable UUID/placeholder, never copy profile state.
- Notifications unavailable: owner commands commit and remain authoritative.
- Search/index unavailable: owner writes commit; future projections catch up
  asynchronously.

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
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

No build, test, migration, verifier, parity, concurrency, accessibility, security,
retry, or recovery command was executed for the membership-application source
slice. FFA/FBA and GROUPS-06 therefore remain `in_progress`; runtime evidence keys
remain `null`.

## Related documents

- [Canonical implementation plan](implementation-plan.md)
- [Root module README](../README.md)
- [FBA registry](../contracts/groups-fba-registry.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)
- [Database contract](../../../docs/architecture/database.md)
- [FFA package architecture](../../../docs/UI/module-package-architecture.md)
- [FBA architecture](../../../docs/backend/module-backend-architecture.md)
