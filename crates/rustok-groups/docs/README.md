# Groups module runtime contract

## Purpose

`rustok-groups` provides the social-container boundary for communities inside a
RusToK tenant. It combines phpFox-style modular groups with RusToK owner-module,
FFA, FBA, multilingual storage, tenant isolation, and headless transport rules.

## Responsibility zone

Groups owns group identity, localized presentation, visibility, join policy,
memberships, local roles, invitations, membership applications, ordered questions
and rules, policy locale management, policy revision history, application lifecycle,
feature bindings, command receipts, audit, and Groups semantic source events.

Groups does not own auth users/sessions, profile presentation, media binaries, Forum,
Blog, Pages, Marketplace, comments, notification inbox/delivery, search projections,
feed entries, checkout, payment, orders, fulfillment, or analytics.

No Groups table has a foreign key to another optional domain module. Cross-domain
references are logical typed identifiers resolved through public ports.

## Multilingual database and locale contract

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
- `group_membership_applications` stores exact policy ID, revision, locale, and
  immutable question/rule snapshot seen by the candidate.

Candidate presentation uses the host-resolved effective locale in `PortContext`.
Groups normalizes it and selects only that row. It never injects an English fallback,
arbitrary first row, or another stored locale.

Policy management is deliberately separate:

- `PortContext.locale` remains the host request/UI locale;
- the selected policy locale is a normalized field on a typed owner request;
- locale catalog returns only existing translation locales in ascending order;
- management read selects only the requested locale;
- missing policy returns an empty view without policy ID/revision;
- missing translation on an existing policy returns an empty view with current policy
  ID/revision and `translation_exists=false`;
- native and GraphQL adapters never substitute selected policy locale into request
  locale context or locale headers;
- missing copy is explicit empty/unavailable state, never fallback.

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

### Candidate policy read

`GroupApplicationReadPort::read_group_application_policy` exposes current policy for
the host-resolved exact locale.

- one current language-neutral policy exists per group;
- each exact-locale row contains at most 20 questions and 20 rules;
- ordered stable normalized keys identify questions and rules;
- prompt/help/rule copy and answer limits are bounded;
- candidate reads use only `PortContext.locale`;
- no module-local locale fallback exists.

### Policy management read

`GroupApplicationPolicyManagementReadPort` exposes:

- `list_group_application_policy_locales`;
- `read_group_application_policy_for_management`.

Both require active owner/admin or platform `groups:manage`, matching policy writes.
The selected exact locale comes from the typed request rather than `PortContext`.
Candidates cannot enumerate the management catalog or views.

A missing policy yields an empty management view without a precondition. A missing
selected translation on an existing policy yields an empty view with current policy
ID/revision and selected locale. The admin editor may therefore create that exact
translation through owner CAS without accepting a stale concurrent policy revision.

### Current policy history

Every successful application-policy translation INSERT/UPDATE is captured into
`group_membership_policy_revisions` by PostgreSQL or SQLite trigger. Capture occurs in
the same database transaction as the current policy write. Existing rows are
backfilled during migration, and history rows reject UPDATE and DELETE.

`GroupApplicationPolicyHistoryReadPort::list_group_application_policy_revisions`
provides manager-only history ordered by revision descending and locale ascending.
It uses active owner/admin/moderator or platform manage authorization. Candidates
cannot enumerate policy history.

### Atomic policy preconditions

Interactive policy save and candidate submit use `GroupApplicationCasCommandPort`:

- `upsert_group_application_policy_if_current`;
- `submit_group_membership_application_if_current`.

Both requests carry `GroupApplicationPolicyPrecondition` containing policy ID,
positive revision, and exact locale rendered by the client. The owner:

1. validates deadline, idempotency, tenant, actor, locale, and request bounds;
2. returns an identical committed receipt before checking policy again;
3. locks an existing candidate application before the group for resubmit, or locks
   the group and rechecks candidate application for a first submission;
