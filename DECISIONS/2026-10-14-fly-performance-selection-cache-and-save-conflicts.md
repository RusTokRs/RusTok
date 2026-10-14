# Fly command-path performance and save-conflict state (Phase 4)

Date: 2026-10-14
Status: accepted
Scope: `fly` (editor command path, id stabilization, revision state), `fly-ui`
(UI state machine), `rustok-page-builder-admin` (controller, toolbar).

## Context

The audit's performance finding (P1-6) measured conceptually what the code did
literally: every editor command paid a full deep clone, a full id-stabilization pass
with per-id string clones, a full-document validation, and a full serialization
hash. Selection — the most frequent interaction in an editor — paid the validation
too. Separately (P1-7), a save rejected because another session saved first was
indistinguishable from any other save failure, so a user could keep editing a
diverged document without ever being told their work was not saved.

## Decisions

1. **Id stabilization gets a borrow-only fast path.** `ProjectDocument::
   ensure_stable_ids` first checks, in one immutable walk with a borrowed-id set,
   whether every component id is present, valid, and unique; only then does it skip
   the mutating repair walk and its per-id string clones. Documents that need repair
   take the existing algorithm unchanged. This preserves id semantics bit-for-bit
   while removing one full walk and all id allocations from steady-state commands.
2. **Selection returns the cached validation report.** Selection cannot change
   document validity, so `FlyEditor` caches the last full report (not serialized;
   `#[serde(skip)]`) and `Select` returns a clone of it. Mutating commands refresh
   the cache with their own report; undo, redo, registry mutation, and validation
   limit changes invalidate it, and the next selection revalidates on demand. The
   deep speculative clone per mutating command stays: it is what makes commands
   transactional, and history already shares the before/after allocations.
3. **Lost updates are a first-class state, not a failure flavour.** `RevisionState`
   and the `fly-ui` `DirtyState` gain `save_conflict`; `UiIntent::SaveConflict`
   keeps `dirty` set (the work is *not* saved), stops the in-progress flag, and
   announces that the page changed in another session and must be reloaded before
   continuing. Only a successful save clears it. In the page-builder admin, a
   `FlyError::RevisionConflict` at acknowledge time is routed to
   `mark_save_conflict` instead of the generic failure path, and the toolbar shows
   a dedicated conflict status (localized) ahead of the generic "save failed" one.
   Presence, locking, and merge remain out of scope, exactly as the audit's minimum
   prescribes.

## Verification

No Rust toolchain exists in this sandbox, so `cargo test`/`clippy` were not run.
What was run: `rust_syntax_check.mjs` over all changed Rust files (the one
tree-sitter complaint in `style_rule.rs` is a known grammar false positive — real
rustfmt parses the file), `rustfmt_check.mjs` (all conform), `node --check` plus a
token simulation of the JS contract assertions, and `scripts/fly-check.sh gates`
(20 passed, 0 failed). This phase also included a full re-verification of
Phases 1–3, which caught and fixed two real defects: action targets were not
addressable in published renders (the renderer now emits a plain `id` for targeted
components and the runtime falls back to `data-fly-component-id`), and the Leptos
workspace referenced a non-existent `Presentation::FullEditor` variant. The
benchmarks in `crates/ui/fly/benches` remain the gate for the numbers; maintainers
must run them plus `cargo test` before calling any of this green.
