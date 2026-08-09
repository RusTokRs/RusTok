# Event contract digest admission

Status: `product_index_family_digest_admitted_maintainer_reverify_pending`.

## Purpose

`crates/rustok-events/contracts/event-contract-digests.json` is a reviewed release artifact. The repository-owned
Rust generator is the sole hashing authority; digest values must not be reconstructed, guessed or hand-authored.

The previously stale baseline was regenerated and admitted through PR #3390. `ProductIndexRefreshEvent` was then
merged through PR #3401 with the typed Product locale/ProductVariant refresh family, while the Product-family
canonical digest remained follow-up work.

## Maintainer-provided Product-family generator output

After pulling `main`, the maintainer reported:

- `cargo run --locked -p rustok-events --example event_contract_digests -- --write` completed successfully;
- the generated artifact changed the registry and typed contract payload/envelope digests while leaving root event
  and root envelope digests unchanged;
- `node scripts/verify/verify-index-product-refresh-event-family.mjs` passed with
  `typed_family=true canonical_factory=true digest_regenerated=true`;
- `node scripts/verify/verify-event-contract-digest-admission.mjs` failed before generation because its documentation
  markers still expected the pre-admission status text.

This follow-up admits exactly the maintainer-provided generated JSON and updates that stale source verifier. The
reported command bundle did not include `git rev-parse HEAD`, so one post-merge exact-SHA rerun remains required
before the M5 digest gate is considered fully closed.

## Workflow

The **Event contract digest admission** GitHub Actions workflow remains a read-only, manually dispatched review
packet path.

It supports two modes:

- `generate_patch`: run the canonical generator and retain the generated artifact and exact patch;
- `verify`: run the same generator and fail when the committed artifact differs from canonical output.

The generator command is exactly:

```bash
cargo run --locked -p rustok-events --example event_contract_digests -- --write
```

No alternative hashing implementation, copied digest, hand-authored JSON Schema or parallel generator is admitted.

## Review packet

When the workflow is used, it archives:

- `event-contract-digests.committed.json`;
- `event-contract-digests.generated.json`;
- `event-contract-digests.patch`;
- `manifest.env`;
- `SHA256SUMS`.

The packet is diagnostic and reviewable. It does not automatically write the repository.

## Write safety

The workflow has only `contents: read` permission and checks out with `persist-credentials: false`. It does not
commit, push or open a pull request and has no scheduled, push, pull-request or workflow-run trigger.

## Product Index dependency

The stale-baseline gate is complete and `ProductIndexRefreshEvent` is merged. The Product-family digest artifact is
admitted by this follow-up from maintainer-generated output.

The remaining closeout is an exact post-merge local rerun on the new `main` head:

```bash
git rev-parse HEAD
node scripts/verify/verify-event-contract-digest-admission.mjs
cargo run --locked -p rustok-events --example event_contract_digests -- --write
git diff --exit-code -- crates/rustok-events/contracts/event-contract-digests.json
node scripts/verify/verify-index-product-refresh-event-family.mjs
```

Only after that reports a passing admission verifier, an empty canonical digest diff and a passing Product-family
verifier does M5 advance to typed Product/ProductVariant delivery into `IndexSourceRefreshEventWorker`.

## Current Product-family boundary

The Product family remains narrow:

- locale target: `product_id`, `locale`, `source_version`;
- variant target: `product_id`, `variant_id`, `source_version`;
- envelope identity/correlation: Product ledger `refresh_id`;
- envelope causation: Product ledger `root_event_id`;
- tenant and actor remain envelope metadata.

No legacy/v2 payload, compatibility decoder or dual publication family is permitted for this repository-owned
pre-release contract.
