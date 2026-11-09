# Scripts

This folder stores scripts that are specific to this crate/module.

Rules:
- Keep module-specific verification, migration, generation, or maintenance scripts here.
- Keep cross-platform orchestration scripts in the repository-level `scripts/` folder.
- When script behavior changes public/runtime contracts, update local docs and central docs accordingly.

Use [repair_index_graph.sql](repair_index_graph.sql) for tenant-scoped recovery after
the Product graph trigger upgrade. See the [recovery procedure](../docs/index-graph-projection-ledger.md#recovery-after-upgrading-the-update-trigger).
