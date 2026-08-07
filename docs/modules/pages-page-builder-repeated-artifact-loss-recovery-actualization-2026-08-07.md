# Pages / Page Builder Repeated Artifact-Loss Recovery Actualization

Date: 2026-08-07  
Status: `current-source-overlay / repeated-artifact-loss-recovery-source-ready / execution-open`

## Why this slice exists

The prior repair chain could recover a canonical publish artifact once, but it intentionally rejected any repeated locale in the bounded post-anchor activation chain. That became a functional continuity gap once rebuilt immutable instances themselves could be physically lost.

The durable authority already survives that loss:

- `page_publish_rebuild_sources` retain the exact reviewed publish provenance;
- `page_artifact_rebuild_operations` retain append-only rebuild receipts without an artifact-row foreign key;
- `page_artifact_binding_replacement_operations` retain activation receipts without an artifact-row foreign key.

Therefore a sequence such as:

```text
publish source A
-> physical loss A
-> rebuild R1
-> activate R1
-> physical loss R1
-> rebuild R2 from the same retained source
-> activate R2
```

can be proven without reading mutable draft content and without introducing a new storage authority.

## Current source contract

`PageService::replace_rebuilt_artifact_binding` keeps the existing command surface and request identity. In particular, `expected_current_artifact_id` remains the historical source artifact id from retained publish provenance; the missing-binding path does not pretend an immediate binding exists.

The bounded post-anchor activation chain now tracks the latest durable repair state per locale instead of requiring every locale to appear at most once.

A repeated locale is admitted only when:

- the original retained source artifact is still physically absent;
- the current locale binding is absent;
- all activation receipts from the selected publish-or-exact-rollback anchor to the current page version form one contiguous bounded chain;
- every receipt request hash, rebuild receipt and retained provenance source revalidates;
- the repeated locale's previously activated rebuilt artifact is physically absent;
- every other locale's latest repaired binding still points to its latest rebuilt artifact;
- every other latest rebuilt artifact still matches its receipt-bound instance key, artifact hash and materialization hash.

The chain remains limited to 256 activation steps and the receipt query remains physically capped at 257 rows.

## Rollback continuity

Repair-aware rollback reconstruction now follows the same latest-state-per-locale lineage.

For a locale that appears more than once, an earlier rebuilt instance must be absent before the later activation can supersede it in the proof. A missing-manifest locale is considered proven only when the prefix reaches the activation whose replacement artifact id equals the artifact that is currently bound for that locale. This prevents rollback from stopping at the first historical activation of a repeatedly recovered locale.

At the minimal completion point, every latest repaired locale represented in the prefix must match the current repaired artifact set and its latest rebuilt instance must still pass receipt-bound identity checks.

Historical rollback targets remain unchanged: they still require their original immutable manifest and live immutable artifacts. Repeated recovery is authority only for reconstructing the current repaired cursor.

## Deliberately unchanged boundaries

This slice does not add:

- schema or migration changes;
- DTO, GraphQL, HTTP, OpenAPI or admin UI changes;
- automatic audit -> rebuild, rebuild -> activation or activation -> rollback chaining;
- mutable draft content as repair authority;
- provenance-only historical rollback targets;
- source artifact recreation;
- inline cache mutation.

Existing-binding activation remains strict and does not fall through into missing-binding recovery.

## PostgreSQL source packet

New environment-gated source packet:

```text
crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs
```

It retains four unexecuted scenarios:

1. the same locale loses its canonical artifact, recovers to `R1`, loses `R1`, then explicitly rebuilds and activates `R2`; exact replay remains idempotent;
2. deleting only the rebuilt binding while leaving the prior rebuilt artifact alive rejects repeated recovery without lifecycle mutation;
3. another locale can recover after a chain in which the first locale was recovered twice, while the latest first-locale rebuilt artifact remains active;
4. rollback to an older publish continues after the current publish locale was recovered twice, proving rollback reconstruction reaches the latest activation rather than the first occurrence.

Machine evidence:

```text
crates/rustok-pages/contracts/evidence/pages-repeated-artifact-loss-recovery-source.json
```

Static source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-repeated-artifact-loss-recovery.mjs
```

## Validation boundary

Execution remains pending. No Rust tests, PostgreSQL/SQLite scenarios, Node verifiers, Cargo checks, formatting, migrations, workflows or CI were run by this implementation slice. FFA/FBA promotion remains blocked on accepted execution evidence.
