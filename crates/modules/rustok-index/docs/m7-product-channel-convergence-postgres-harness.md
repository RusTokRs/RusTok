# M7 Product visibility / Channel identity convergence PostgreSQL harness

Status: `source_ready_execution_pending`.

## Purpose

This retained PostgreSQL packet covers the cross-owner convergence state machine that follows the
materialized Product query-freshness packet. It uses the production Product and Channel migration
chains, Product-owned convergence storage, the selected distribution worker registered through the
generic ModuleWork runtime, the real Product replay source, generic Index mutation storage, persisted
schema readiness, and the canonical shared query runtime.

The packet is source-ready but has not been executed or admitted by the implementation agent.

## Production runtime path

`crates/rustok-distribution/tests/product_channel_convergence_postgres.rs` builds one selected runtime
with Index, Channel, and Product. It retrieves `ModuleWorkRegistrations` from the same distribution
composition used by the host and registers them into two independent `ModuleWorkScheduler` hosts with
separate PostgreSQL connections.

The test does **not** construct the private distribution resolver or convergence adapter. Work executes
only through `ModuleWorkScheduler::run_once`. The one direct owner call is the public
`ProductSalesChannelIndexRelationConvergenceStore::claim` used to simulate a host that has durably
claimed work and then crashed before handler execution; this is the same Product-owned lease operation
used by the registered ModuleWork source.

Scheduler draining is bounded by an explicit maximum iteration count. There is no worker-local infinite
loop.

## Scenario 1: multi-host lease exclusion and restart reclaim

Initial Channel inserts advance the real tenant Channel identity generation. Initial Product inserts
append the real Product-owned visibility convergence requests.

1. Host A claims the first exact Product request with a one-second lease through the public Product
   convergence store.
2. Host B's independently registered scheduler runs while that lease is active and must execute zero
   work.
3. The packet retains the lease token and attempt count from Product-owned state.
4. After bounded lease expiry, Host B runs the real registered worker and must reclaim/complete that
   same durable request.
5. Product-owned state must advance exactly to the retained request sequence, clear the lease, and show
   attempt count two.
6. Host B then drains the remaining initial requests and the baseline Channel-generation sweep through
   bounded `run_once` calls.

This proves durable work ownership survives loss of one host and another host resumes without resetting
visibility cursor or tenant Channel checkpoint.

## Scenario 2: rejected Product isolation

Products are ordered so a malformed Product precedes a later valid Product in Channel sweep order. The
malformed Product uses non-canonical Product visibility (`" Alpha "`) and therefore remains fail-closed.

After the initial exact requests and baseline sweep:

- the malformed Product must have no relation snapshot and no freshness witness;
- the later valid Product must have a freshness witness at the completed tenant Channel generation;
- the tenant convergence state must still reach a completed generation checkpoint.

The same assertions are repeated after a later Channel-generation sweep. This proves rejected Product
owner data does not head-of-line block valid Product convergence.

## Scenario 3: Product visibility alpha -> beta after source read

A restricted Product initially resolves only Channel `alpha`.

1. The real Product source reads a valid mutation and retains it in memory.
2. Product metadata changes from allowed slug `alpha` to `beta`; real Product revision/projection and
   visibility-request triggers advance owner state.
3. The previously produced mutation is applied through `PostgresMutationStore` and is confirmed
   physically present in `index_entities`.
4. Canonical Product query admission must return zero rows while owner visibility/projection is newer.
5. Host A's real scheduler drains the exact visibility request.
6. Relation membership must change from alpha UUID to beta UUID and both `relation_epoch` and
   `projection_epoch` must advance.
7. The old Index row must remain query-inadmissible after convergence because its materialized
   projection is still old.
8. Loading/applying the current Product mutation makes the Product query-visible again.

This ties visibility convergence and the materialized query fence together without adding another
Product clock.

## Scenario 4: Channel generation change with unchanged UUID membership

The unrestricted Product contains both Channel UUIDs. Renaming Channel `alpha` to `alpha-renamed`
advances the real Channel identity generation but leaves the unrestricted UUID set unchanged.

Before convergence the existing materialized Product must be query-inadmissible because its witness has
the previous Channel generation. After a real tenant sweep:

- unrestricted `relation_epoch` is unchanged;
- unrestricted `projection_epoch` is unchanged;
- freshness witness advances to the new Channel generation;
- the exact same already-materialized Product row becomes query-admissible again without a new Index
  mutation.

The later valid Product must also receive the new freshness generation even though the malformed Product
appears before it in the sweep.

This is the key proof that freshness-only Channel identity changes do not fabricate relation or Product
mutation epochs.

## Scenario 5: Channel identity change with changed membership

The restricted Product is now allowed only slug `beta` and has been materialized with beta membership.
Renaming Channel `beta` to `beta-renamed` advances Channel generation and makes the restricted resolved
membership empty.

After the real sweep:

- restricted `relation_epoch` advances;
- restricted `projection_epoch` advances;
- restricted membership is empty;
- the previously materialized restricted Product remains query-inadmissible;
- unrestricted Product relation/projection remain unchanged and its existing Index row is admitted once
  its freshness witness reaches the new generation.

Only after the current restricted Product projection is loaded/applied does that Product become
query-visible again. Tenant convergence state must finish with the newest Channel generation, no sweep
cursor, and no lease.

## Deliberate scope boundary

This packet covers automatic convergence, multi-host lease exclusion/reclaim, rejected Product
isolation, Product visibility change, Channel generation with unchanged membership, Channel generation
with changed membership, and interaction with Product root query admission.

It does not claim:

- production event-contract digest admission or typed Product Index events;
- linked ProductVariant/SalesChannel target materialization equivalence;
- Storefront traffic cutover;
- successful execution of this retained source packet.

Those remain separate admission gates. No new Product schema, relation copy, or freshness clock is
introduced by this packet.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
cargo test -p rustok-distribution --features mod-product --test product_channel_convergence_postgres -- --nocapture
node scripts/verify/verify-index-product-channel-convergence-postgres-harness.mjs
node scripts/verify/verify-index-product-channel-relation-convergence.mjs
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-channel --all-targets
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
