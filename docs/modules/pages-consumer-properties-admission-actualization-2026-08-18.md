# Pages consumer-properties admission actualization — 2026-08-18

Status: `source-ready / maintainer-browser-and-deployment-evidence-pending / admission-runner-ready`.

## Cursor

The first remaining Page Builder FBA terminal blocker is still
`/provider/consumer_properties_contract/executed_evidence = pending`.

The exact-main Rust/source successor run after the browser-run indexing slice is now real:
run `32170986733` completed successfully for
`2a3f717dc3cac8b0c99c5b1cbe4bee7c8c5492bd`, and retained the
`pages-consumer-properties-source-32170986733-2a3f717dc3cac8b0c99c5b1cbe4bee7c8c5492bd`
artifact. Its stable commit-status context is
`pages-consumer-properties-source-evidence-index`.

The protected browser workflow has a separate stable exact-commit context,
`pages-published-metadata-browser-evidence-index`, but no successful real browser packet is
claimed by this source slice.

This slice closes the source-architecture gap between those retained execution packets and a
future evidence-containing registry update. It does not change `executed_evidence`.

## Admission source

The machine contract is:

`crates/rustok-pages/contracts/evidence/pages-consumer-properties-admission-source.json`

The offline admission runner is:

`scripts/evidence/admit-pages-consumer-properties.mjs`

It accepts exactly three bounded JSON inputs:

1. the exact-main Rust/source receipt;
2. the retained published-metadata browser packet;
3. a separate maintainer-reviewed deployment provenance packet.

All three must bind the same checkout source commit. The browser packet and deployment
provenance must bind the same immutable `REPOSITORY@sha256:<digest>`.

The runner also re-hashes every retained source-file set against the checkout and requires the
consumer contract plus Page Builder FBA registry to still contain the pre-admission
`executed_evidence = pending` values.

## Deployment provenance boundary

The browser packet deliberately cannot prove that the reviewed route was served by the supplied
RepoDigest. The deployment provenance input therefore records that association as
`maintainer_reviewed_external_fact`.

The reviewed packet binds:

- the exact source commit;
- the immutable deployment RepoDigest;
- the SHA-256 identities of all four reviewed browser profile URLs;
- the successful source workflow run id and
  `pages-consumer-properties-source-evidence-index`;
- the successful browser workflow run id and
  `pages-published-metadata-browser-evidence-index`;
- a bounded reviewer/operator id.

The packet must explicitly retain:

- `cryptographic_signature_present = false`;
- `cryptographic_origin_to_repo_digest_binding = false`;
- no raw profile URLs;
- no credentials.

The admission runner does not query GitHub. The workflow run ids and commit-status outcomes are
maintainer-reviewed external facts supplied to the packet. Their equality and exact-source
lineage are checked offline; they are not upgraded into cryptographic CI or deployment
attestation.

## Fail-closed output

A successful runner invocation writes only:

`target/pages-consumer-properties-admission.json`

with:

- format `pages_consumer_properties_admission_v1`;
- status `consumer_properties_execution_evidence_admitted_registry_update_pending`;
- exact source commit and immutable RepoDigest;
- source/browser workflow run ids and index contexts;
- SHA-256 identities of the reviewed browser routes;
- byte lengths and SHA-256 hashes of all three input packets;
- SHA-256 hashes of the source files defining the admission contract.

The output explicitly keeps these boundaries false:

- consumer-contract mutation;
- FBA-registry mutation;
- `executed_evidence` verification;
- terminal-inventory completion;
- Pages FFA promotion;
- Page Builder FBA promotion;
- cryptographic CI attestation;
- cryptographic origin-to-RepoDigest binding.

A later evidence-containing PR must review a real admission packet before changing the consumer
contract or the Page Builder FBA registry.

## Synthetic coverage

`scripts/evidence/admit-pages-consumer-properties.test.mjs` exercises one admissible synthetic
exact-checkout chain and verifies fail-closed rejection for:

- source-commit drift;
- deployment-digest drift;
- a failed browser observation;
- reviewed route-hash drift;
- source workflow run-id drift;
- an invalid cryptographic deployment overclaim.

The static source guard is:

`node scripts/verify/verify-pages-consumer-properties-admission.mjs`

The synthetic runner coverage is:

`node scripts/evidence/admit-pages-consumer-properties.test.mjs`

Both are wired into the existing Pages Consumer Properties Source Evidence workflow. The new
admission files are also included in the Rust/source receipt source-hash set.

## Successor receipt requirement

Because this slice changes the admission source files bound by the Rust/source receipt, merge of
this PR invalidates the old receipt as the final admission source lineage. A successor exact-main
source receipt must complete successfully on the merge commit before a real browser/deployment
admission can be accepted.

## Governance

This source slice performs no browser execution, deployment operation, network observation,
metadata mutation, consumer-contract mutation, FBA-registry mutation, owner acceptance, platform
acceptance, FFA promotion, or FBA promotion.

The provider consumer-properties `executed_evidence` remains `pending`.

## Next cursor

1. merge this source-only admission scaffold and retain the successor exact-main source receipt;
2. maintainers execute the protected browser workflow against that exact reviewed deployment;
3. maintainers review deployment provenance and both exact-commit workflow indexes;
4. run the offline admission runner over the real three-packet chain;
5. only then prepare a separate evidence-containing PR that changes the two pending
   consumer-properties evidence values.
