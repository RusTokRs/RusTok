# Fly command-path performance and save-conflict state (Phase 4)

- Date: 2026-10-14
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `fly`, `fly-ui`, `rustok-page-builder-admin`
- Extends: [Fly design tokens, animations, and admin convergence](./2026-10-14-fly-design-tokens-animations-and-admin-convergence.md)
- Supersedes: None
- Superseded by: None

## Context

The audit's performance finding (P1-6) measured conceptually what the code did
literally: every editor command paid a full deep clone, a full id-stabilization pass
with per-id string clones, a full-document validation, and a full serialization
hash. Selection — the most frequent interaction in an editor — paid the validation
too. Separately (P1-7), a save rejected because another session saved first was
indistinguishable from any other save failure, so a user could keep editing a
diverged document without ever being told their work was not saved.

## Decision

1. **Id stabilization gets a borrow-only fast path.** `ProjectDocument::ensure_stable_ids`
   first checks, in one immutable walk with a borrowed-id set, whether every component id
   is present, valid, and unique; only then does it skip the mutating repair walk and its
   per-id string clones. Documents that need repair take the existing algorithm unchanged.
   This preserves id semantics bit-for-bit while removing one full walk and all id
   allocations from steady-state commands.
2. **Selection returns the cached validation report.** Selection cannot change document
   validity, so `FlyEditor` caches the last full report (not serialized; `#[serde(skip)]`)
   and `Select` returns a clone of it. Mutating commands refresh the cache with their own
   report; undo, redo, registry mutation, and validation limit changes invalidate it, and
   the next selection revalidates on demand. The deep speculative clone per mutating command
   stays: it is what makes commands transactional, and history already shares the
   before/after allocations.
3. **Lost updates are a first-class state, not a failure flavour.** `RevisionState` and
   the `fly-ui` `DirtyState` gain `save_conflict`; `UiIntent::SaveConflict` keeps `dirty`
   set (the work is *not* saved), stops the in-progress flag, and announces that the page
   changed in another session and must be reloaded before continuing. Only a successful
   save clears it. In the page-builder admin, a `FlyError::RevisionConflict` at acknowledge
   time is routed to `mark_save_conflict` instead of the generic failure path, and the
   toolbar shows a dedicated conflict status (localized) ahead of the generic "save failed"
   one. Presence, locking, and merge remain out of scope, exactly as the audit's minimum
   prescribes.

## Sources of truth and ownership

- Command execution pipeline, selection caching, and id stabilization: `crates/ui/fly`.
- Dirty state and save-conflict intent models: `crates/ui/fly/ui`.
- Save conflict resolution UI handling: `crates/modules/rustok-page-builder/admin`.

## Invariants

- Component ID stabilization preserves identical ID assignments and uniqueness semantics across mutations.
- Cached validation reports are immediately invalidated upon any document mutation, undo, redo, or schema change.
- A save conflict explicitly retains the local unsaved dirty state until explicitly resolved or reloaded.

## Non-goals

- Automatic 3-way document merging or operational transformation in this phase.
- Distributed advisory locking or real-time multi-cursor collaboration.

## Data, transaction, and concurrency boundary

- Document transactions remain atomic with speculative clone-and-commit rollback semantics.
- Revision tokens are checked on save commit to guarantee optimistic concurrency.

## Context dimensions

- Concurrency checks operate per tenant, site, and page document instance.

## Events and projections

- Conflict states trigger distinct UI notification projections and disable automatic background saves.

## Failure semantics

- Concurrent save collisions yield typed `RevisionConflict` errors rather than generic I/O failures.

## Migration and cutover

- Transparent to persisted documents; validation cache is skipped during serialization.

## Alternatives considered

- **Fine-grained diffing engine**: Rejected as premature optimization before measuring macro-allocations.
- **Ignoring selection validation**: Rejected because callers rely on validation reports for toolbar action readiness.

## Verification

- Crate tests in `fly` verifying id fast-path validation and report cache invalidation.
- Conflict state handling tests in `rustok-page-builder-admin`.

## Consequences

- Selection commands operate with zero allocations and minimal CPU overhead.
- Users receive clear, actionable notification when edits diverge from another author's work.