4. repeats authorization and group-state checks;
5. reloads current policy state and compares ID, revision, and locale;
6. returns `groups.application_policy_changed` on mismatch before owner-state write;
7. commits successful policy/application state, group version, audit, and receipt in
   one transaction.

Policy upsert does not require an application lock. For a new policy,
`expected_policy = null` is accepted only while no current policy exists. Updating an
existing policy, including creating a new locale translation for it, requires a
matching current policy precondition carrying the selected locale.

The older unconditional save and submit methods on `GroupApplicationCommandPort`
remain public for source compatibility. Module-owned FFA and final GraphQL do not use
or expose them. Their removal or versioned deprecation remains an API migration gate.

### Submission

Only active `request` groups accept applications. Secret groups return not-found
semantics; banned users and active members are rejected. Pending or approved
applications cannot be resubmitted, while rejected and cancelled applications may
receive a fresh current-policy snapshot.

Unknown answer keys and rule acknowledgements are rejected. Every required question
must contain a bounded non-empty answer, and every required rule must be acknowledged.
A stale form returns `groups.application_policy_changed` without changing membership,
application, group version, audit, or receipt state.

A successful submit stores pending membership, application snapshot, group version,
audit, and receipt together.

### Exact-candidate application read

`GroupApplicationLifecycleReadPort::read_my_group_membership_application` returns only
the authenticated actor's current application for the requested tenant/group. The
query cannot enumerate another candidate's application. Storefront uses this read
before exposing submit or cancellation controls.

### Candidate cancellation

`GroupApplicationLifecycleCommandPort::cancel_group_membership_application`:

- accepts only exact candidate; another actor receives not-found semantics;
- accepts only pending application whose membership is still pending and not banned;
- locks application then group where supported;
- moves membership to `left`, records `left_at`, and marks application `cancelled`;
- clears review metadata but preserves submitted policy identity/revision/locale,
  snapshot, answers, and acknowledgements;
- commits application, membership, group version, audit
  `group.membership_application_cancelled`, and receipt together.

Cancellation keeps `apply=<group_uuid>` in storefront route. After refetch, candidate
may prepare a fresh CAS submission using current policy.

### Manager reopen

`GroupApplicationLifecycleCommandPort::reopen_group_membership_application`:

- requires active owner/admin/moderator or platform manage authority;
- authorizes before disclosing or validating current application status;
- accepts only rejected or cancelled applications;
- requires active non-secret `request` group and left, non-banned, non-active
  membership;
- locks application then group where supported;
- restores membership/application to `pending` and clears prior review metadata;
- preserves submitted timestamp, policy identity/revision/locale, snapshot, answers,
  and acknowledgements;
- commits group version, audit `group.membership_application_reopened`, and receipt
  together.

Reopen preserves snapshot for later manager review. A fresh candidate resubmit is a
distinct CAS command that replaces snapshot only after success.

### Review

Listing/review through `GroupApplicationReadPort` and
`GroupApplicationCommandPort::review_group_membership_application` requires active
owner/admin/moderator or platform manage authority. Only pending applications may be
reviewed. Approve activates membership and increments member count; reject moves
membership to `left`. Review note is optional and bounded to 2,000 characters.
Application, membership, group version, audit, and receipt commit together.

## FBA contract

Published ports include summary, membership, access, localization, invitation,
targeted invitation, application read, application policy history, application policy
management, application CAS, application lifecycle read/command, group command, and
governance boundaries. All use `PortContext`, `PortCallPolicy`, and `PortError`. Reads
require deadline; writes require deadline plus idempotency key. Consumers never import
Groups entities or query Groups tables directly.

Final GraphQL composition is
`graphql_application_cas::GroupsQueryRoot` / `GroupsMutationRoot`. It retains core,
localization, governance, invitation, targeted invitation, application, and history
fields and adds policy locale catalog, selected-locale management view, CAS
save/submit, exact-candidate read, review, cancel, and reopen. Legacy unconditional
application mutations remain absent. Native server functions and GraphQL adapters call
the same owner ports.

