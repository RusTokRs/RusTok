# Pages / Page Builder Multi-Locale Repair-to-Rollback Evidence Actualization

Date: 2026-08-07  
Status: current-source-overlay / multilocale-repair-rollback-evidence-source-ready / rollback-activated-loss-recovery-source-open / execution-open

This overlay continues `pages-page-builder-multilocale-activation-recovery-actualization-2026-08-07.md` after PR #3178 made sequential physical-loss activation source-ready for multiple locales from one reviewed publish.

## Main recheck

The source-ready Pages/Page Builder owner chain currently covers the direct reviewed-publish recovery path:

```text
reviewed publish
-> immutable manifest + retained rebuild provenance
-> bounded audit
-> explicit rebuild
-> explicit activation
-> missing-binding physical-loss recovery
-> sequential same-publish multi-locale recovery
-> repair-aware current rollback cursor
-> strict historical rollback target
```

Execution remains separate and pending. This overlay does not reopen any source-complete transport, cache, authoring or release-composition boundary.

## Newly confirmed durable-evidence gap

PR #3178 made the **activation admission** for later physically lost locales stricter than the durable **rollback reconstruction** of the same repaired current cursor.

Activation now proves every page-version step between the source publish and a later missing-binding recovery with a bounded, contiguous, same-publish chain of activation receipts. It also recomputes every prior activation request hash and revalidates rebuild/provenance/binding/artifact identity.

The repair-aware rollback fallback added in PR #3173 still validated each current repaired locale independently. It checked receipt shape and exact source/rebuild/activation identity, but it did not:

- recompute the canonical activation request hash; or
- independently prove the physical-loss recovery receipts as the same contiguous multi-locale prefix that admission required.

That meant corrupted durable receipt evidence could be structurally SHA-shaped and individually self-consistent enough to reach rollback reconstruction even though the same evidence could never have passed current activation admission.

## Source change

`crates/rustok-pages/src/services/page/artifact_set.rs` now revalidates the durable evidence at rollback time rather than trusting that admission once happened correctly.

### Canonical activation request identity

Every repaired-current activation receipt recomputes the same request hash payload used by `PageService::replace_rebuilt_artifact_binding`:

```text
operation_format
+ tenant_id
+ page_id
+ rebuild_operation_id
+ expected_version
+ expected_current_artifact_id
```

A valid-looking but noncanonical SHA-256 request hash is rejected.

### Minimal physical-loss activation prefix

The fallback first identifies the locales whose current source manifest rows are missing and whose historical source artifacts are absent. Those are the durable physical-loss locales that require recovery proof.

Rollback then scans at most 257 ordered activation rows and accepts at most the first 256 receipts beginning immediately after the source publish version. Until every required lost-manifest locale has been proven, the prefix requires:

- exact `expected_version -> result_version` contiguity;
- unique locale identity;
- canonical activation request hash;
- an exact retained source from the selected publish;
- exact rebuild receipt and provenance identity;
- exact activation body/source/replacement identity;
- the receipt-bound rebuilt artifact instance key, artifact hash and materialization hash to remain present.

The function returns as soon as every required physical-loss locale has appeared in that verified prefix.

This is intentionally a **minimal prefix**, not a requirement that every later page version remain an activation. After physical-loss recovery is complete, later ordinary activation, metadata/lifecycle work or other valid page-version changes are outside this proof and remain allowed by the existing rollback cursor contract.

## Preserved fail-closed boundaries

This slice keeps all earlier fences:

- original publish manifest is always tried first;
- database errors are not hidden by repair fallback;
- current active artifact-set hash must equal the selected publish receipt;
- complete retained publish provenance is required;
- surviving manifest rows stay authoritative and must match retained provenance;
- unchanged locales still require their original manifest rows;
- a missing repaired manifest row is admitted only when its historical source artifact is absent;
- current repaired artifacts still require exact rebuild and activation receipts;
- historical rollback targets still require their original manifest and live immutable artifacts;
- no provenance fallback is added for historical targets;
- no schema, migration, DTO, GraphQL, HTTP, OpenAPI, admin UI or cache contract changes are added;
- no audit -> rebuild -> activation -> rollback automation is added.

## PostgreSQL source packet

New execution-pending packet:

```text
crates/rustok-pages/tests/artifact_multilocale_repair_rollback_evidence_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-multilocale-repair-rollback-evidence.mjs
crates/rustok-pages/contracts/evidence/pages-multilocale-repair-rollback-evidence-source.json
```

It retains three source scenarios:

1. publish A -> publish B with `en` + `fr` -> physical loss of both B source artifacts -> rebuild/activate `en` then `fr` -> rollback succeeds to A and exact replay is idempotent;
2. the same recovered current cursor with one activation receipt changed to a different but still SHA-shaped request hash -> rollback is rejected without page-version or binding mutation;
3. the same recovered current cursor with the first receipt changed so both activation records remain individually `result_version = expected_version + 1` and have canonical request hashes, but no longer form a contiguous prefix from the source publish -> rollback is rejected without page-version or binding mutation.

The packet is authored only. PostgreSQL was not executed.

## Adjacent source gap: physical loss after rollback activation

The same recheck exposed a separate owner-level gap that is intentionally **not** folded into this PR.

A publish artifact set can become current through `page_rollback_operations`. In that state the active set is traceable to an older publish, but `pages.version` is the newer rollback receipt `result_version`, not that source publish's original `result_version`.

Current missing-binding recovery accepts either:

1. the source publish version itself; or
2. a contiguous chain of artifact activation receipts starting at that source publish version.

A rollback receipt is neither. Therefore, if an artifact is physically lost **after rollback made that older publish current**, explicit missing-binding recovery still rejects the otherwise traceable current state as stale.

The next source slice must introduce an exact activation anchor that can be either the source publish receipt itself or a current rollback receipt resolving to that exact publish, then apply the same bounded repair-chain rules from that anchor. Arbitrary lifecycle/version drift must remain rejected.

## Actualized matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Single-locale physical-loss activation from direct publish | Source-ready | PostgreSQL execution pending |
| Multi-locale sequential physical-loss activation from direct publish | Source-ready | PostgreSQL execution pending |
| Repair-aware rollback current cursor | Source-ready | PostgreSQL execution pending |
| Canonical activation request-hash revalidation during rollback | **Source-ready in this overlay** | PostgreSQL negative execution pending |
| Minimal physical-loss activation-prefix revalidation during rollback | **Source-ready in this overlay** | PostgreSQL success/negative execution pending |
| Later ordinary activation or page-version work after completed recovery | Preserved | Execution pending |
| Physical loss after rollback activated an older publish | **Source gap confirmed** | Not yet source-ready |
| Historical target provenance fallback | Deliberately absent | Not allowed |
| Automatic repair/rollback chaining | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Implement physical-loss recovery when the exact current artifact set was activated by a rollback receipt resolving to the source publish; preserve a strict publish-or-current-rollback activation anchor and bounded subsequent repair chain.
2. Execute the new multi-locale repair-to-rollback packet and retain success plus canonical-request-hash and noncontiguous-prefix rejection evidence.
3. Execute the prior single-/multi-locale activation recovery, repair rollback, provenance, rebuild, audit, failure/atomicity/cache and repair transport packets.
4. Retain metadata conflict/isolation, cache, artifact/HTTP/browser and tenant Wave evidence.
5. Promote no FFA/FBA state until source gaps are closed and accepted execution evidence exists.

## Validation boundary

Source review and authoring only; execution remains pending. No tests, Node verifiers, Cargo commands, formatting, migrations, PostgreSQL scenarios, workflows, CI or `git diff --check` were executed by this authoring workflow.
