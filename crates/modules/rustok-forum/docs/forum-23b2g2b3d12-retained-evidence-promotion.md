# FORUM-23B2G2B3D12 retained evidence review and promotion candidate

## Status

`source_ready_maintainer_execution_pending`

This slice adds the review boundary after the D11 aggregate assembler. It does
not execute D2 through D10, run D11, create runtime evidence during source
implementation, promote the canonical D0 contract or close `FORUM-23` or
`LINK-FORUM-03`.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-retained-evidence-promotion.json
```

The reviewer is:

```text
scripts/evidence/review-forum-search-versioned-invalidation-runtime-evidence.mjs
```

The source-only verifier is:

```text
scripts/verify/verify-forum-search-versioned-invalidation-retained-evidence-promotion.mjs
```

The reviewed promotion candidate is written to:

```text
target/forum-search-versioned-invalidation-runtime-promotion-candidate.json
```

## Why review is separate from assembly

D11 proves that one aggregate file can be assembled only from nine successful
runtime artifacts generated from the same source commit. It does not establish
that a maintainer inspected and retained the exact aggregate bytes.

D12 requires an explicit reviewer identity, an immutable retention reference and
an independently supplied SHA-256 attestation. The reviewer recomputes the
aggregate digest, requires the supplied retained digest to match it, and records
both values in a promotion candidate.

The script does not contact or authenticate the external retention service. The
maintainer is responsible for placing the exact bytes at the stated retention
reference before invoking D12. That limitation is explicit in the promotion
candidate rather than hidden behind a successful local check.

## Complete revalidation

Before writing a candidate, the reviewer requires:

1. the D0 parent contract to retain exact identity, pending status, aggregate
   path and the ten frozen scenarios;
2. D12 to be registered in the D0 source-ready subproof list;
3. the aggregate artifact to report `runtime_evidence_assembled`, PostgreSQL,
   `outbox_iggy`, consumer group `rustok-search-forum-projection-v1`, topic
   `domain` and the current Git commit;
4. all ten frozen scenarios to appear exactly once, in canonical order, with
   `passed`, non-empty facts and the exact D2 through D10 source mapping;
5. all nine source artifacts to exist and retain exact contract, task, scenario,
   PostgreSQL and source-commit identity;
6. every aggregate source-artifact SHA-256 and byte length to match the bytes
   still present under `target/`;
7. the aggregate parent-contract digest to match the current D0 source bytes;
8. every grouped owner-revision, identity, inbox, ingest-sequence, checkpoint,
   poison, DLQ and storefront fact to equal the attributed source scenario;
9. the assembly record to retain nine validated inputs, ten frozen scenarios,
   one source commit, current-HEAD equality and write-after-validation;
10. the maintainer-supplied retained SHA-256 to equal the exact aggregate digest.

Any failure occurs before promotion-candidate creation or replacement. There is
no skip, stale-source, mixed-commit, partial-review or canonical-source mutation
mode.

## Required maintainer attestations

The reviewer accepts no command-line arguments. It requires three bounded
environment variables:

```text
RUSTOK_FORUM_EVIDENCE_REVIEWER
RUSTOK_FORUM_EVIDENCE_RETENTION_REF
RUSTOK_FORUM_EVIDENCE_RETAINED_SHA256
```

`RUSTOK_FORUM_EVIDENCE_REVIEWER` identifies the accountable reviewer.
`RUSTOK_FORUM_EVIDENCE_RETENTION_REF` identifies the immutable retained object.
`RUSTOK_FORUM_EVIDENCE_RETAINED_SHA256` must be the lowercase SHA-256 digest of
the exact aggregate JSON bytes.

The reviewer and retention reference reject surrounding whitespace, control
characters and unbounded lengths. The retained digest must be exactly 64
lowercase hexadecimal characters.

## Promotion candidate

After complete validation, D12 writes one atomic JSON candidate containing:

- reviewer, review time and exact source commit;
- retention reference and attested aggregate SHA-256;
- aggregate path, digest, byte length, generation time and frozen scenarios;
- current D0 parent path, digest and pending status;
- all nine source artifact identities and digests;
- explicit successful validation assertions;
- the proposed transition from
  `source_ready_maintainer_execution_pending` to `runtime_evidence_reviewed`;
- explicit statements that a separate canonical-source pull request is required
  and that neither `FORUM-23` nor `LINK-FORUM-03` is closed.

The reviewer never edits the D0 contract or the implementation plan. This keeps
runtime review evidence outside source control until a maintainer deliberately
opens the separate promotion PR with the candidate and retained artifact
available for review.

## Maintainer order

On one checked-out commit:

1. run all D2 through D10 verifiers and executable runtime proofs;
2. run the D11 verifier and aggregate assembler;
3. retain the exact aggregate bytes in the chosen immutable evidence store;
4. compute the retained object's SHA-256 independently;
5. run:

```bash
node scripts/verify/verify-forum-search-versioned-invalidation-retained-evidence-promotion.mjs
RUSTOK_FORUM_EVIDENCE_REVIEWER="<reviewer>" \
RUSTOK_FORUM_EVIDENCE_RETENTION_REF="<immutable-retention-reference>" \
RUSTOK_FORUM_EVIDENCE_RETAINED_SHA256="<aggregate-sha256>" \
node scripts/evidence/review-forum-search-versioned-invalidation-runtime-evidence.mjs
```

Only after the promotion candidate is reviewed should a separate source PR
update canonical D0 and plan status. That later PR must not claim
`LINK-FORUM-03` completion unless its separate runtime proof also exists.

## Compatibility

D12 adds scripts, documentation and machine contracts only. It changes no Rust
production path, migration, event schema or digest, DTO, runtime flag,
dependency, `Cargo.toml` or `Cargo.lock` entry.

No command above was run by the implementation agent, per maintainer request.
