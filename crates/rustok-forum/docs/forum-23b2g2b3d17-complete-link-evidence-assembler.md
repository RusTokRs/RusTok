# FORUM-23B2G2B3D17 complete LINK-FORUM-03 evidence assembler

## Status

`source_ready_maintainer_execution_pending`

D17 adds the fail-closed downstream assembler for the complete canonical
`LINK-FORUM-03` runtime scenario set. It combines the reviewed D13 ordering,
visibility and Search-disabled core with the source-ready D14 translation and
moderation proof, D15 private and trusted-channel exclusion proof, and D16 real
topic-move category-scope proof.

D17 does not execute any runtime command and does not change the canonical
`FORUM-21`, `FORUM-23` or `LINK-FORUM-03` status.

## Files

Machine contract:

```text
crates/rustok-forum/contracts/forum-search-link-forum-03-complete-evidence-assembler.json
```

Assembler:

```text
scripts/evidence/assemble-link-forum-03-complete-forum-search-evidence.mjs
```

Source verifier:

```text
scripts/verify/verify-link-forum-03-complete-forum-search-evidence.mjs
```

Output:

```text
target/link-forum-03-forum-index-search-complete-evidence.json
```

## Required retained inputs

All inputs must have been generated from the exact checked-out `HEAD`:

```text
target/link-forum-03-forum-index-search-ordering-visibility-evidence.json
target/forum-search-link-forum-03-translation-moderation-evidence.json
target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json
target/forum-search-link-forum-03-topic-move-evidence.json
```

The first input is produced by D13 only after the D11 aggregate and D12 reviewer
candidate have passed their exact digest and retention checks. D17 revalidates
that partial artifact identity, its three selected scenario records, its retained
lineage digests and its canonical-transition boundary.

The other three inputs must each contain exactly one `passed` PostgreSQL scenario
for the current source commit:

- D14: `translation_and_moderation_approval`;
- D15: `private_and_trusted_channel_exclusion`;
- D16: `topic_move_category_scope`.

Missing files, copied fixtures, skipped results, mixed commits, empty facts,
argument overrides and best-effort assembly are rejected.

## Complete runtime coverage represented

After maintainer execution, the output assembles these six scenario groups:

1. `normal_delivery` — one correlated owner transaction through external Iggy,
   durable Search ingress, projection checkpoint and storefront visibility;
2. `deletion_acl_ordering` — duplicate, reverse and stale delivery cannot restore
   hidden, deleted or richer-ACL-denied content;
3. `search_disabled_profile` — Forum owner writes continue while Search is absent
   and later owner-ledger recovery converges;
4. `translation_and_moderation_approval` — English remains visible, French
   translations become visible, pending reply stays absent and real approval
   exposes the exact reply;
5. `private_and_trusted_channel_exclusion` — legitimate projection excludes
   restricted topics and stale candidates are allowed only by exact current owner
   authorization;
6. `topic_move_category_scope` — the real idempotent owner move retains topic and
   reply identities while old category scope empties and new scope gains both.

All runtime inputs must report the same current source commit. The assembler
records the exact bytes, SHA-256, byte length, generated time and scenario
identity of every input.

## Why the output remains review-pending

Runtime coverage and evidence review are separate gates. D13 carries an existing
D12 reviewer and retention lineage for the older core, but D14, D15 and D16 are
new independent runtime artifacts. D17 does not treat successful assembly as a
review or retention attestation for those files or for the new complete output.

The output therefore uses:

```text
status = complete_runtime_evidence_assembled_review_pending
coverage = canonical_link_forum_03_runtime_scope
status_change_allowed_from_this_artifact = false
```

It also records:

```text
complete_artifact_independently_reviewed = false
complete_artifact_retention_attested = false
canonical_source_mutated_by_assembler = false
```

A later bounded reviewer slice must re-read the complete artifact and every
source artifact, bind them to immutable retention, and create a promotion
candidate. Only a separate canonical-source pull request may then change the
`LINK-FORUM-03` status.

## Maintainer order

Generate and retain the D13 core using its existing D11/D12 workflow. Then run
the D14, D15 and D16 source verifiers and PostgreSQL tests from their contracts.
Finally run:

```bash
node scripts/verify/verify-link-forum-03-complete-forum-search-evidence.mjs
node scripts/evidence/assemble-link-forum-03-complete-forum-search-evidence.mjs
```

The assembler accepts no command-line arguments.

## Deliberate boundaries

D17 adds no Rust production code, migration, event schema, digest, dependency,
runtime flag, transport or workflow. It does not edit D0, D12, D13, D14, D15,
D16 or the canonical implementation plan.

The complete LINK artifact does not independently promote `FORUM-21`; merge,
split, fork, reply-range operations, URL aliases and transports remain outside
the bounded topic-move owner slice. It also does not close `FORUM-23`.

No command above was run by the implementation agent, per maintainer request.
