# `rustok-index` maintainer unblock handoff — 2026-08-09

Status: `source_gates_complete_maintainer_execution_required`.

This is the operational continuation of the current Index implementation cursor after Targeted GraphQL source completion. It does not create a new source-code feature. Its purpose is to make the remaining owner gates executable in a fixed order without weakening them.

Rechecked against `main@2525e13078a4fb61190f84864f3d3ceb7b0c3726`. Mainline changes after the post-Targeted cursor update are Blog/Commerce/Forum only and do not modify `rustok-index` or Index replay transport paths.

## Priority 1 — M6 concrete repair PostgreSQL admission

The current primary Index owner gate remains concrete repair PostgreSQL execution/admission. The canonical contract is:

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

Review the redacted logs and packet before committing them. Do not hand-edit the packet or logs. Source work should resume only if this execution exposes a concrete defect, or after the retained evidence is admitted and the current plan is rechecked.

Canonical detail: `m6-repair-retained-evidence-admission.md`.

## Priority 2 — M5 canonical event-contract digest admission

The Product Index typed event family remains blocked by the stale committed event-contract digest artifact. The canonical workflow is **Event contract digest admission** and must be run explicitly on the reviewed commit.

Admission sequence:

1. run the workflow in `generate_patch` mode on reviewed `main` or a dedicated admission branch;
2. inspect the committed/generated JSON pair and exact patch;
3. commit the canonical generated `crates/rustok-events/contracts/event-contract-digests.json` in a separate reviewed PR;
4. run the workflow in `verify` mode on that admitted commit and retain a matching packet;
5. only then add `ProductIndexRefreshEvent` and regenerate the digest artifact again in that same reviewed wire-contract PR.

The canonical generator used by the workflow is:

```bash
cargo run --locked -p rustok-events --example event_contract_digests -- --write
```

Do not replace this with hand-authored hashes, copied schema JSON, or a parallel compatibility event family.

Canonical detail: `../../rustok-events/docs/event-contract-digest-admission.md`.

## Priority 3 — M7 Product Storefront evidence gates

Mounted Storefront must remain owner-native until retained evidence is executed/admitted. The source-complete gates remain, in order:

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

Do not add a `partition_key` dimension merely to advance the checklist. Partition replay remains blocked until a real source contract can filter the requested partition before pagination. No current Index source provides that contract.

## Resume rule

After any maintainer-run packet is executed:

- if it fails, resume source work only against the concrete failure and preserve the existing ownership boundaries;
- if it passes and is admitted, recheck current `main`, update the current implementation cursor, and advance only the newly unblocked milestone;
- do not add legacy/version-family compatibility for repository-owned pre-release contracts;
- do not infer admission from source inspection alone.

## Source guard

`scripts/verify/verify-index-maintainer-unblock-handoff.mjs` locks this handoff to the canonical M6 execution command/output blocks, the pending M5 digest sequence, the fail-closed M7 Storefront boundary, the current Index cursor, and this README-visible handoff surface. It must be updated together with the canonical gate documents when an owner execution/admission result legitimately changes one of those states.

This handoff itself executes no Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or `git diff --check`.