## FFA contract

Admin and storefront packages retain `core → transport → UI` separation. UI imports
only transport facade, never raw adapters. Selected native or GraphQL transport never
falls back implicitly.

The admin policy editor:

- loads locale catalog and selected management view through explicit facade methods;
- uses a datalist for existing or new exact locale;
- invalidates loaded CAS state when group or locale changes;
- disables save until selected management view has loaded;
- shows missing translation as empty form with current CAS identity;
- sends selected locale only in typed management/CAS input;
- on `groups.application_policy_changed`, invalidates loaded view and requires explicit
  selected-locale reload;
- keeps append-only history manager-only.

The public `load_group_admin_application_policy` name remains as a compatibility
facade over management read. It returns a selected-path error when no policy or
translation exists and never falls back to another locale or transport.

The admin application workspace filters pending, approved, rejected, and cancelled
rows. Pending rows can be reviewed; rejected/cancelled rows expose manager reopen.
Reopen uses lifecycle port and preserves submitted snapshot.

Storefront uses `apply=<group_uuid>` to read current candidate status and load
host-resolved exact-locale policy when fresh form is allowed. Pending exposes cancel,
approved blocks duplicate submit, and rejected/cancelled exposes current-policy CAS
form. A stale conflict preserves route, disables repeated submission, and exposes
explicit reload that clears old answers and acknowledgements. Successful submit
clears `apply`; cancellation never clears it.

## Degraded modes

- Groups provider unavailable: deny private content.
- Candidate exact-locale policy unavailable: disable application form; never choose
  another locale.
- Management locale catalog unavailable: disable locale selection/save; never infer
  locales from history or host locale.
- Selected management translation missing: expose explicit empty form with current CAS
  identity; never substitute another translation.
- Policy CAS conflict: write no owner state, preserve selected transport error, and
  require explicit selected-locale reload before retry.
- Current-application lifecycle read unavailable: do not guess status or expose
  submit/cancel controls.
- Lifecycle command transport failure: preserve selected-path error and `apply`; never
  retry through another transport.
- Policy history unavailable: current owner policy remains authoritative; hide history
  rather than synthesizing revisions.
- Native or GraphQL transport failure: surface selected-path error; never retry through
  another transport.
- Profiles unavailable: display stable UUID/placeholder, never copy canonical profile
  state.
- Notifications unavailable: owner commands commit and remain authoritative.
- Search/index unavailable: owner writes commit; future projections may catch up.

## Open gates

The following remain source or evidence work:

- remove or version-deprecate legacy unconditional application command methods;
- bounded bulk review and per-item audit/result handling;
- ProfilesReader summaries and application semantic events;
- locale translation deletion/lifecycle policy if required by product behavior;
- migration execution/backfill/immutability evidence;
- native/GraphQL locale-catalog/management/result parity, replay, stale/locale/lifecycle
  races, concurrency, lock ordering, accessibility, security, retry, and recovery
  evidence.

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
node scripts/verify/verify-groups-application-lifecycle.mjs
node scripts/verify/verify-groups-application-policy-locales.mjs
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

No build, test, migration, verifier, parity, replay, stale/locale/lifecycle-race,
concurrency, accessibility, security, retry, or recovery command was executed for
this source slice. FFA, FBA, GROUPS-06, and GROUPS-19 remain `in_progress`; policy
locale-management runtime evidence remains `null`.

## Related documents

- [Canonical implementation plan](implementation-plan.md)
- [Root module README](../README.md)
- [FBA registry](../contracts/groups-fba-registry.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)
- [Database contract](../../../docs/architecture/database.md)
- [FFA package architecture](../../../docs/UI/module-package-architecture.md)
- [FBA architecture](../../../docs/backend/module-backend-architecture.md)
