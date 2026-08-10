# Current `rustok-index` implementation plan — 2026-08-09

Status: `m5_product_refresh_family_digest_clean_ci_gate_installation_pending`.

This overlay supersedes `implementation-plan-current-2026-08-08.md` as the live execution cursor. The older file
remains detailed architecture/source history.

Rechecked against `main@3b1ba79619a9c37f7bc90fb773843c7287d2d4ff`. Mainline changes after the maintainer exact-SHA
run are Commerce, Forum, Page Builder, Payment and narrow Product storefront-read work; they do not change the
Product Index refresh event family, canonical digest generator, Index source-refresh worker or M6/M7 source gates.

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

### Maintainer exact-SHA closeout evidence

On `main@535c6d4a3aca4412c27c69453e08c6942c281ac9`, the maintainer reported:

- `cargo run --locked -p rustok-events --example event_contract_digests -- --write` completed successfully;
- `git diff --exit-code -- crates/rustok-events/contracts/event-contract-digests.json` returned exit code 0;
- `verify-index-product-refresh-event-family.mjs` passed with
  `typed_family=true canonical_factory=true digest_regenerated=true`;
- `verify-event-contract-digest-admission.mjs` failed only on one case-sensitive prose marker.

PR #3434 removed that brittle prose marker without weakening the actual workflow/admission checks. The generated
Product-family digest is therefore clean at the recorded exact SHA; the remaining source-process gap is to make the
same checks automatically repeatable on every relevant PR instead of requiring a maintainer local command bundle.

### Focused Index contract CI gate

This slice adds `.github/workflows/index-contract-ci.yml` as a read-only path-scoped PR/push gate for changes under
`rustok-events`, `rustok-product`, `rustok-index`, their focused verifier scripts, workspace manifests and the two
related workflows.

The gate requires:

```text
verify-index-contract-ci.mjs
verify-event-contract-digest-admission.mjs
verify-index-product-refresh-event-family.mjs
canonical event_contract_digests regeneration with an empty git diff
cargo check --locked -p rustok-events -p rustok-product -p rustok-index --all-targets
cargo test --locked -p rustok-events --test canonical_contracts
```

The workflow has `contents: read`, disables persisted checkout credentials and contains no repository mutation path.
Its stable aggregate job is `Index Contract Gate` so future branch protection can require one focused status rather
than the entire workspace CI matrix.

The first passing pull-request execution of this workflow closes the M5 family/digest admission gate. After that,
the next independent M5 source boundary is Product/ProductVariant typed delivery into the existing generic
`IndexSourceRefreshEventWorker`, including exact event-route registration, canonical target-key decoding and
commit-before-ack consumption.

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

The implementation agent performs static GitHub source/diff review for this CI slice. The new workflow itself is the
execution boundary: its pull-request run must prove the source verifiers, canonical digest drift check and focused
Cargo checks before merge. No PostgreSQL M6/M7 evidence is claimed by this CI gate.
