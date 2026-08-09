# `rustok-index` maintainer unblock handoff — 2026-08-09

Status: `m5_baseline_verified_product_family_digest_pending_m6_m7_execution_required`.

This is the operational continuation of the Index cursor after Targeted GraphQL source completion and the
canonical M5 digest-baseline admission. It keeps execution-owned gates explicit without manufacturing new source
work.

Rechecked against `main@96c8886738c3df22e176c808fd04d27d8eedb552`. The mainline change after the digest
admission merge is Page Builder/Telemetry-only and does not alter Product, Events or Index refresh wire paths.

## Priority 1 — M6 concrete repair PostgreSQL admission

The primary execution-owned Index gate remains concrete repair PostgreSQL execution/admission. The canonical
contract is:

```text
crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution-contract.json
```

Run from the exact clean commit intended for admission:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  node scripts/evidence/capture-index-repair-postgres.mjs

node scripts/verify/verify-index-repair-retained-evidence.mjs
node scripts/verify/verify-index-repair-execution-postgres-harness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

A successful capture must produce the complete retained set together:

```text
crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution.json
crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution.stdout.log
crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution.stderr.log
```

The latest maintainer attempt stopped before PostgreSQL execution because no opt-in database URL was configured.
No packet or log was created. This does not open a source repair slice; rerun with the real local PostgreSQL URL.

Review the redacted logs and packet before committing them. Do not hand-edit the packet or logs. Source work
should resume only if execution exposes a concrete defect, or after retained evidence is admitted and the current
plan is rechecked.

Canonical detail: `m6-repair-retained-evidence-admission.md`.

## Priority 2 — M5 Product Index typed event family

The stale canonical digest baseline no longer blocks source work.

The generated baseline was admitted through PR #3390 at
`7983092f96e14c002c57451709de936e40c01356`. On that exact admitted commit the maintainer then reported:

1. `verify-event-contract-digest-admission.mjs` passed;
2. the canonical `event_contract_digests -- --write` generator passed;
3. the digest diff was empty.

No GitHub Actions verification packet is claimed; the exact-SHA local canonical execution is the supplied
maintainer result.

`ProductIndexRefreshEvent` is therefore the active M5 source boundary. The reviewed family must remain narrow:

- `product.index.locale_refresh_requested` -> `product_id`, `locale`, `source_version`;
- `product.index.variant_refresh_requested` -> `product_id`, `variant_id`, `source_version`;
- `id = correlation_id = refresh_id` remains Product ledger/writer ownership;
- `causation_id = root_event_id` remains Product ledger/writer ownership.

The current family branch must run the canonical generator again and commit the newly generated
`crates/rustok-events/contracts/event-contract-digests.json` in the **same wire-contract PR before merge**.
Do not hand-author those new hashes.

Only after that family/digest PR is admitted may M5 advance to Product/ProductVariant typed route registration and
commit-before-ack delivery into the existing generic Index source-refresh worker.

Canonical detail: `../../rustok-events/docs/event-contract-digest-admission.md` and
`../../rustok-product/docs/index-refresh-event-family.md`.

## Priority 3 — M7 Product Storefront evidence gates

Mounted Storefront must remain owner-native until retained evidence is executed/admitted. The source-complete
gates remain, in order:

1. deterministic budgeted timeout evidence;
2. Product key-4 promotion/restart PostgreSQL packet;
3. current-key Storefront core/EAV/collation and actualized retained Product PostgreSQL packets;
4. deployment-specific owner/default-vs-Index `COLLATE "C"` admission;
5. stale locale/readiness/admission/restart evidence;
6. only then a real tenant stage/rebuild/`register_current`, followed last by any eligible traffic switch.

Relevant retained source guards include:

```text
scripts/verify/verify-index-product-storefront-budgeted-execution-evidence.mjs
scripts/verify/verify-index-product-current-schema-promotion.mjs
scripts/verify/verify-index-product-current-schema-promotion-postgres-packet.mjs
scripts/verify/verify-index-product-storefront-parity-gate.mjs
```

Canonical detail: `m7-product-storefront-parity-gate.md` and `m7-product-current-schema-promotion.md`.

## Still blocked — partition replay

Do not add a `partition_key` dimension merely to advance the checklist. Partition replay remains blocked until a
real source contract can filter the requested partition before pagination. No current Index source provides that
contract.

## Resume rule

- M5: finish the current family with its canonical generated digest before starting delivery/route work;
- M6: if PostgreSQL evidence fails, resume source work only against the concrete failure; if it passes and is
  admitted, recheck current `main` before advancing;
- M7: do not move Storefront traffic from source inspection alone;
- do not add legacy/version-family compatibility for repository-owned pre-release contracts;
- do not infer runtime admission from source inspection alone.

## Source guard

`scripts/verify/verify-index-maintainer-unblock-handoff.mjs` must remain synchronized with this handoff, the M6
canonical execution/output blocks, the admitted M5 baseline/current Product-family digest gate, the fail-closed M7
Storefront boundary and the current 2026-08-09 Index cursor.

The implementation agent claims no Rust test, Node verifier, Cargo check, formatting, PostgreSQL scenario,
workflow, CI or `git diff --check` execution on the Product-family review branch.
