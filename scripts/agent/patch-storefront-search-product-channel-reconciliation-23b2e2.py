from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}\n{old}")
    target.write_text(text.replace(old, new, 1))


def replace_all(path: str, old: str, new: str, expected: int) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} replacements, found {count}\n{old}")
    target.write_text(text.replace(old, new))


projector = "crates/rustok-search/src/projector.rs"
search_lib = "crates/rustok-search/src/lib.rs"
server_services = "apps/server/src/services/mod.rs"
server_bootstrap = "apps/server/src/services/server_bootstrap.rs"
forum_plan = "crates/rustok-forum/docs/implementation-plan.md"
search_plan = "crates/rustok-search/docs/implementation-plan.md"
note = "crates/rustok-forum/docs/forum-23b2e2-product-channel-visibility.md"
verifier = "scripts/verify/verify-forum-search-product-channel-visibility.mjs"

# Repair only legacy documents where the projection path is absent. Explicit
# malformed owner data remains projected as JSON null and hidden until owner fix.
replace_all(
    projector,
    "PRODUCT_CHANNEL_VISIBILITY_DRIFT_COUNT_SQL",
    "PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL",
    3,
)
replace_once(
    projector,
    """  AND jsonb_typeof(payload #> '{channel_visibility,allowed_channel_slugs}')
      IS DISTINCT FROM 'array'
""",
    """  AND payload #> '{channel_visibility,allowed_channel_slugs}' IS NULL
""",
)
replace_all(projector, "drift_statement", "legacy_statement", 2)
replace_all(projector, "drift_total", "legacy_total", 2)
replace_once(
    projector,
    """    fn product_channel_visibility_drift_is_fail_closed() {
        assert!(PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL.contains("entity_type = 'product'"));
        assert!(PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL.contains("allowed_channel_slugs"));
        assert!(PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL.contains("IS DISTINCT FROM 'array'"));
    }
""",
    """    fn product_channel_visibility_legacy_projection_is_detected() {
        assert!(PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL.contains("entity_type = 'product'"));
        assert!(PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL.contains("allowed_channel_slugs"));
        assert!(PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL.contains("IS NULL"));
        assert!(!PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL.contains("IS DISTINCT FROM"));
    }
""",
)

# Export the Search-owned bounded reconciler.
replace_once(
    search_lib,
    "pub mod projection_source;\npub mod projector;\n",
    "pub mod projection_source;\nmod product_channel_reconciliation;\npub mod projector;\n",
)
replace_once(
    search_lib,
    """pub use projector::SearchProjector;
pub use ranking::SearchRankingProfile;
""",
    """pub use product_channel_reconciliation::{
    DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT, ProductChannelProjectionReconciler,
    ProductChannelProjectionSweepReport,
};
pub use projector::SearchProjector;
pub use ranking::SearchRankingProfile;
""",
)

# Host startup starts the bounded one-shot repair worker for PostgreSQL runtimes.
replace_once(
    server_services,
    "pub mod search_product_channel_reconciliation;\npub mod server_bootstrap;\n"
    if "pub mod search_product_channel_reconciliation;" in Path(server_services).read_text()
    else "pub mod server_bootstrap;\n",
    "pub mod search_product_channel_reconciliation;\npub mod server_bootstrap;\n",
)
replace_once(
    server_bootstrap,
    """    #[cfg(feature = "mod-forum")]
    crate::services::forum_search_inbox_worker::start_forum_search_inbox_worker_if_ready(
        &runtime_ctx,
    )?;
""",
    """    crate::services::search_product_channel_reconciliation::start_product_channel_projection_reconciliation_if_ready(
        &runtime_ctx,
    )?;

    #[cfg(feature = "mod-forum")]
    crate::services::forum_search_inbox_worker::start_forum_search_inbox_worker_if_ready(
        &runtime_ctx,
    )?;
""",
)

