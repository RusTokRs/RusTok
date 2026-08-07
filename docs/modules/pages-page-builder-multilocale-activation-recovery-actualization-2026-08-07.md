# Pages / Page Builder Multi-Locale Artifact-Loss Activation Recovery Actualization

Date: 2026-08-07  
Status: current-source-overlay / multilocale-missing-binding-recovery-source-ready / execution-open

This overlay continues `pages-page-builder-repair-rollback-continuity-actualization-2026-08-07.md` after PR #3173 restored rollback continuity for an explicitly repaired current artifact set.

## Main recheck

The source-ready Pages/Page Builder chain remains:

- registered metadata contribution with the bespoke metadata form removed;
- reviewed static Page Builder publish with immutable artifacts, manifests and retained rebuild provenance;
- immutable rollback with strict historical target authority;
- bounded immutable artifact audit;
- explicit append-only artifact rebuild;
- strict existing-binding activation;
- missing-binding activation after physical source-artifact loss;
- repair-to-rollback current-cursor continuity with original historical target manifests still mandatory;
- event-driven cache invalidation and immutable public readers;
- authenticated authoring, asset delivery, same-origin launch and release composition.

Execution evidence remains pending where previously recorded. No source-complete item above is reopened by this overlay.

## Newly confirmed multi-locale gap

The physical-loss activation path merged in PR #3167 correctly fenced one missing locale with:

```text
source publish result_version == current expected_version
```

Every successful activation also advances `pages.version` exactly once.

For a publish containing multiple locales, if two source artifacts are physically lost, the first locale can be rebuilt and activated at the source publish version. The page then advances by one version. The second lost locale still belongs to the exact same reviewed publish, but the old equality fence sees the first activation's legitimate version increment as stale drift and rejects it.

That made the single-locale recovery source-ready while leaving multi-locale physical loss incomplete.

## Source change

Missing-binding recovery now has two version admissions after all existing source-artifact/body/publish fences pass:

1. **first recovery:** `publish.result_version == expected_version`;
2. **sequential recovery:** `publish.result_version < expected_version` only when the entire version gap is a bounded, contiguous same-publish activation chain for other locales.

No future publish version is accepted.

### Exact same-publish activation chain

The sequential branch requires:

- at most 256 intervening activation receipts;
- exact receipt count equal to the version gap;
- ascending contiguous `expected_version -> result_version` steps with no missing version;
- unique prior locales and no prior activation for the locale currently being recovered;
- recomputed canonical activation request hashes;
- revalidated rebuild receipts and retained provenance for every prior step;
- every prior rebuild/provenance source to reference the exact same source publish operation;
- exact activation body, locale, source artifact, replacement artifact and replacement hash identity;
- every prior repaired locale binding to remain active on that rebuilt artifact;
- every prior rebuilt artifact to retain its receipt-bound instance key, artifact hash and materialization hash.

A version increment without one exact prior activation receipt therefore remains a conflict. Unpublish/publish, rollback, metadata/lifecycle drift, direct fixture mutation or a repair from a different publish cannot bridge the gap.

## Preserved boundaries

This slice does not:

- change the activation DTO or receipt schema;
- add a migration; the existing `(tenant_id, page_id, result_version)` receipt index is reused;
- batch multiple locales in one command;
- weaken the strict existing-binding path;
- allow a source artifact that still exists into missing-binding recovery;
- use mutable current draft content as repair authority;
- recreate source artifacts or publish manifest rows;
- combine rebuild and activation;
- change GraphQL, HTTP, OpenAPI or admin transport;
- mutate caches inline;
- schedule repair automatically;
- promote FFA/FBA.

## PostgreSQL source packet

New source packet:

```text
crates/rustok-pages/tests/artifact_loss_multilocale_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-multilocale-activation-recovery-postgres.mjs
```

It retains:

1. two physically lost locales from one publish -> explicit rebuild for each -> first activation at publish version -> second activation at first activation result version -> both rebuilt locale bindings active;
2. first locale activation -> unexplained page-version increment -> second locale activation rejected with no second receipt/event/binding mutation.

The original single-locale PostgreSQL packet remains unchanged in behavioral intent and still proves direct stale drift is rejected.

## Actualized parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Reviewed publish + retained rebuild provenance | Source-ready | PostgreSQL/runtime evidence pending |
| Explicit immutable rebuild | Source-ready | SQLite/PostgreSQL evidence pending |
| Existing-binding activation | Source-ready | Execution pending |
| Single-locale physical-loss missing-binding activation | Source-ready | PostgreSQL execution pending |
| Multi-locale sequential physical-loss activation | **Source-ready in this overlay** | PostgreSQL execution pending |
| Unexplained version drift between locale recoveries | **Rejected by source contract** | PostgreSQL negative execution pending |
| Repair -> rollback current-cursor continuity | Source-ready | PostgreSQL execution pending |
| Historical rollback target without original manifest | Rejected | PostgreSQL negative execution pending |
| Automatic rebuild/activation/rollback chaining | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Plan parity correction

The Pages-local implementation plan and Page Builder FBA registry are actualized in this slice so the canonical planning surfaces no longer stop at inline-authoring or single-locale repair. The plan now records the artifact audit -> provenance -> rebuild -> activation -> multi-locale recovery -> rollback-continuity chain as source-ready, while keeping all execution claims open.

## Next cursor

Because maintainer execution is intentionally external to this authoring workflow, the next cursor remains evidence-first:

1. run the single-locale and new multi-locale activation PostgreSQL packets;
2. retain successful two-locale sequential recovery and unexplained-version-drift rejection;
3. run repair-to-rollback continuity after a multi-locale recovered set and retain the current-cursor/historical-target boundaries;
4. run previously source-ready provenance, rebuild, audit, repair failure/atomicity/cache and transport packets;
5. retain metadata conflict/isolation, cache, artifact/HTTP/browser and tenant Wave evidence;
6. promote no FFA/FBA state until the accepted execution chain is complete.

## Validation boundary

Source review and authoring only. No tests, Node verifiers, Cargo commands, formatting, PostgreSQL scenarios, workflows or CI were executed.
