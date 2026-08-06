# Pages / Page Builder Rebuild Provenance Continuation

Date: 2026-08-06  
Status: source-ready / execution-pending
Scope: Pages immutable artifact audit, reviewed publication provenance and the next append-only repair/rebuild boundary

## Rechecked parity cursor

Current `main` already contains:

- provider-owned reviewed static publish resource limits from PR #3099;
- the bounded Pages immutable artifact integrity audit from PR #3103;
- tenant-admin GraphQL, HTTP and OpenAPI audit adapters from PR #3107.

The audit can identify invalid, partial or oversized retained artifacts, but it intentionally cannot reconstruct historical Page Builder source from the mutable current draft. The next source prerequisite is immutable source provenance captured when a reviewed publish receipt is created.

## Continued source slice

New reviewed publications now retain one source row per publish operation and locale in:

```text
page_publish_rebuild_sources
```

The row carries immutable source provenance for the exact selected body revision and its canonical sanitized Page Builder snapshot, plus the reviewed, artifact and materialization identity hashes needed to admit a future rebuild safely.

Capture is owned by the existing publish-operation after-save hook. The hook re-sanitizes the exact current body, verifies the canonical sanitizer envelope, requires complete materialization identity/runtime snapshots, recomputes both locale-ordered publish set hashes and writes the manifest plus provenance before the owner transaction commits.

The provenance record is independent of the artifact row. It remains available if an artifact row is missing, while page/publish-operation ownership still controls its lifecycle.

## Parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Reviewed static publish resource budgets | Source-ready | Real-project and publish execution pending |
| Bounded immutable artifact audit | Source-ready | SQLite/PostgreSQL execution pending |
| GraphQL/HTTP/OpenAPI audit transports | Source-ready | Transport execution pending |
| New-publish immutable source provenance | Source-ready | Migration and publish execution pending |
| Legacy provenance backfill/import | Open | Not designed |
| Explicit append-only repair/rebuild command | Open | Not implemented |
| Explicit binding replacement | Open | Not implemented |
| Automatic repair | Deliberately absent | Not allowed by this slice |
| FFA/FBA promotion | Open | Not promoted |

## Preserved boundaries

No automatic repair is added.

This slice does not:

- mutate, delete or overwrite an existing immutable artifact;
- change a published binding;
- rebuild any artifact;
- backfill a historical publish operation;
- treat the mutable current draft as historical authority;
- persist the complete reviewed runtime context or bypass future context reauthorization;
- add GraphQL, HTTP, admin UI or worker transports;
- emit new events or rotate cache generations;
- change anonymous storefront rendering, artifact HTTP behavior or rollback selection;
- claim tests, verifiers, Cargo, formatting, migrations, database scenarios, workflows or CI execution;
- promote FFA or FBA.

## Next cursor

1. Execute the provenance source guard and focused migration/publish scenarios.
2. Prove an aggregate sanitized/artifact set mismatch aborts the complete owner transaction.
3. Prove one exact locale row is retained for every new reviewed publish locale and that legacy operations remain unbackfilled.
4. Prove provenance remains readable when the artifact row is absent.
5. Define the explicit repair input around one tenant, page, publish operation, locale and expected provenance hash.
6. Require tenant-wide `pages:manage` and an explicitly reviewed runtime context matching the stored review/context identities.
7. Compile and append a new immutable artifact only; keep binding replacement a separate idempotent command.
8. Retain lifecycle/cache evidence only after an explicit binding switch.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-transport.mjs
cargo test -p rustok-pages publish_rebuild_provenance -- --nocapture
cargo check -p rustok-pages --all-targets
```

Execution evidence remains pending.
