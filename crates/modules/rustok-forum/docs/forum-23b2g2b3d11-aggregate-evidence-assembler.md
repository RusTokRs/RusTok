# FORUM-23B2G2B3D11 aggregate evidence assembler

## Status

`source_ready_maintainer_execution_pending`

This slice adds the fail-closed assembly boundary for the aggregate
`FORUM-23B2G2B3D0` runtime artifact. It does not execute any Forum, Search,
PostgreSQL, Iggy, restart, poison, DLQ or multi-process scenario and does not
promote D0 to runtime-complete.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-aggregate-evidence-assembler.json
```

The assembler is:

```text
scripts/evidence/assemble-forum-search-versioned-invalidation-runtime-evidence.mjs
```

The retained output path remains:

```text
target/forum-search-versioned-invalidation-runtime-evidence.json
```

## Why assembly is a separate gate

D2 through D10 deliberately generate separate executable artifacts. A maintainer
may run one proof, change source, and later run another proof. Merely finding nine
JSON files under `target/` therefore cannot establish one reviewed runtime
baseline.

D11 requires every subproof to be regenerated from one exact source commit and
refuses to combine mixed or stale evidence. This preserves the D0 requirement
that all ten frozen scenarios describe one source tree rather than a collection
of unrelated successful runs.

## Required input set

The assembler requires all nine executable artifacts:

- D2 PostgreSQL ingress and legacy/typed duplicate evidence;
- D3 acknowledgement failure and consumer restart evidence;
- D4 raw poison, durable receipt and deterministic DLQ evidence;
- D5 semantic inbox-identity poison evidence;
- D6 missing-delivery owner-ledger repair evidence;
- D7 multi-process lock, checkpoint and scan-cursor evidence;
- D8 deletion, richer ACL and storefront fail-closed evidence;
- D9 Search-disabled owner continuity and late recovery evidence;
- D10 correlated normal owner delivery through Iggy, projection, checkpoint and
  storefront visibility.

D2 supplies both frozen duplicate scenarios. D10 supplies the frozen successful
`normal_delivery` scenario. The remaining artifacts supply one frozen scenario
each.

## Fail-closed validation

Before any output file is written, the assembler requires:

1. every input path to exist and contain valid JSON;
2. exact contract and task identities for D2 through D10;
3. exact scenario order and membership for each input artifact;
4. every scenario result to equal `passed` and contain non-empty facts;
5. `database_backend` to equal `postgresql` for every input;
6. all nine `source_commit` values to be one lowercase forty-character SHA;
7. that shared SHA to equal `git rev-parse HEAD` at assembly time;
8. every Iggy-backed artifact to report `outbox_iggy`, consumer group
   `rustok-search-forum-projection-v1`, topic `domain` and a non-empty stream;
9. the D0 parent contract to retain the exact ten frozen scenarios and every
   required aggregate field;
10. every D2 through D10 artifact path to remain registered in D0.

Failure at any step throws before output creation or replacement. There is no
skip mode, partial mode, best-effort mode, alternate source-commit argument or
static fixture fallback.

## Aggregate artifact

After complete validation, the assembler writes one JSON document by atomic
rename. It records:

- the exact source commit and assembly time;
- PostgreSQL, `outbox_iggy`, canonical consumer-group and topic identity;
- all ten frozen scenario results, preserving each source artifact's facts;
- grouped owner-revision, root/typed identity, inbox, ingest-sequence,
  checkpoint, poison, DLQ and storefront evidence;
- D2 supporting typed-ingress and semantic-collision facts;
- every source artifact's task, contract, path, generated time, byte length and
  SHA-256 digest;
- the D0 parent-contract SHA-256 digest and assembly invariants.

The grouped fields retain source attribution rather than inventing a normalized
fact that an individual executable proof did not emit.

## Maintainer order

Run the D2 through D10 static verifiers and executable commands from the D0
contract on the same checked-out commit. Confirm that each expected artifact was
created; skipped tests do not create acceptable evidence. Then run:

```bash
node scripts/verify/verify-forum-search-versioned-invalidation-aggregate-evidence-assembler.mjs
node scripts/evidence/assemble-forum-search-versioned-invalidation-runtime-evidence.mjs
```

A later retention/review step may inspect the generated aggregate artifact and
only then decide whether the canonical D0 status can be promoted. The assembler
itself does not edit the D0 contract, the Forum plan or `LINK-FORUM-03`.

## Compatibility

D11 adds scripts, documentation and machine contracts only. It changes no Rust
production path, migration, event schema or digest, DTO, runtime flag,
dependency, `Cargo.toml` or `Cargo.lock` entry.

No command above was run by the implementation agent, per maintainer request.
