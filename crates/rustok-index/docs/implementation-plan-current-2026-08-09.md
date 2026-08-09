# Current `rustok-index` implementation plan — 2026-08-09

Status: `m5_product_refresh_family_and_digest_admitted_maintainer_reverify_pending`.

This overlay supersedes `implementation-plan-current-2026-08-08.md` as the live execution cursor. The older file
remains detailed architecture/source history.

Rechecked against `main@de4fd6cc358e9112c372565a064fbc862656252e`. `ProductIndexRefreshEvent` is already merged
through PR #3401. Subsequent mainline work does not reopen the Product refresh-family source shape, M6 repair source
boundary, M7 Storefront evidence gate or partition-replay rule.

## M5 — Product Index typed refresh family

The stale event-contract baseline gate is complete and the typed Product refresh family is merged with exactly two
schema-v1 event types:

```text
product.index.locale_refresh_requested
product.index.variant_refresh_requested
```

Payload ownership remains narrow:

- locale: `product_id`, `locale`, `source_version`;
- variant: `product_id`, `variant_id`, `source_version`.

The Product ledger/canonical writer owns envelope identity, tenant, actor and causation:

```text
id = correlation_id = refresh_id
causation_id = root_event_id
```

`CanonicalProductIndexRefreshEventFactory` binds immutable Product locale/ProductVariant ledger rows to the sealed
family. No alternate JSON/event-name route or compatibility family is admitted.

### Maintainer follow-up after #3401

The maintainer pulled `main` and reported:

- `verify-event-contract-digest-admission.mjs` failed only because it still required the obsolete
  `source_complete_maintainer_execution_pending` documentation marker;
- the canonical `event_contract_digests -- --write` generator completed successfully and produced the submitted
  Product-family digest update;
- `verify-index-product-refresh-event-family.mjs` passed with
  `typed_family=true canonical_factory=true digest_regenerated=true`.

This follow-up admits exactly that maintainer-generated digest artifact and repairs the stale verifier/documentation
contract. The supplied execution transcript did not include `git rev-parse HEAD`, so the final exact-SHA admission
closeout remains a post-merge maintainer rerun rather than an implementation-agent claim.

### M5 closeout gate

On the new merged `main`, run:

```bash
git rev-parse HEAD
node scripts/verify/verify-event-contract-digest-admission.mjs
cargo run --locked -p rustok-events --example event_contract_digests -- --write
git diff --exit-code -- crates/rustok-events/contracts/event-contract-digests.json
node scripts/verify/verify-index-product-refresh-event-family.mjs
```

If the admission verifier passes, the canonical digest diff is empty and the Product-family verifier passes, the
next independent M5 source boundary opens: Product/ProductVariant typed delivery into the existing generic
`IndexSourceRefreshEventWorker`, including exact event-route registration, canonical target-key decoding and
commit-before-ack consumption.

Do not start that source boundary before this exact-SHA closeout is reported.

## M6 — concrete repair PostgreSQL evidence

M6 source remains complete and execution/admission-gated. The latest maintainer attempt stopped before PostgreSQL
work because neither `RUSTOK_INDEX_TEST_DATABASE_URL` nor `DATABASE_URL` was present. No evidence packet or logs
were created, so this remains an environment/configuration blocker rather than a source defect.

The next M6 action is an exact rerun with a real opt-in PostgreSQL URL, followed by the retained evidence/verifier/
Cargo command set in `m6-repair-retained-evidence-admission.md`. Do not add another M6 source slice unless that
execution exposes a concrete source failure.

## M7 — Product Storefront

M7 remains evidence/admission-gated. Mounted Storefront stays Product owner-native. Existing timeout, Product key-4
promotion/restart, current-key core/EAV/collation, deployment collation, stale/readiness/restart evidence must be
executed/admitted before serving traffic composition changes.

## Partition replay

Partition replay remains blocked until a real source contract filters the requested partition before pagination.
Do not populate `partition_key` without that source capability.

## Compatibility rule

Repository-owned pre-release contracts have one current shape. Do not introduce legacy readers, v2 families,
fallback decoders, dual formats or compatibility publication paths unless an explicit external compatibility bridge
is approved.

## Validation boundary

The implementation agent performed static GitHub source/diff review only for this follow-up and did not rerun Rust
tests, Node verifiers, Cargo checks, formatting, PostgreSQL execution, workflows or CI. Generator and Product-family
verifier results above are maintainer-provided local execution results.