# Correct the canonical plans to describe actual startup wiring and the distinction
# between repairable legacy absence and explicit malformed owner data.
replace_once(
    forum_plan,
    """- Search bootstrap detects tenant Product documents with a missing or malformed
  allowlist projection and runs the existing product-scope rebuild; legacy drift is
  hidden before repair and does not require a database migration or manual backfill;
""",
    """- a Search-owned bounded reconciler discovers tenant Product documents whose
  projection path is absent, and a host background worker runs product-scope rebuilds
  at server startup until no legacy batch remains;
- legacy missing projections are hidden before repair, while malformed explicit
  owner values remain hidden and are not rebuilt forever without an owner fix;
""",
)
replace_once(
    forum_plan,
    """channel allowlist projection, and existing drift is repaired by an automatic
product-scope rebuild during Search bootstrap.
""",
    """channel allowlist projection, and missing legacy projections are repaired by a
bounded PostgreSQL background worker started during server bootstrap.
""",
)
replace_once(
    forum_plan,
    """totals, facets, typo fallback, query-rule pins and document suggestions. Missing or
malformed Product projections remain hidden until the Search-owned product rebuild
repairs them; admin/global Search behavior remains unchanged.
""",
    """totals, facets, typo fallback, query-rule pins and document suggestions. Missing
legacy projections remain hidden until the Search-owned startup worker repairs them;
malformed explicit owner values remain hidden until Product is corrected. Admin/global
Search behavior remains unchanged.
""",
)
replace_once(
    forum_plan,
    "cargo test -p rustok-search product_channel_visibility_drift_is_fail_closed -- --nocapture\n",
    """cargo test -p rustok-search product_channel_visibility_legacy_projection_is_detected -- --nocapture
cargo test -p rustok-search product_channel_reconciliation -- --nocapture
""",
)

replace_once(
    search_plan,
    """suggestions therefore share the trusted channel decision. Existing Product drift
triggers the Search-owned product-scope rebuild, while admin preview/global Search
retain the previous non-storefront path. Runtime evidence remains pending.
""",
    """suggestions therefore share the trusted channel decision. A Search-owned bounded
reconciler and host startup worker repair missing legacy Product projections in
PostgreSQL batches. Malformed explicit owner values remain hidden until Product is
corrected instead of entering an endless rebuild loop. Admin preview/global Search
retain the previous non-storefront path. Runtime evidence remains pending.
""",
)
replace_once(
    search_plan,
    """19. Projected canonical Product channel allowlists, added fail-closed product-scope
    bootstrap repair, and applied one storefront predicate to FTS, typo fallback,
    rows, totals, facets, query-rule pins and document suggestions under
    `FORUM-23B2E2`.
""",
    """19. Projected canonical Product channel allowlists, added bounded startup repair
    for missing legacy projections, and applied one storefront predicate to FTS,
    typo fallback, rows, totals, facets, query-rule pins and document suggestions
    under `FORUM-23B2E2`.
""",
)
replace_once(
    search_plan,
    "- `cargo test -p rustok-search product_channel_visibility_drift_is_fail_closed -- --nocapture`\n",
    """- `cargo test -p rustok-search product_channel_visibility_legacy_projection_is_detected -- --nocapture`
- `cargo test -p rustok-search product_channel_reconciliation -- --nocapture`
""",
)

# Owner note and machine guard bind the executable startup path.
replace_once(
    note,
    """Older Product Search documents do not contain the projected allowlist. They are
already hidden by the fail-closed predicate. During Search bootstrap,
`SearchProjector` counts tenant Product documents whose projected value is missing
or is not an array and runs the existing product-scope rebuild when drift exists.
This is a Search-owned rebuild, not a database migration or a Product write.
""",
    """Older Product Search documents do not contain the projected allowlist and are
already hidden by the fail-closed predicate. A Search-owned reconciler selects a
bounded batch of tenant IDs whose Product projection path is absent. The host starts
its background worker during server bootstrap; each batch runs the existing
product-scope rebuild until no legacy tenant remains.

Explicit malformed Product owner data projects JSON null. It remains hidden and is
not selected repeatedly by the legacy repair worker; Product must correct the owner
value. The repair is a Search-owned rebuild, not a database migration or Product
write.
""",
)
replace_once(
    note,
    """channel visibility projection. Existing canonical Products are repaired through
the product-scope rebuild; no manual backfill is required.

If bootstrap repair has not run yet, old or malformed Product documents remain
hidden rather than becoming visible in every channel. Admin preview/global Search
keeps its previous operator semantics because it does not call the storefront-only
engine method.
""",
    """channel visibility projection. Missing legacy projections are repaired through
the bounded PostgreSQL startup worker; no manual backfill is required when background
workers are enabled.

Before repair, legacy documents remain hidden rather than becoming visible in every
channel. Explicit malformed owner data also remains hidden until Product is fixed.
Admin preview/global Search keeps its previous operator semantics because it does
not call the storefront-only engine method.
""",
)
replace_once(
    note,
    "cargo test -p rustok-search product_channel_visibility_drift_is_fail_closed -- --nocapture\n",
    """cargo test -p rustok-search product_channel_visibility_legacy_projection_is_detected -- --nocapture
cargo test -p rustok-search product_channel_reconciliation -- --nocapture
""",
)

