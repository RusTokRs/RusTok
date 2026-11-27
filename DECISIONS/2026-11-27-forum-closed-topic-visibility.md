# Forum closed-topic visibility and lifecycle semantics

- Date: 2026-11-27
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `rustok-forum`
- Extends: None
- Supersedes: None
- Superseded by: None

Related: `forum-logic-review-2026-11-27.md` (finding G1)

## Context

A logic review surfaced an apparent inconsistency in how `Closed` topics behave
across read, list, and write surfaces. This document records the intended
semantics so the behavior is documented rather than treated as an accidental
gap.

## Decision

`TopicStatus::Closed` is a **publication-state** value, distinct from the
soft-delete tombstone (`deleted_at IS NOT NULL`, which also drives `status =
Archived`). The two are orthogonal:

| Surface | `Open` | `Closed` | `Archived` (deleted) |
|---|---|---|---|
| Storefront topic list | visible | hidden | hidden |
| Storefront topic detail | visible | hidden (404) | hidden (404) |
| Owner topic list | visible | visible (owner only) | hidden |
| Owner topic detail | visible | visible | visible (restore target) |
| Votes / edits by owner | allowed | allowed | blocked |
| New replies / reply restores | allowed | blocked | blocked |

Rationale:

- The storefront only lists `Status == Open` topics
  (`topic.rs` list storefront path, `topic_visibility.rs`,
  `read_model_owner.rs`), so `Closed` topics are intentionally not surfaced to
  anonymous or storefront readers.
- The owner list (`topic_owner_read_condition`,
  `read_model_owner.rs`) and owner detail read intentionally surface `Closed`
  topics to their author, so an author can still see and manage a topic they
  closed.
- `enforce_topic_engagement` (`topic_engagement_guard.rs`) permits votes,
  edits, and reply updates on `Closed` topics, but blocks `CreateReply` and
  `RestoreReply`, matching the documented "closed ⇒ no new replies" contract.

## Consequences

Because the storefront never displays `Closed` topics, public aggregates such as
`category.topic_count` should exclude `Closed` (and `Archived`) topics. Imported
topics finalized as `Closed` must therefore not keep the `+1` granted when the
temporary row was inserted as `Open`. (See `topic.rs::finalize_import_topic_in_tx`
and the review finding E1.)

## Open product decisions (not changed here)

- Whether a `Closed` topic's `category.topic_count` contribution should be
  excluded is a product choice; this document records the current consistent
  implementation (exclude `Closed`/`Archived` from public aggregates).
- Author self-close/self-lock remains explicitly out of scope per
  `2026-11-02-forum-remove-unwired-closing-and-digest-settings.md`.

## Sources of truth and ownership

`rustok-forum` owns topic status, tombstones and engagement admission. Storefront
lists and category counters are derived views, not alternative lifecycle owners.

## Invariants

Closed is distinct from deleted. Storefront reads exclude Closed; author reads
retain it. New replies and reply restores reject Closed parents.

## Non-goals

No new status, self-close command or general counter redesign is introduced.

## Data, transaction, and concurrency boundary

This documents existing lifecycle boundaries; it adds no schema or lock.
Import finalization reconciles the temporary Open contribution in its existing
transaction. Existing engagement guards retain responsibility for write checks.

## Context dimensions

Tenant and principal determine owner access; storefront audience gates remain
mandatory. Locale affects content rendering, not status. Channel visibility is
an additional independent gate. Currency/timezone are not applicable.

## Events and projections

No event contract changes. Existing lifecycle transactions own counter updates;
category counts must not retain a temporary Open import contribution when the
final state is Closed or Archived. Historical counter repair is not added here.

## Failure semantics

Storefront reads hide non-public topics; engagement guards return the existing
typed closed/archived errors. This decision adds no degraded-access fallback.

## Migration and cutover

No schema migration or status reinterpretation. Existing rows retain their
status. The import counter correction applies at finalization, not retroactively.

## Alternatives considered

Treating Closed as publicly readable but reply-disabled would change the current
storefront contract. Treating it as a tombstone would break owner management.

## Verification

Source inspection covered visibility and engagement guards and import counter
finalization. Node/Python source checks ran; Rust compilation and database-backed
lifecycle checks remain unexecuted because the toolchain is absent.
