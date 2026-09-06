# FORUM-23B2G2B3D18 complete LINK-FORUM-03 evidence promotion

## Status

`source_ready_maintainer_execution_pending`

D18 adds the independent maintainer review and retention boundary for the complete
runtime artifact assembled by D17. It does not execute Forum, Search, PostgreSQL,
Iggy or storefront code and does not edit the canonical Forum plan.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-promotion.json
```

The reviewer is:

```text
scripts/evidence/review-link-forum-03-complete-forum-search-evidence.mjs
```

The generated promotion candidate is:

```text
target/link-forum-03-forum-index-search-complete-promotion-candidate.json
```

## Review boundary

D17 can assemble complete canonical runtime coverage, but its output explicitly
records:

```text
status = complete_runtime_evidence_assembled_review_pending
complete_artifact_independently_reviewed = false
complete_artifact_retention_attested = false
status_change_allowed_from_this_artifact = false
```

D18 is the separate review gate. It requires an explicit reviewer identity, an
immutable retention reference and the exact SHA-256 of the retained complete
artifact. It does not retrieve the retention object, authenticate an external
retention service or create a cryptographic signature.

## Required retained artifacts

All files must be generated on the same checked-out commit:

```text
target/link-forum-03-forum-index-search-complete-evidence.json
target/link-forum-03-forum-index-search-ordering-visibility-evidence.json
target/forum-search-link-forum-03-translation-moderation-evidence.json
target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json
target/forum-search-link-forum-03-topic-move-evidence.json
```

Missing artifacts, skipped results, copied fixtures, hand-edited JSON and
mixed-commit artifacts are rejected.

## Fail-closed validation

Before writing a promotion candidate, the reviewer checks:

1. the D18 and D17 machine contracts have exact identities, pending source
   statuses, paths and scenario order;
2. the canonical plan still records `FORUM-21` as `planned`, `FORUM-23` as
   `in_progress` and `LINK-FORUM-03` as `planned`;
3. the complete D17 artifact has exact contract, source slice, review-pending
   status, canonical runtime coverage and current `HEAD`;
4. all six canonical scenarios exist in exact order;
5. D13 remains the partial reviewed core with exact inherited D12 reviewer and
   retention lineage;
6. D13 core scenario entries preserve their original D8, D9 and D10 source
   attribution rather than being rewritten as D13-owned runtime facts;
7. D14, D15 and D16 have exact task, contract, current source commit,
   PostgreSQL backend, no-broker profile, passed scenario and non-empty facts;
8. every source artifact's exact SHA-256, byte length, generation time, source
   commit and scenario metadata match the D17 retained records;
9. every complete-artifact scenario fact equals its exact retained source fact;
10. D17 still records independent review and retention as false and forbids
    automatic canonical transition;
11. the maintainer-supplied retained digest equals the exact complete artifact
    bytes;
12. the candidate is written only after validation by same-directory atomic
    rename.

The reviewer accepts no command-line arguments or source-commit overrides.

## Promotion candidate

A successful review writes:

```text
contract = link_forum_03_forum_index_search_complete_promotion_candidate_v1
status = approved_for_canonical_status_promotion
```

The candidate records:

- reviewer identity and review time;
- retention reference and attested complete-artifact digest;
- exact D17 and D18 contract digests;
- exact complete artifact digest, length, generation time and six scenarios;
- exact canonical plan digest and statuses at review time;
- exact metadata for D13 through D16 retained artifacts;
- inherited D13/D12 core-retention lineage;
- complete revalidation assertions;
- the proposed canonical transition.

## Canonical transition boundary

The proposed transition is limited to:

```text
LINK-FORUM-03: planned -> done
```

It requires a separate canonical-source pull request. The reviewer does not edit
the plan. The candidate explicitly records:

```text
promotes_forum_21 = false
promotes_forum_23 = false
canonical_source_mutated_by_reviewer = false
```

`FORUM-21` remains an independent owner-workflow task and `FORUM-23` retains
broader unfinished Search product scope.

## Maintainer command

After generating and immutably retaining the complete D17 artifact, run:

```bash
RUSTOK_LINK_FORUM_03_EVIDENCE_REVIEWER="<reviewer>" \
RUSTOK_LINK_FORUM_03_EVIDENCE_RETENTION_REF="<immutable-retention-reference>" \
RUSTOK_LINK_FORUM_03_EVIDENCE_RETAINED_SHA256="<complete-artifact-sha256>" \
node scripts/evidence/review-link-forum-03-complete-forum-search-evidence.mjs
```

The retained SHA must be the lowercase SHA-256 of the exact bytes at:

```text
target/link-forum-03-forum-index-search-complete-evidence.json
```

## Deliberate boundary

D18 adds a contract, reviewer, verifier and handoff only. It changes no Rust
production code, migration, transport, event schema, runtime flag, dependency,
`Cargo.toml`, `Cargo.lock`, D0-D17 artifact schema or canonical task status.

No command above was run by the implementation agent, per maintainer request.
