# Event contract digest admission

Status: `source_complete_maintainer_execution_pending`.

## Purpose

`crates/rustok-events/contracts/event-contract-digests.json` is a reviewed release artifact, not a value that should be reconstructed by hand. The registry digest is derived from the complete sorted event schema registry, while the root and typed payload/envelope digests depend on the exact Schemars output produced by the pinned Rust toolchain and crate dependencies.

The current repository state contains a typed Reactions family while the committed digest artifact still has the values retained before that family was added. A new Product Index refresh family must not be layered on top of that stale release artifact.

This slice adds a maintainer-owned, manually dispatched admission workflow that runs the existing canonical generator and produces a review packet. It does not change any event family, schema, digest value, or transport profile by itself.

## Workflow

Run **Event contract digest admission** from GitHub Actions on the exact branch or commit being reviewed.

The workflow supports two modes:

- `generate_patch`: run the canonical generator, retain the generated artifact and patch, and complete successfully even when drift exists;
- `verify`: run the same generator and fail when the committed artifact differs from the canonical output.

The generator command is exactly:

```bash
cargo run --locked -p rustok-events --example event_contract_digests -- --write
```

No alternative hashing implementation, copied digest, hand-authored JSON Schema, or unpinned external generator is admitted.

## Review packet

The workflow archives one artifact containing:

- `event-contract-digests.committed.json` — the artifact before generation;
- `event-contract-digests.generated.json` — the canonical generator output;
- `event-contract-digests.patch` — the exact repository diff;
- `manifest.env` — source SHA/ref, pinned toolchain, mode, status, and generator command;
- `SHA256SUMS` — hashes for every packet file.

The packet is diagnostic and reviewable. It is not itself release admission. A maintainer must inspect the patch and commit the generated JSON in a separate PR with the normal event-contract review.

## Write safety

The workflow has only `contents: read` permission and checks out with `persist-credentials: false`.

It does not commit, push, open a pull request, or alter the repository. It has no scheduled, push, pull-request, or workflow-run trigger. Execution is explicit through `workflow_dispatch` only while the repository contains known digest drift.

The workflow deliberately does not use a bot token, repository secret, release credential, or automatic branch mutation. The generated patch remains an artifact until a maintainer admits it intentionally.

## Product Index dependency

The Product Index refresh event family remains blocked until all of the following are complete:

1. run this workflow in `generate_patch` mode on the current reviewed `main` or a dedicated admission branch;
2. inspect the generated/committed JSON pair and patch;
3. commit the canonical generated artifact through a separate PR;
4. run the workflow in `verify` mode on the admitted commit and retain a matching packet;
5. only then add `ProductIndexRefreshEvent` and regenerate the artifact again in the same reviewed wire-contract PR.

The existing Product locale/ProductVariant ledgers, canonical writer, and durable relay step do not bypass this gate.

## Deliberate limits

This slice does not:

- update `event-contract-digests.json`;
- add or modify an event payload;
- run the canonical generator;
- claim that the current artifact is valid;
- enable automatic pull-request enforcement before the known drift is repaired;
- add Product Index routes, consumers, retries, acknowledgements, or runtime evidence;
- alter the concrete Index repair evidence gate.

## Maintainer commands

Equivalent local generation:

```bash
cargo run --locked -p rustok-events --example event_contract_digests -- --write
git diff -- crates/rustok-events/contracts/event-contract-digests.json
```

Source-contract guard:

```bash
node scripts/verify/verify-event-contract-digest-admission.mjs
```

No workflow, Cargo command, test, verifier, or CI job was executed by the implementation agent.
