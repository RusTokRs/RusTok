# FORUM-23B2G2B3D19 LINK-FORUM-03 canonical promotion packet

## Status

`source_ready_maintainer_execution_pending`

D19 adds the final source-ready handoff boundary between a retained D18 promotion
candidate and a separate human-reviewed canonical-source pull request. It does
not execute Forum, Search, PostgreSQL, Iggy or storefront code and does not edit
the canonical Forum plan.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-search-link-forum-03-canonical-promotion-packet.json
```

The packet builder is:

```text
scripts/evidence/prepare-link-forum-03-canonical-promotion-packet.mjs
```

The generated packet is:

```text
target/link-forum-03-canonical-promotion-packet.json
```

## Why D19 remains separate

D18 can generate an approved promotion candidate only after an independent
reviewer supplies an identity, immutable retention reference and the exact
retained SHA-256 of the complete D17 artifact. That candidate deliberately does
not edit canonical source.

D19 converts the validated candidate into a bounded canonical-PR packet. The
packet records the exact current plan digest, the exact planned ledger row, the
proposed done row, all retained evidence identities and the required completion
record. It still does not apply the edit.

This separation prevents a reviewer script from both approving its own evidence
and changing the canonical roadmap.

## Required retained inputs

All inputs must exist on the exact checked-out commit:

```text
target/link-forum-03-forum-index-search-complete-promotion-candidate.json
target/link-forum-03-forum-index-search-complete-evidence.json
target/link-forum-03-forum-index-search-ordering-visibility-evidence.json
target/forum-search-link-forum-03-translation-moderation-evidence.json
target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json
target/forum-search-link-forum-03-topic-move-evidence.json
```

The D18 and D19 contracts and canonical Forum plan must also be the exact current
repository bytes.

Missing files, mixed commits, copied fixtures, hand-edited evidence and skipped
runtime results are rejected.

## Fail-closed validation

Before writing the packet, the builder checks:

1. D19 has the exact pending machine-contract identity, paths, output boundary
   and LINK-only proposed transition;
2. D18 has the exact pending review-contract identity and still requires a
   separate canonical-source pull request;
3. the D18 candidate is approved, targets only `LINK-FORUM-03`, belongs to the
   current `HEAD` and contains bounded reviewer and retention fields;
4. the candidate retained digest equals the exact complete D17 artifact bytes;
5. the complete artifact retains its exact review-pending status, canonical
   runtime coverage, six scenarios and current source commit;
6. the D18 review-contract digest in the candidate equals the current D18
   contract bytes;
7. the candidate canonical-plan digest equals the exact current plan bytes and
   records `FORUM-21` as `planned`, `FORUM-23` as `in_progress` and
   `LINK-FORUM-03` as `planned`;
8. all four D13-D16 source records match the exact retained artifact path, task,
   contract, source commit, generation time, SHA-256, byte length and scenario
   identity;
9. all D18 validation assertions remain true;
10. the current plan contains the exact planned `LINK-FORUM-03` row once and the
    proposed done row zero times;
11. the packet is written only after validation by same-directory atomic rename.

The builder accepts no command-line arguments, source-commit override,
missing-artifact mode, skipped-result acceptance or static fallback.

## Proposed ledger transition

The exact current row is:

```text
| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |
```

The packet proposes:

```text
| `LINK-FORUM-03` | `done` | D13-D18 provide reviewed and retained Forum/index/search ordering, recovery, multilingual, moderation, private/trusted exclusion and topic-move evidence. |
```

D19 does not perform this replacement.

## Required completion record

The separate canonical-source pull request must update the ledger and completion
evidence together. It must record:

- the retained D18 promotion-candidate path and digest;
- the complete D17 artifact path and digest;
- the four retained D13-D16 source artifacts;
- all six canonical scenario identities;
- reviewer identity and review time;
- immutable retention reference and retained complete-artifact SHA-256;
- that `FORUM-21` remains `planned`;
- that `FORUM-23` remains `in_progress`;
- that `LINK-FORUM-03` is the only proposed status change.

The canonical pull request must revalidate the packet against its exact `HEAD`.
A stale packet cannot authorize a plan change after unrelated plan edits.

## Maintainer order

After executing and retaining the full D13-D18 chain, run:

```bash
node scripts/verify/verify-link-forum-03-canonical-promotion-packet.mjs
node scripts/evidence/prepare-link-forum-03-canonical-promotion-packet.mjs
```

Then open a separate canonical-source pull request using the generated packet as
review input. Do not copy the proposed row without also carrying the retained
candidate and completion evidence.

## Canonical boundary

The packet may support only:

```text
LINK-FORUM-03: planned -> done
```

It explicitly records:

```text
canonical_source_mutated_by_builder = false
promotes_forum_21 = false
promotes_forum_23 = false
```

`FORUM-21` remains an independent move/merge/split/fork owner-workflow task.
`FORUM-23` retains broader unfinished Search product scope.

## Deliberate boundary

D19 adds a contract, packet builder, verifier and handoff only. It changes no
Rust production code, migration, event schema, transport, runtime flag,
dependency, `Cargo.toml`, `Cargo.lock`, D0-D18 artifact schema or canonical task
status.

No command above was run by the implementation agent, per maintainer request.
