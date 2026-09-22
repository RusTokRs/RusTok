# FORUM-34J bounded export page composer actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor and recheck

FORUM-34A through FORUM-34I are merged before this slice. Fresh `main` for 34J is `a1ceae709b6dfd359b1ba6053f49b3dffdb926bd`, the 34I merge commit; no later Forum import/export commit was present when this slice started.

The canonical Forum implementation-plan ledger still labels `FORUM-34` as `planned`. This dated packet records the truthful 34J cursor without replacing the large concurrently edited roadmap from a partial whole-file image.

## Purpose

34I can enumerate one bounded live page of candidate source IDs. 34H expands explicit source IDs into exact stored-locale read targets. 34F performs exact owner reads and 34D maps those already-authorized owner views into `rustok.forum.export.v1`.

34J adds the missing read-only composition boundary for one page. It does not add another storage path or duplicate any mapping logic.

## Public in-process contract

`ForumExportPageComposer::compose_page(...)` accepts the existing:

- `ForumExportSourceInventoryService`;
- category/topic/reply owner services;
- `SecurityContext`;
- `ForumExportSourceInventoryRequest`.

It returns `ForumExportPage` containing the exact 34I `source` page plus `fragment: Option<ForumExportFragment>`.

For a non-empty source page the only data path is:

`34I list_page -> 34H target_plan_request/plan_fragment -> 34F read_fragment -> 34D mapping`.

For an empty terminal source page, `fragment` is `None`. The composer does not manufacture an empty 34H request that would violate 34H's `EmptySources` contract. An impossible empty page with `has_more=true` fails closed.

The page wrapper and composer contract are non-wire and add no serde/transport contract.

## Authorization

34J does not weaken or reinterpret authorization. The first stage is still 34I, which requires a non-public operator context and `PermissionScope::All` for the requested resource kind's exact `Action::Manage`. 34H and 34F then repeat their existing requested-kind manage admission.

The composer adds no bypass around those owner boundaries.

## Bounded behavior

34I still limits one source page to at most 512 IDs. 34H/34F still limit the expanded localized target batch to at most 512 targets.

A page with many locales can therefore be valid at the source-ID layer but exceed the localized-target cap. 34J deliberately propagates `ForumExportTargetPlanError::TooManyTargets`; it does not truncate locales, split one source page invisibly, or advance the source cursor after a failed plan. A caller may retry the same `after_id` with a smaller source limit.

## Composition integrity checks

After 34F/34D returns a fragment, 34J validates:

- fragment tenant ID equals the 34I page tenant;
- the fragment contains records only for the requested resource kind;
- the ordered unique source IDs in the fragment equal the ordered 34I source IDs.

These checks protect the composition boundary from accidental cross-kind contamination, source identity substitution, or ordering/cardinality drift. They do not replace the stricter exact-locale and exact-owner checks already enforced by 34H/34F.

## Live-read consistency boundary

34J does **not** claim a frozen database snapshot, even within one composed page. Inventory, locale enumeration and exact owner reads remain separate owner calls. Concurrent archive/restore/delete/merge/content changes can therefore race the composition pipeline.

Some races fail closed through existing not-found/canonical/exact-locale checks; other live-state transitions can produce a fragment reflecting a later state than the inventory query. A neutral shared migration runner must own quiescence or snapshot semantics, durable checkpoints, retry identity and receipts before FORUM-34 can claim a complete resumable tenant export.

The returned source cursor is still only the 34I live keyset position, not durable job state.

## Storage and transport boundaries

`export_page.rs` imports no SeaORM/database types and issues no storage query itself. It owns orchestration only. It does not call `ForumOwnerExportMapper` directly; mapping remains inside 34F's established reader path.

34J adds no GraphQL/REST/CLI endpoint, schema migration, background job, checkpoint table, audit/receipt persistence or shared runner implementation.

## Current source-ready export chain

The bounded current-state export path is now composed end-to-end for one resource-kind page:

`34J page composer -> 34I bounded candidate IDs -> 34H exact-locale targets -> 34F exact owner reads -> 34D export mapping`.

Cross-page completeness, frozen snapshot semantics and durable resume remain intentionally open. Import persistence, external-user identity resolution, history/revisions, votes/reputation, attachments, merge/route history and Search rebuild orchestration also remain open.

## Next FORUM-34 cursor

With the bounded current-state export page now composed, the next safe Forum-owned slice should move back to the import side: define a bounded resolved import-application boundary that consumes already-inspected dependencies/identities without inventing the absent durable shared runner. External-user resolution must remain explicit rather than fabricating RusTok user IDs.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-export-page-composer-source.mjs
```
