import fs from "node:fs";

const migrationPath =
  "crates/rustok-product/src/migrations/m20260829_000019_retire_product_category_closure_invariant.rs";
const migrationIndexPath = "crates/rustok-product/src/migrations/mod.rs";
const contractPath = "crates/rustok-product/docs/category-taxonomy-binding.md";

const migration = fs.readFileSync(migrationPath, "utf8");
const migrationIndex = fs.readFileSync(migrationIndexPath, "utf8");
const contract = fs.readFileSync(contractPath, "utf8");

const downMarker = "async fn down(&self, manager: &SchemaManager)";
const downOffset = migration.indexOf(downMarker);
if (downOffset < 0) {
  throw new Error("CAT-33 migration must expose a reversible PostgreSQL down path");
}
const up = migration.slice(0, downOffset);
const down = migration.slice(downOffset);

function requireIncludes(source, marker, message) {
  if (!source.includes(marker)) {
    throw new Error(message);
  }
}

function requireExcludes(source, marker, message) {
  if (source.includes(marker)) {
    throw new Error(message);
  }
}

requireIncludes(
  migrationIndex,
  "mod m20260829_000019_retire_product_category_closure_invariant;",
  "CAT-33 migration module must be registered",
);
requireIncludes(
  migrationIndex,
  "Box::new(m20260829_000019_retire_product_category_closure_invariant::Migration)",
  "CAT-33 migration must be appended to the Product migration plan",
);

requireIncludes(
  up,
  "if manager.get_database_backend() != DatabaseBackend::Postgres",
  "CAT-33 invariant cutover must remain PostgreSQL-only",
);
requireIncludes(
  up,
  "SELECT rustok_product_assert_category_tree();",
  "CAT-33 must preflight the historical tree/closure invariant before replacing it",
);
requireIncludes(
  up,
  "CREATE OR REPLACE FUNCTION rustok_product_assert_category_tree()",
  "CAT-33 must replace the historical assertion function",
);
requireIncludes(
  up,
  "catalog category tree contains a cycle",
  "CAT-33 must retain cycle rejection",
);
requireExcludes(
  up,
  "FULL OUTER JOIN catalog_category_closure",
  "CAT-33 PostgreSQL up path must not keep closure parity as a commit invariant",
);
requireExcludes(
  up,
  "DROP TABLE",
  "CAT-33 is invariant retirement, not physical closure storage retirement",
);
requireExcludes(
  up,
  "DROP TRIGGER",
  "CAT-33 must retain the historical closure trigger as a cycle-only compatibility object",
);

requireIncludes(
  down,
  "DELETE FROM catalog_category_closure;",
  "CAT-33 rollback must rebuild closure from canonical Product parent projection before restoring parity",
);
requireIncludes(
  down,
  "WITH RECURSIVE category_walk AS",
  "CAT-33 rollback must derive the closure projection recursively",
);
requireIncludes(
  down,
  "INSERT INTO catalog_category_closure (tenant_id, ancestor_id, descendant_id, depth)",
  "CAT-33 rollback must restore exact closure rows",
);
requireIncludes(
  down,
  "FULL OUTER JOIN catalog_category_closure actual",
  "CAT-33 rollback must restore the historical closure-parity assertion",
);
requireIncludes(
  down,
  "catalog category closure is not the canonical parent-tree projection",
  "CAT-33 rollback must restore historical parity failure semantics",
);

for (const marker of [
  "TAXONOMY-CAT-33",
  "closure invariant retirement",
  "cycle-only",
  "physical closure storage retirement",
  "Non-PostgreSQL",
]) {
  requireIncludes(contract, marker, `CAT-33 contract is missing marker: ${marker}`);
}

console.log("Product Category closure invariant retirement contract verified.");
