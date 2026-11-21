# poison duplicate moving window scan checkpoint

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-iggy-dlq-duplicate-moving-window-scan.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `poison duplicate moving window scan checkpoint`.
- Verifier: `scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- required marker: Profiles still never authorizes
- required marker: private per-partition next offsets
- required marker: explicit reset
- required marker: restart-safe progress
- required marker: server observer mode is source-complete
