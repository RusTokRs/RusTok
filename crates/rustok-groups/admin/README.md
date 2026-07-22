# rustok-groups-admin

Module-owned Leptos admin FFA package for Groups.

## Structure

- framework-neutral `core.rs` and `application_core.rs` validate inputs and prepare
  idempotent commands;
- `application_model.rs` carries policy preconditions, snapshots, review, reopen, and
  membership results;
- `transport.rs` is the only facade consumed by UI;
- native and GraphQL adapters cover directory, governance, localization, invitations,
  policy CAS/history, application list/review, and application reopen;
- `ui/policy_editor.rs` provides exact-locale CAS policy editing and immutable history;
- `ui/applications.rs` provides status filtering, snapshot review, and manager reopen;
- `locales/` contains EN/RU copy.

## Policy editor

The editor loads the host-resolved exact-locale policy, captures
`(policy_id, revision, locale)`, and saves through `GroupApplicationCasCommandPort`.
The owner compares the precondition after the group lock and before state writes.
`groups.application_policy_changed` keeps the stale identity and requires explicit
reload. Policy revision history remains append-only and manager-only.

## Application workspace

The workspace can filter `pending`, `approved`, `rejected`, and `cancelled` rows.
Each row displays the immutable policy snapshot, answers, and acknowledged rules.

- review remains available for owner-authorized pending applications;
- reopen controls render only for `rejected` or `cancelled` rows;
- reopen uses `GroupApplicationLifecycleCommandPort`, not direct table updates;
- the owner requires a `left`, non-banned, non-active membership and an active
  non-secret `request` group;
- reopen restores application/membership to `pending`, clears review metadata, and
  preserves policy identity, snapshot, answers, acknowledgements, and submitted time;
- application, membership, group version, audit, and receipt remain transactional.

Native and GraphQL paths call the same owner port through
`execute_selected_transport`. An owner denial, conflict, timeout, or unavailable result
never triggers fallback.

## Compatibility and open gates

Legacy unconditional policy-save and candidate-submit Rust methods remain backend
source compatibility only; this package does not call them. Manual UUID entry,
multi-locale selection, profile-backed pickers, bulk review, confirmation,
audit/receipt history, accessibility execution, and native/GraphQL parity remain open.

No source artifact promotes readiness without executed build, runtime, migration,
replay, CAS/lifecycle race, concurrency, lock-order, security, accessibility, and
recovery evidence.
