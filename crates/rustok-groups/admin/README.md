# rustok-groups-admin

Module-owned Leptos admin FFA package for Groups.

## Structure

- `core.rs`: framework-neutral UUID/locale/text/invitation validation, command
  preparation, and transport profile;
- `application_core.rs`: framework-neutral policy locale-catalog/query, CAS
  precondition, question/rule, review, reopen, and idempotency-key preparation;
- `model.rs`: directory, governance, localization, and invitation models;
- `application_model.rs`: policy locale catalog, management view, CAS precondition,
  revision history, snapshot, review, reopen, and membership models;
- `policy_locale_compat.rs`: compatibility facade for the former single-policy read;
- `transport.rs`: the only selected transport facade consumed by UI;
- `transport/native_server_adapter.rs`: native directory/governance server functions;
- `transport/native_localization_adapter.rs`: exact-locale localization server
  functions;
- `transport/native_invitations_adapter.rs`: invitation management server functions;
- `transport/native_applications_adapter.rs`: application list/review server functions;
- `transport/native_application_lifecycle_adapter.rs`: manager reopen server function;
- `transport/native_policy_locale_adapter.rs`: locale catalog, explicit selected-locale
  management read, and atomic CAS save server functions;
- `transport/native_policy_history_adapter.rs`: manager-only revision-history server
  function;
- `transport/graphql_adapter.rs`: directory/governance/localization GraphQL paths;
- `transport/graphql_invitations_adapter.rs`: invitation GraphQL paths;
- `transport/graphql_applications_adapter.rs`: application list/review GraphQL paths;
- `transport/graphql_application_lifecycle_adapter.rs`: manager reopen GraphQL path;
- `transport/graphql_policy_locale_adapter.rs`: locale catalog, selected-locale
  management read, and CAS save GraphQL paths;
- `transport/graphql_policy_history_adapter.rs`: policy-history GraphQL path;
- `ui/leptos.rs`: directory and governance binding;
- `ui/localization.rs`: exact-locale group presentation workspace;
- `ui/policy_editor.rs`: multi-locale visual policy editor, atomic stale protection,
  and revision history;
- `ui/applications.rs`: status-filtered application snapshot, review, and reopen
  workspace;
- `ui/invitations.rs`: targeted/shareable invitation management;
- `ui/root.rs`: module-owned composition root;
- `locales/`: English and Russian copy.

## Membership policy editor

The visual editor supports:

- loading an authorized catalog of existing exact-locale policy translations;
- selecting an existing locale or entering a new normalized locale tag through a
  datalist;
- loading the selected locale through
  `GroupApplicationPolicyManagementReadPort`, independently of the host UI locale;
- displaying an explicit empty form when the selected translation does not exist;
- enabling/disabling applications;
- adding, removing, and reordering up to 20 questions and 20 rules;
- editing stable keys, prompt/help copy, required flags, answer limits, titles, and
  bodies;
- capturing current policy ID/revision plus the selected exact locale;
- saving through `GroupApplicationCasCommandPort` in native or GraphQL mode;
- listing append-only policy revisions through native or GraphQL transport;
- displaying revision, locale, actor, timestamp, enabled state, and item counts;
- invalidating loaded CAS state whenever group or selected locale changes;
- disabling save until the selected management view has loaded;
- requiring explicit selected-locale reload after a stale conflict.

`PortContext.locale` remains the host request/UI locale. The locale selected in the
editor is carried only in the typed management request or CAS input. Native adapters
do not replace request locale with the selected policy locale, and GraphQL adapters do
not place the selected policy locale into the locale header.

When no policy exists, the owner returns an empty management view without a
precondition. When a policy exists but the selected translation does not, the owner
returns an empty view with current policy ID/revision and the selected locale. Saving
that form creates the translation only if the current policy CAS precondition still
matches.

Policy saves send the loaded policy identity directly to the owner transaction. The
owner locks the group row and compares `(policy_id, revision, locale)` before any
policy, version, audit, or receipt write. A mismatch returns
`groups.application_policy_changed`. The editor invalidates the loaded view and
requires `Load selected locale` before another save.

An identical committed idempotent command is replayed before its precondition is
checked again. Later policy revisions therefore do not invalidate recovery of an
already-committed response.

Every successful policy translation INSERT/UPDATE is captured into
`group_membership_policy_revisions` in the same database transaction. Revision rows
are append-only, and history listing remains manager-only.

The public `load_group_admin_application_policy` name remains as a compatibility
facade over the new management read. It returns an explicit selected-transport error
when no policy or selected translation exists and never falls back to another locale
or transport.

## Application review and reopen workspace

The application facade lists immutable policy snapshots, candidate answers, and rule
acknowledgements. Operators can filter pending, approved, rejected, and cancelled
applications.

Pending rows continue through the approve/reject owner review service. Rejected and
cancelled rows expose `Reopen`, prepared in the framework-neutral core and executed
through `GroupApplicationLifecycleCommandPort`.

The owner locks application then group, verifies active owner/admin/moderator or
platform authority before disclosing reopen state, and accepts only rejected or
cancelled applications with a left, non-banned, non-active membership. Reopen restores
application and membership to pending, clears prior review metadata, and preserves
submitted time, policy identity/revision/locale, policy snapshot, answers, and
acknowledgements. Group version, audit, and idempotency receipt commit with owner
state.

A manager reopen is not a candidate resubmit. Reopen keeps the submitted snapshot for
later review; a fresh candidate resubmit instead uses current-policy CAS and replaces
the snapshot only after success.

## Other admin surfaces

The localization facade never selects fallback locale rows. The invitation facade
never stores or recovers invitation plaintext after the first create response. The
governance facade never copies local-role or ownership rules into UI.

All facades choose exactly one transport through `execute_selected_transport`; an
owner denial, timeout, conflict, or unavailable result never triggers implicit retry
through another path.

## Compatibility and open gates

The older unconditional policy-save and candidate-submit methods remain in the
backend command port for source compatibility, but this admin package does not call
them. Their removal or versioned deprecation is a separate API migration gate.

Manual group/member/application/invitation UUID entry remains an intermediate
operator surface. Profile-backed pickers, locale-translation deletion policy,
explicit destructive confirmation, bulk review, audit/receipt history, accessibility
execution, and native/GraphQL parity remain open.

No source artifact in this package promotes FFA readiness without executed build,
runtime, migration, replay, stale/locale/lifecycle race, concurrency, lock-order,
security, accessibility, and recovery evidence.
