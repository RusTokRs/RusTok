# rustok-groups-admin

Module-owned Leptos admin FFA package for Groups.

## Structure

- `core.rs`: framework-neutral UUID/locale/text/invitation validation, command
  preparation, and transport profile;
- `application_core.rs`: framework-neutral policy/review validation and fresh
  idempotency-key preparation;
- `model.rs`: directory, governance, localization, and invitation models;
- `application_model.rs`: application policy, revision history, snapshot, review, and
  membership models;
- `transport.rs`: the only selected transport facade consumed by UI;
- `transport/native_server_adapter.rs`: native directory/governance server functions;
- `transport/native_localization_adapter.rs`: exact-locale localization server
  functions;
- `transport/native_invitations_adapter.rs`: invitation management server functions;
- `transport/native_applications_adapter.rs`: policy/list/review server functions;
- `transport/native_policy_history_adapter.rs`: manager-only revision-history server
  function;
- `transport/graphql_adapter.rs`: directory/governance/localization GraphQL paths;
- `transport/graphql_invitations_adapter.rs`: invitation GraphQL paths;
- `transport/graphql_applications_adapter.rs`: policy/list/review GraphQL paths;
- `transport/graphql_policy_history_adapter.rs`: policy-history GraphQL path;
- `ui/leptos.rs`: directory and governance binding;
- `ui/localization.rs`: exact-locale group presentation workspace;
- `ui/policy_editor.rs`: visual membership policy editor and revision history;
- `ui/applications.rs`: pending application snapshot/review workspace;
- `ui/invitations.rs`: targeted/shareable invitation management;
- `ui/root.rs`: module-owned composition root;
- `locales/`: English and Russian copy.

## Membership policy editor

The visual editor supports:

- loading the current owner policy for the host-resolved effective locale;
- enabling/disabling applications;
- adding, removing, and reordering up to 20 questions and 20 rules;
- editing stable keys, prompt/help copy, required flags, answer limits, titles, and
  bodies;
- saving through the existing idempotent owner command;
- listing append-only policy revisions through native or GraphQL transport;
- displaying revision, locale, actor, timestamp, enabled state, and item counts;
- blocking the UI save when a reread observes a different revision.

The locale field is read-only because the owner read contract consumes
`PortContext.locale`. A multi-locale picker must be added only with an explicit
selected-locale read contract; the UI must not pretend that changing a text field
changes owner selection policy.

The revision reread is a **non-atomic stale preflight**. It reduces accidental
operator overwrites but does not close the race between reread and write. Atomic
expected-revision enforcement inside the owner transaction remains planned.

Every successful policy translation INSERT/UPDATE is captured into
`group_membership_policy_revisions` in the same database transaction. Revision rows
are append-only, and history listing reuses the application-review authorization
boundary.

## Other admin surfaces

The application review facade lists policy snapshots, candidate answers, and rule
acknowledgements, then calls the same approve/reject owner service from native and
GraphQL paths. Approval/rejection, membership state, group version, audit, and
idempotency receipt remain owner-transactional.

The localization facade never selects fallback locale rows. The invitation facade
never stores or recovers invitation plaintext after the first create response. The
governance facade never copies local-role or ownership rules into UI.

All facades choose exactly one transport through `execute_selected_transport`; an
owner denial, timeout, conflict, or unavailable result never triggers implicit retry
through another path.

## Open gates

Manual group/member/application/invitation UUID entry remains an intermediate
operator surface. Multi-locale policy selection, atomic expected-revision, pickers,
explicit destructive confirmation, bulk review, audit/receipt history,
accessibility execution, and native/GraphQL parity remain open.

No source artifact in this package promotes FFA readiness without executed build,
runtime, migration, replay, concurrency, security, accessibility, and recovery
evidence.
