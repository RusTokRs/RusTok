# rustok-groups-storefront

Module-owned Leptos storefront FFA for public group discovery, authenticated membership
applications, the group shell, and invitation acceptance.

The package uses framework-neutral cores, an explicit native/GraphQL facade,
host-provided exact locale/auth context, and thin Leptos bindings. Secret groups are
not listed publicly. Provider feature screens remain host-composed.

## Membership applications

Request-policy groups expose `apply=<group_uuid>`. Before rendering submit controls,
the storefront reads the authenticated candidate's current application through
`GroupApplicationLifecycleReadPort`.

- no current application: render the current exact-locale CAS form;
- `pending`: show status and permit candidate cancellation; hide duplicate submit;
- `approved`: show approved status and block duplicate submit;
- `rejected` or `cancelled`: show prior status and render a fresh current-policy CAS
  form.

Candidate cancellation calls
`GroupApplicationLifecycleCommandPort::cancel_group_membership_application` through
the selected transport. The owner accepts only the exact candidate's pending
application, moves membership to `left`, marks the application `cancelled`, preserves
the submitted snapshot, and commits group version, audit, and receipt atomically.

Cancellation does not clear `apply`; current state is reloaded and a fresh application
can be submitted immediately. Fresh resubmit is separate from reopen: it carries the
currently rendered `(policy_id, revision, locale)` through
`GroupApplicationCasCommandPort` and replaces the old snapshot only after success.
Successful submit clears `apply`.

Stale submit remains a distinct recovery state: route and old answers remain until the
candidate explicitly reloads; reload clears answers/acknowledgements and fetches the
current exact-locale policy. `groups.application_policy_changed` writes no stale owner
state.

Native and GraphQL lifecycle/CAS adapters call the same owner ports through
`execute_selected_transport`. They never fall back to one another. The older
unconditional candidate-submit Rust method remains backend compatibility only and is
not called here.

## Invitations

Invitation acceptance remains explicit:

- `invite=<opaque>` uses token acceptance;
- `invitation=<uuid>` uses authenticated exact-recipient targeted acceptance.

The UI never renders a plaintext token after acceptance and never owns notification
recipient resolution, inbox, preference, email, push, retry, or delivery state.

## Readiness

Build, runtime, GraphQL schema, native/GraphQL parity, idempotent replay, cancel/review/
resubmit races, lock ordering, accessibility, security, retry, and recovery evidence
remain unexecuted. Storefront FFA and GROUPS-06 remain `in_progress`.
