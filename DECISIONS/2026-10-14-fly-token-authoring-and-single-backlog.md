# Fly token authoring and the single owned backlog (Phase 6)

- Date: 2026-10-14
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `fly`, `fly-ui`, `rustok-page-builder-admin`
- Extends: 2026-10-14-fly-design-tokens-animations-and-admin-convergence.md
- Supersedes: None
- Superseded by: None

## Context

Phase 3 landed design tokens as validated project data with rendering, but no
authoring path: adapters displayed them read-only and hosts had to poke the
`flyTheme` extension directly, bypassing validation and history. Separately, the
audit's documentation finding (P3-19) recorded three parallel plans whose open
checkboxes could not distinguish release blockers from "someday".

## Decision

1. **Token mutation is an editor command.** `EditorCommand::Theme` carries a
   `ThemeCommand` (replace/upsert/remove). Applying it goes through the editor's
   speculative-clone transaction, full validation (which already gates on
   `theme_token_invalid`), and undo history, like every other edit. Removing the
   last token removes the `flyTheme` extension entirely; removing a missing token
   is an error. The capability classifier requires `Styles` for theme commands.
   Renaming a token deliberately does *not* rewrite `var(--name)` references in
   declarations (recorded in the Fly README): usage-aware rename is a separate,
   future authoring feature, not a silent side effect.
2. **One owned backlog.** `docs/modules/page-builder-backlog.md` is the single
   forward-looking Fly/Page Builder plan, ordered by priority with owners. The
   implementation plan and the parity actualization notes are marked historical
   (they keep the record; they no longer collect open items), and `docs/index.md`
   points at the backlog.

## Sources of truth and ownership

- Canonical theme token command definition: `fly::ThemeCommand`.
- Canonical capability gating: `fly_ui::CommandCapabilityRequirement`.
- Canonical Fly/Page Builder forward backlog: `docs/modules/page-builder-backlog.md`.

## Invariants

- All token mutations participate in `FlyEditor` history and validation invariants.
- Token removal is explicit; removing non-existent tokens yields an actionable error.
- Backlog items maintain strict owner assignment and verification evidence.

## Non-goals

- Automatic recursive refactoring of CSS variable usages on token rename.
- Multiple uncoordinated backlogs across Fly and Page Builder.

## Data, transaction, and concurrency boundary

- Theme tokens commit inside the document's `flyTheme` extension through atomic editor commands.
- Lost-update prevention relies on document revision tracking.

## Context dimensions

- Theme tokens apply globally to the document layout and are resolved per active style scope.

## Events and projections

- Theme token mutations trigger editor canvas re-renders and reactive preview updates.

## Failure semantics

- Invalid token names or values reject before applying to the active document and emit error diagnostics.

## Migration and cutover

- Pre-existing documents lacking tokens load with empty defaults.
- Deprecated parallel plans are linked to the unified backlog.

## Alternatives considered

- Direct document extension mutation: rejected for bypassing undo/redo history and capability gating.
- Maintaining separate plans per adapter: rejected to prevent architectural drift.

## Verification

Unit tests for `ThemeCommand` execution, undo/redo cycle validation, and capability gate enforcement.

## Consequences

- Design tokens become first-class editable assets across Fly authoring surfaces.
- Backlog governance is unified under a single living owner contract.
