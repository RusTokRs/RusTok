# M7 Product / Channel identity-transition PostgreSQL harness

Status: `source_ready_execution_pending`.

## Purpose

This retained PostgreSQL packet follows the Product materialized-freshness and Product/Channel
convergence packets. It focuses on Channel identity transitions not covered by slug-only convergence
scenarios: Channel create, Channel delete, tenant move, and delete + recreate of the same Channel
identity before convergence.

The packet uses the production Channel/Product/Index migration chains, selected distribution runtime,
Product-owned convergence state, generic `ModuleWorkScheduler`, real Product replay source, generic
Index mutation storage, persisted tenant schema readiness, and the canonical Product query-admission
fence. It is source-ready but has not been executed or admitted by the implementation agent.

## Baseline

Two tenants are created:

- tenant A has unrestricted Product A and Channel `alpha`;
- tenant B has unrestricted Product B and no Channel identity history.

Initial Product INSERT requests and the baseline Channel-generation sweep run through the real generic
scheduler. The packet requires:

- tenant A generation to be positive;
- tenant B generation to be exactly zero;
- completed convergence checkpoints for both tenants;
- Product A membership `[alpha UUID]`;
- Product B membership `[]`.

Both current Product projections are then materialized through the real Product source and
`PostgresMutationStore` and must be query-visible.

## Channel create

Channel `beta` is inserted into tenant A through the real Channel owner table.

Expected retained evidence:

1. tenant A Channel identity generation advances;
2. Product A becomes query-inadmissible immediately because its freshness witness is older;
3. tenant B Product remains query-visible;
4. the real convergence sweep changes Product A membership from `[alpha]` to `[alpha, beta]`;
5. Product A `relation_epoch` and `projection_epoch` both advance;
6. the old materialized Product A row remains query-inadmissible after convergence;
7. only the current Product projection mutation restores query authority.

This proves Channel create is a true relation-membership mutation for unrestricted Products.

## Channel delete

The newly created `beta` Channel is deleted from tenant A.

Expected retained evidence mirrors create:

- tenant A generation advances;
- Product A is hidden before convergence;
- convergence returns membership to `[alpha]`;
- relation/projection both advance;
- the old Index row stays hidden;
- applying the current Product projection restores query authority.

This proves a retained stale Product row cannot survive a Channel delete merely because its old link
rows remain physically stored.

## Channel tenant move

Channel `alpha` is moved from tenant A to tenant B by updating the Channel owner `tenant_id`.

The Channel owner trigger is required to advance **both tenant generations** under its deterministic
lock order. Before convergence, Product A and Product B must both be query-inadmissible because each
tenant now has a newer Channel identity watermark.

After the real scheduler drains both due tenants:

- Product A membership becomes empty;
- Product B membership becomes `[alpha UUID]`;
- Product A relation/projection advance;
- Product B relation/projection advance;
- both previously materialized Product rows remain hidden until their current Product projection
  mutations are applied.

This proves tenant movement invalidates both sides of the cross-owner relation and does not collapse two
owner changes into one tenant-local watermark.

## Channel delete + recreate before convergence

After tenant move, Product B is current and materialized with `[alpha UUID]`. The packet then:

1. deletes Channel `alpha` from tenant B;
2. records the resulting generation;
3. recreates the same Channel UUID and canonical slug in tenant B **before** running convergence;
4. requires a second Channel generation advance;
5. requires the old Product B row to be query-inadmissible while freshness is stale.

When the scheduler finally converges, current resolved membership is again exactly `[alpha UUID]`.
Therefore the correct result is freshness-only convergence:

- `relation_epoch` does not advance;
- `projection_epoch` does not advance;
- freshness witness advances to the newest Channel generation;
- the physically existing Product B `index_entities.source_version` remains unchanged;
- the **same materialized Product row** becomes query-admissible again without a Product mutation.

This is the important delete/recreate proof: Channel identity generations preserve the ordering fence
even when transient identity loss is repaired before the resolver observes it, while the relation and
Product mutation clocks remain semantic membership clocks instead of generic invalidation counters.

Tenant A is also required to remain query-visible throughout the tenant-B delete/recreate sequence,
proving tenant scope is not widened.

## Deliberate scope

Together with the previous retained packets, source now exists for:

- Product scalar/locale source-read -> delayed-apply races;
- Product visibility changes;
- Channel slug-generation changes with unchanged and changed membership;
- multi-host convergence lease expiry/reclaim;
- rejected Product isolation;
- Channel create/delete;
- Channel tenant move;
- Channel delete + recreate with net-unchanged membership.

This packet does not claim successful runtime execution. It also does not close typed Product event
admission, ProductVariant/SalesChannel linked-target materialization parity, or Storefront cutover.
Those remain separate admission gates. No new Product schema, relation copy, or freshness clock is
introduced.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
cargo test -p rustok-distribution --features mod-product --test product_channel_identity_transitions_postgres -- --nocapture
node scripts/verify/verify-index-product-channel-identity-transitions-postgres-harness.mjs
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
