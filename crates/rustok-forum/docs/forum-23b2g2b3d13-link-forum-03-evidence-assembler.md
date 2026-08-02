# FORUM-23B2G2B3D13 LINK-FORUM-03 core evidence assembler

## Status

`source_ready_maintainer_execution_pending`

This slice adds a fail-closed downstream evidence boundary for the already
delivered ordering, visibility and Search-disabled core of canonical
`LINK-FORUM-03`. It does not run PostgreSQL, Iggy, Forum, Search or storefront
code and does not change the canonical `LINK-FORUM-03` status from `planned`.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-search-link-forum-03-evidence-assembler.json
```

The assembler is:

```text
scripts/evidence/assemble-link-forum-03-forum-search-evidence.mjs
```

The partial output path is:

```text
target/link-forum-03-forum-index-search-ordering-visibility-evidence.json
```

## Why this is downstream of D12

The D0 evidence chain deliberately separates executable runtime proofs from
review and retention:

- D8 proves deletion, richer ACL, reverse/stale delivery and storefront
  fail-closed visibility;
- D9 proves Forum owner writes while Search storage/runtime are absent and one
  late owner-ledger recovery after Search is enabled;
- D10 proves one correlated real Forum owner transaction through external Iggy,
  durable Search ingress, production projection, delivery-covered checkpoint and
  exact storefront visibility;
- D11 assembles all D2-D10 runtime artifacts from one exact source commit;
- D12 re-reads the aggregate and every retained source artifact, requires an
  explicit reviewer and retained SHA-256 attestation, and writes a promotion
  candidate without mutating canonical source.

D13 does not replace those gates. It requires their retained outputs and derives
only the partial cross-module evidence already supported by D8, D9 and D10.

## Required artifacts

All five files must exist on the same checked-out commit:

```text
target/forum-search-versioned-invalidation-runtime-promotion-candidate.json
target/forum-search-versioned-invalidation-runtime-evidence.json
target/forum-search-versioned-invalidation-deletion-acl-ordering-evidence.json
target/forum-search-versioned-invalidation-search-disabled-recovery-evidence.json
target/forum-search-versioned-invalidation-normal-delivery-evidence.json
```

Skipped tests do not create acceptable evidence. Static fixtures, copied JSON,
manually edited facts and mixed-commit artifacts are rejected.

## Fail-closed lineage validation

Before writing the partial LINK artifact, the assembler requires:

1. exact D13 and D12 machine-contract identities and pending source statuses;
2. the canonical Forum plan to still record `FORUM-23` as `in_progress` and
   `LINK-FORUM-03` as `planned`;
3. the D0 parent to retain its exact identity, pending status, output path and
   frozen ten-scenario order;
4. the D12 candidate to be
   `approved_for_canonical_status_promotion` for the current `HEAD`;
5. the candidate transition to remain
   `source_ready_maintainer_execution_pending -> runtime_evidence_reviewed` and
   require a separate canonical-source pull request;
6. the candidate parent-contract path, status and SHA-256 to match the exact
   current D0 bytes;
7. the aggregate bytes, byte length, generated time, scenario list and SHA-256
   to match both the D12 candidate and the maintainer retention attestation;
8. the aggregate to retain the exact ten frozen scenarios, all nine D2-D10
   source tasks and the completed D11 assembly invariants;
9. exact D8, D9 and D10 contracts, tasks, scenario IDs, `passed` results,
   PostgreSQL backend and non-empty facts;
10. D8 and D9 to report `broker_used = false`, and D10 to report `outbox_iggy`,
    the canonical consumer group, topic `domain` and a non-empty external stream;
11. D8, D9 and D10 source bytes to match the digest, length, generated time and
    source commit retained in both the D11 aggregate and D12 candidate;
12. aggregate `normal_delivery`, `deletion_acl_ordering` and
    `search_disabled_profile` facts to equal the original D10, D8 and D9 facts;
13. same-directory atomic output after all validation succeeds.

There is no missing-artifact mode, best-effort mode, source-commit override,
skipped-result acceptance or static fallback.

## Evidence represented by the output

After maintainer execution, the generated partial LINK artifact records:

- the correlated Forum owner -> external Iggy -> durable Search inbox ->
  production projector -> delivery-covered checkpoint -> storefront path from
  D10;
- the D8 reverse/stale and duplicate delivery proof showing that hidden, deleted
  and richer-ACL-denied owner content is not restored;
- current-owner reauthorization excluding deliberately stale denied Search rows
  before visible items, totals and facets;
- D9 Search-disabled Forum owner continuity and late owner-ledger recovery;
- exact canonical-plan, D0 parent, D8, D9, D10, D11 aggregate and D12
  promotion-candidate digests;
- the D12 reviewer identity, review time and immutable retention reference;
- the single reviewed source commit shared by the complete retained lineage.

The assembler records that it did not independently authenticate the external
retention service. It validates the explicit D12 retention attestation and exact
retained bytes only.

## Remaining LINK-FORUM-03 runtime scope

This partial artifact cannot mark `LINK-FORUM-03` done. Separate executable
runtime evidence is still required for:

- translation projection and retrieval;
- a real moderation approval transition into Search visibility;
- topic move and category-scope projection update;
- an exact private and trusted-channel exclusion profile;
- review of the final combined LINK artifact before any canonical plan change.

The assembler therefore writes:

```text
status = partial_runtime_evidence_assembled
coverage = ordering_visibility_and_search_disabled_core_only
status_change_allowed_from_this_artifact = false
```

## Maintainer order

On one exact commit, execute the D2-D10 commands, D11 assembler and D12 reviewer
from their contracts. Retain every generated artifact, then run:

```bash
node scripts/verify/verify-link-forum-03-forum-search-evidence.mjs
node scripts/evidence/assemble-link-forum-03-forum-search-evidence.mjs
```

A later bounded slice must add the remaining executable LINK scenarios and
assemble a final reviewed artifact. Only that later complete artifact may support
another canonical-source pull request.

## Deliberate boundary

D13 adds scripts, documentation and a machine contract only. It changes no Rust
production path, migration, event schema or digest, DTO, runtime flag,
dependency, `Cargo.toml` or `Cargo.lock` entry.

It does not close arbitrary channel/group filtering, topic kinds or attachment
presence. Those remain blocked on their named owner contracts.

No command above was run by the implementation agent, per maintainer request.
