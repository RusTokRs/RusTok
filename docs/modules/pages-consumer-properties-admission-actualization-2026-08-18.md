# Pages consumer-properties admission actualization — 2026-08-18

Status: `source-ready / maintainer-browser-and-deployment-evidence-pending / admission-runner-ready`.

## Current evidence and drift

The first remaining Page Builder FBA terminal blocker is still
`/provider/consumer_properties_contract/executed_evidence = pending`.

The latest retained canonical source receipt is run `32177516104` on
`c0a7bd91fc68b5462996a6d4e929bad6e7d6a208`. It is indexed by
`pages-consumer-properties-source-evidence-index` and contains the admission source files in its
retained SHA-256 set.

Current `main` is `de5eec28762b29ead4389d740e3b3aa3e9743de9`. The commits between those points
changed only marketplace-cache files, so the Pages/Page Builder receipt-bound files and both
pending evidence targets are byte-identical. The source-evidence workflow is path-filtered and
correctly did not spend another Rust run on that unrelated drift.

## Source lineage policy

A source receipt no longer has to equal checkout HEAD. It is admissible only when all of these
conditions hold:

- the receipt was produced by the canonical `Pages Consumer Properties Source Evidence` workflow
  on `main`;
- its `source_commit` is checkout HEAD or an ancestor verified locally with
  `git merge-base --is-ancestor`;
- every retained receipt-bound source SHA-256 still matches the current checkout byte-for-byte;
- the consumer contract and Page Builder FBA registry still match the receipt target hashes and
  both still contain `executed_evidence = pending`;
- the reviewed `pages-consumer-properties-source-evidence-index` success belongs to the receipt
  commit and run id.

The runner never fetches history. If the checkout is shallow enough that ancestry cannot be
verified locally, admission fails closed. This is intentional: absence of local ancestry proof is
not converted into an external-network lookup.

The browser/deployment side stays stricter. A real browser packet must bind exact current checkout
HEAD, and the maintainer-reviewed deployment provenance must bind that same exact commit and the
same immutable `REPOSITORY@sha256:<digest>`. The reviewed
`pages-published-metadata-browser-evidence-index` success therefore belongs to the browser/deployment
commit, while the source index belongs to the source receipt commit.

Those workflow-status observations and the origin-to-RepoDigest association remain
`maintainer_reviewed_external_fact`; the offline runner does not query GitHub and does not upgrade
them into cryptographic CI or deployment attestation.

## Fail-closed runner

`scripts/evidence/admit-pages-consumer-properties.mjs` accepts three bounded JSON inputs: the source
receipt, protected browser packet, and reviewed deployment provenance. It re-hashes source files,
checks pending target bytes, verifies source ancestry, enforces exact browser/deployment commit and
RepoDigest identity, and binds the supplied packet SHA-256 values.

Its output separately retains checkout commit, source receipt commit, and browser/deployment
commit. It does not change `executed_evidence`, the consumer contract, or the FBA registry.

Synthetic coverage accepts the equal-HEAD case used by normal PR CI and rejects ten classes of
lineage/provenance drift. An additional local fixture was exercised with a receipt from a parent
commit and an unrelated descendant commit; it passed only while all receipt-bound bytes remained
byte-identical.

## Successor receipt requirement

This lineage-policy slice itself changes files that are part of the source receipt hash set.
Therefore its merge still requires one successor exact-main source receipt. After that successor
exists, later unrelated descendant commits may reuse it without another Rust run only while the
full retained source set and both pending target hashes remain unchanged.

## Next cursor

1. merge this source-only lineage policy and retain its successor exact-main source receipt;
2. execute the protected published-metadata browser workflow on the reviewed current deployment;
3. review deployment provenance, binding the source status to the receipt commit and the browser
   status to the browser/deployment commit;
4. run the offline admission runner over the real three-packet chain;
5. only then prepare a separate evidence-containing PR that changes the two pending
   consumer-properties evidence values.