replace_once(
    verifier,
    """  bootstrap: "crates/rustok-search/src/projector.rs",
  engine: "crates/rustok-search/src/pg_engine.rs",
""",
    """  bootstrap: "crates/rustok-search/src/projector.rs",
  reconciler: "crates/rustok-search/src/product_channel_reconciliation.rs",
  serverWorker:
    "apps/server/src/services/search_product_channel_reconciliation.rs",
  serverServices: "apps/server/src/services/mod.rs",
  serverBootstrap: "apps/server/src/services/server_bootstrap.rs",
  engine: "crates/rustok-search/src/pg_engine.rs",
""",
)
replace_once(
    verifier,
    """const bootstrap = read(paths.bootstrap);
const engine = read(paths.engine);
""",
    """const bootstrap = read(paths.bootstrap);
const reconciler = read(paths.reconciler);
const serverWorker = read(paths.serverWorker);
const serverServices = read(paths.serverServices);
const serverBootstrap = read(paths.serverBootstrap);
const engine = read(paths.engine);
""",
)
replace_once(
    verifier,
    """    "PRODUCT_CHANNEL_VISIBILITY_DRIFT_COUNT_SQL",
    "entity_type = 'product'",
    "IS DISTINCT FROM 'array'",
    "self.rebuild_product_scope(tenant_id).await?",
    "product_channel_visibility_drift_is_fail_closed",
""",
    """    "PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL",
    "entity_type = 'product'",
    "IS NULL",
    "self.rebuild_product_scope(tenant_id).await?",
    "product_channel_visibility_legacy_projection_is_detected",
""",
)
replace_once(
    verifier,
    """  paths.bootstrap,
);

requireAll(
  engine,
""",
    """  paths.bootstrap,
);

requireAll(
  reconciler,
  [
    "pub struct ProductChannelProjectionReconciler",
    "DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT",
    "LEGACY_PRODUCT_CHANNEL_TENANTS_SQL",
    "allowed_channel_slugs}' IS NULL",
    "self.projector.rebuild_product_scope(tenant_id).await?",
    "reconciliation_selects_only_missing_legacy_projection",
  ],
  paths.reconciler,
);
requireAll(
  serverWorker,
  [
    "start_product_channel_projection_reconciliation_if_ready",
    "ProductChannelProjectionReconciler::new",
    "sweep_due(DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT)",
    "Product Search channel projection reconciliation completed",
  ],
  paths.serverWorker,
);
requireAll(
  serverServices,
  ["pub mod search_product_channel_reconciliation;"],
  paths.serverServices,
);
requireAll(
  serverBootstrap,
  ["start_product_channel_projection_reconciliation_if_ready"],
  paths.serverBootstrap,
);

requireAll(
  engine,
""",
)
replace_once(
    verifier,
    """  searchLib,
  ["mod storefront_product_channel_visibility;"],
""",
    """  searchLib,
  [
    "mod product_channel_reconciliation;",
    "mod storefront_product_channel_visibility;",
    "ProductChannelProjectionReconciler",
  ],
""",
)
replace_once(
    verifier,
    """  if (contract.reconciliation?.manual_backfill_required !== false) {
    failures.push(`${paths.contract}: manual backfill claim drift`);
  }
""",
    """  if (!contract.reconciliation?.startup_worker_detects_missing_legacy_projection) {
    failures.push(`${paths.contract}: startup repair invariant is missing`);
  }
  if (!contract.reconciliation?.malformed_explicit_owner_projection_is_not_rebuilt_forever) {
    failures.push(`${paths.contract}: malformed owner loop guard is missing`);
  }
  if (contract.reconciliation?.manual_backfill_required !== false) {
    failures.push(`${paths.contract}: manual backfill claim drift`);
  }
""",
)

print("FORUM-23B2E2 startup reconciliation wired")
