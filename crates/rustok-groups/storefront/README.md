# rustok-groups-storefront

Module-owned Leptos storefront FFA package for public group discovery,
authenticated membership applications, the group shell, and invitation acceptance.

The package uses framework-neutral cores, an explicit native/GraphQL transport
facade, host-provided exact locale, host-provided auth session, and thin Leptos
adapters. Secret groups are never listed by the public transport. Provider-owned
feature screens are composed by the host and are not embedded in this package.

## Membership applications

Request-policy groups expose an `apply=<group_uuid>` route from their directory card.
Before exposing application controls, the storefront loads the authenticated
candidate's exact tenant/group application through
`GroupApplicationLifecycleReadPort::read_my_group_membership_application`.

Current status controls the surface:

- no current application: load the exact-locale policy and render a fresh CAS form;
- `pending`: render status and candidate cancellation; hide duplicate submit;
- `approved`: render approved status and block duplicate submit;
- `rejected` or `cancelled`: render prior status and a fresh current-policy CAS form.

`application_core` validates policy identity, required answers, answer bounds, rule
acknowledgements, candidate application IDs, and fresh idempotency keys before
transport. `ui/application.rs` calls only the selected facade.

The loaded policy ID, revision, and exact locale are captured in the submit command.
Native and GraphQL adapters call
`GroupApplicationCasCommandPort::submit_group_membership_application_if_current`.
For an existing rejected/cancelled application, the owner locks the application before
the group. For a first submission, it repeats the application lookup after the group
lock. The current policy identity is compared before membership, application, group
version, audit, or receipt state changes. A mismatch returns
`groups.application_policy_changed` and writes no stale application state.

The UI handles a stale policy as a distinct recovery state:

- the `apply` query key remains in the URL;
- repeated submit is disabled;
- old answers and acknowledgements remain visible until the candidate chooses reload;
- explicit reload clears those old values and loads the current exact-locale policy;
- the candidate must review and submit the new form.

Candidate cancellation is a separate lifecycle command. Native and GraphQL adapters
call `GroupApplicationLifecycleCommandPort::cancel_group_membership_application`.
The owner permits only the exact candidate's pending application, moves membership to
`left`, records `left_at`, marks the application `cancelled`, clears review metadata,
and preserves the submitted policy identity, snapshot, answers, and acknowledgements.
Application, membership, group version, audit, and receipt commit atomically.

Cancellation never clears `apply`. The current state is refetched and the candidate
may immediately prepare a fresh current-policy CAS submission. Fresh resubmit replaces
the old snapshot only after success. A manager reopen is different: it restores the
preserved submitted snapshot to pending review.

Successful fresh submission creates or updates pending membership/application,
increments group version, and commits the new immutable policy snapshot, audit, and
idempotency receipt in one transaction. The `apply` query key is removed only after
success. An identical already-committed idempotent command may replay before the
policy precondition is checked again.

The older unconditional candidate-submit backend method remains for source
compatibility, but this package does not call it. Native and GraphQL transports never
fall back to one another.

## Invitations

Invitation acceptance is prepared in `core`, executed through the selected transport
without implicit fallback, and bound in `ui/invitation_acceptance.rs`.

Two explicit flows are supported:

- `invite=<opaque>` or password-style manual input uses token acceptance for shareable
  and directly delivered tokens;
- `invitation=<uuid>` uses authenticated exact-recipient acceptance for a targeted
  invitation opened through an authorized Notifications source route.

The UI removes the active invitation query value when submission begins, preserves
in-memory input for a failed retry, clears it after success, and never renders a
plaintext token as result text. Native and GraphQL adapters call the same owner ports
for validation, target authorization, redemption, membership, group version, audit,
and receipt rules.

The package does not resolve notification recipients, publish notification events, or
own inbox, preference, email, push, retry, or delivery state. Those responsibilities
remain in the Groups backend source provider and the Notifications owner.

## Readiness

Build, runtime, native/GraphQL stable-code and lifecycle parity, idempotent replay,
stale-submit and cancel/review/resubmit races, lock ordering, accessibility, security,
retry, and recovery evidence remain unexecuted. Storefront FFA and GROUPS-06 remain
`in_progress`.
