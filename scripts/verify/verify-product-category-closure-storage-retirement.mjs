import fs from "node:fs";

const migrationPath =
  "crates/rustok-product/src/migrations/m20260829_000020_retire_product_category_closure_storage.rs";
const migrationIndexPath = "crates/rustok-product/src/migrations/mod.rs";
const initialSchemaPath =
  "crates/rustok-product/src/migrations/m20260701_000001_create_product_catalog_attributes.rs";
const tenantConsistencyPath =
  "crates/rustok-product/src/migrations/m20260701_000002_add_product_catalog_tenant_consistency_constraints.rs";
const categoriesPath =
  "crates/rustok-product/src/services/catalog_schema_service/categories.rs";
const effectiveFormsPath =
  "crates/rustok-product/src/services/catalog_schema_service/effective_forms.rs";
const contractPath = "crates/rustok-product/docs/category-taxonomy-binding.md";
const backfillContractPath = "docs/migrations/backfill-contracts.json";
const retainedInvariantWorkflowPath =
  ".github/workflows/product-category-closure-invariant-retirement.yml";

const migration = fs.readFileSync(migrationPath, "utf8");
const migrationIndex = fs.readFileSync(migrationIndexPath, "utf8");
const initialSchema = fs.readFileSync(initialSchemaPath, "utf8");
const tenantConsistency = fs.readFileSync(tenantConsistencyPath, "utf8");
const categories = fs.readFileSync(categoriesPath, "utf8");
const effectiveForms = fs.readFileSync(effectiveFormsPath, "utf8");
const contract = fs.readFileSync(contractPath, "utf8");
const backfillRegister = JSON.parse(fs.readFileSync(backfillContractPath, "utf8"));
const retainedInvariantWorkflow = fs.readFileSync(retainedInvariantWorkflowPath, "utf8");

const downMarker = "async fn down(&self, manager: &SchemaManager)";
const downOffset = migration.indexOf(downMarker);
if (downOffset < 0) {
  throw new Error("CAT-34 migration must expose a reversible PostgreSQL down path");
}
const up = migration.slice(0, downOffset);
const down = migration.slice(downOffset);

function requireIncludes(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

function requireExcludes(source, marker, message) {
  if (source.includes(marker)) throw new Error(message);
}

requireIncludes(
  migrationIndex,
  "mod m20260829_000020_retire_product_category_closure_storage;",
  "CAT-34 migration module must be registered",
);
const priorRegistration = migrationIndex.indexOf(
  "Box::new(m20260829_000019_retire_product_category_closure_invariant::Migration)",
);
const currentRegistration = migrationIndex.indexOf(
  "Box::new(m20260829_000020_retire_product_category_closure_storage::Migration)",
);
if (priorRegistration < 0 || currentRegistration <= priorRegistration) {
  throw new Error("CAT-34 migration must be appended after CAT-33 in the Product migration plan");
}

for (const [marker, message] of [
  [
    "if manager.get_database_backend() != DatabaseBackend::Postgres",
    "CAT-34 storage retirement must remain PostgreSQL-only",
  ],
  [
    "SELECT rustok_product_assert_category_tree();",
    "CAT-34 must retain the Product parent-cycle assertion around physical retirement",
  ],
  [
    "DROP TRIGGER trg_catalog_category_closure_validate_tree ON catalog_category_closure;",
    "CAT-34 must retire the closure-table compatibility trigger explicitly",
  ],
  [
    "DROP TABLE catalog_category_closure;",
    "CAT-34 must physically retire PostgreSQL closure storage",
  ],
]) {
  requireIncludes(up, marker, message);
}
requireExcludes(
  up,
  "DROP TABLE catalog_category_closure CASCADE",
  "CAT-34 must fail closed on unexpected external dependencies instead of using CASCADE",
);
requireExcludes(
  up,
  "CREATE TABLE catalog_category_closure",
  "CAT-34 up path must not recreate the retired closure table",
);
const dropOffset = up.indexOf("DROP TABLE catalog_category_closure;");
const postDropAssertion = up.lastIndexOf("SELECT rustok_product_assert_category_tree();");
if (dropOffset < 0 || postDropAssertion <= dropOffset) {
  throw new Error("CAT-34 must prove the retained cycle-only assertion still works after closure drop");
}

for (const [marker, message] of [
  [
    "if manager.get_database_backend() != DatabaseBackend::Postgres",
    "CAT-34 rollback must remain PostgreSQL-only",
  ],
  ["CREATE TABLE catalog_category_closure (", "CAT-34 rollback must recreate closure storage"],
  [
    "PRIMARY KEY (tenant_id, ancestor_id, descendant_id)",
    "CAT-34 rollback must restore the closure primary key",
  ],
  [
    "CONSTRAINT chk_catalog_category_closure_depth CHECK (depth >= 0)",
    "CAT-34 rollback must restore the closure depth check",
  ],
  [
    "fk_catalog_category_closure_ancestor_tenant",
    "CAT-34 rollback must restore the tenant-safe ancestor foreign key",
  ],
  [
    "fk_catalog_category_closure_descendant_tenant",
    "CAT-34 rollback must restore the tenant-safe descendant foreign key",
  ],
  [
    "CREATE INDEX idx_catalog_category_closure_descendant",
    "CAT-34 rollback must restore the historical descendant lookup index",
  ],
  ["WITH RECURSIVE category_walk AS", "CAT-34 rollback must rebuild closure recursively"],
  [
    "INSERT INTO catalog_category_closure (tenant_id, ancestor_id, descendant_id, depth)",
    "CAT-34 rollback must reconstruct closure rows from Product parent projection",
  ],
  [
    "FULL OUTER JOIN catalog_category_closure actual",
    "CAT-34 rollback must prove exact reconstructed closure parity",
  ],
  [
    "CREATE CONSTRAINT TRIGGER trg_catalog_category_closure_validate_tree",
    "CAT-34 rollback must restore the deferred closure compatibility trigger",
  ],
  ["DEFERRABLE INITIALLY DEFERRED", "CAT-34 rollback must preserve deferred trigger semantics"],
]) {
  requireIncludes(down, marker, message);
}
requireExcludes(
  down,
  "CREATE OR REPLACE FUNCTION rustok_product_assert_category_tree()",
  "CAT-34 rollback must return to CAT-33 cycle-only semantics rather than rolling CAT-33 back too",
);

for (const [source, marker, message] of [
  [
    initialSchema,
    "CREATE TABLE IF NOT EXISTS catalog_category_closure (",
    "historical closure creation migration must remain available for schema provenance",
  ],
  [
    initialSchema,
    "idx_catalog_category_closure_descendant",
    "historical closure descendant index provenance must remain available",
  ],
  [
    tenantConsistency,
    "fk_catalog_category_closure_ancestor_tenant",
    "historical tenant-safe ancestor constraint provenance must remain available",
  ],
  [
    tenantConsistency,
    "fk_catalog_category_closure_descendant_tenant",
    "historical tenant-safe descendant constraint provenance must remain available",
  ],
]) {
  requireIncludes(source, marker, message);
}

const closureHelperStart = categories.indexOf(
  "fn should_write_product_category_closure(",
);
const syncStart = categories.indexOf("async fn sync_created_category_to_taxonomy_in_tx(");
if (closureHelperStart < 0 || syncStart <= closureHelperStart) {
  throw new Error("Product closure compatibility helper boundaries are required");
}
requireIncludes(
  categories.slice(closureHelperStart, syncStart),
  "backend != DatabaseBackend::Postgres",
  "PostgreSQL runtime must remain closure-write retired before physical storage retirement",
);

const labelsStart = effectiveForms.indexOf("pub async fn load_effective_form_group_labels(");
const categoryMapStart = effectiveForms.indexOf("async fn load_category_schema_map<C>(");
if (labelsStart < 0 || categoryMapStart <= labelsStart) {
  throw new Error("effective-form group label boundaries are required");
}
const labelsBody = effectiveForms.slice(labelsStart, categoryMapStart);
const postgresBranch = labelsBody.indexOf(
  "self.db.get_database_backend() == DatabaseBackend::Postgres",
);
const taxonomyChain = labelsBody.indexOf("taxonomy_ancestor_chain(category_id, &parent_map)?");
const compatibilityElse = labelsBody.indexOf("} else {");
const closureRead = labelsBody.indexOf("FROM catalog_category_closure");
if (
  postgresBranch < 0 ||
  taxonomyChain < 0 ||
  compatibilityElse < 0 ||
  closureRead < 0 ||
  !(postgresBranch < taxonomyChain &&
    taxonomyChain < compatibilityElse &&
    compatibilityElse < closureRead)
) {
  throw new Error(
    "PostgreSQL must remain Taxonomy-backed while closure reads stay only in the non-PostgreSQL compatibility branch",
  );
}

for (const marker of [
  "TAXONOMY-CAT-34 PostgreSQL closure storage retirement",
  "physical PostgreSQL `catalog_category_closure` storage",
  "without `CASCADE`",
  "one-step rollback",
  "Non-PostgreSQL backends",
  "`parent_id`, `path` and `level` projections",
]) {
  requireIncludes(contract, marker, `CAT-34 contract is missing marker: ${marker}`);
}

const backfillContract = backfillRegister.contracts?.find(
  (entry) =>
    entry.migration === "m20260829_000020_retire_product_category_closure_storage",
);
if (!backfillContract) {
  throw new Error("CAT-34 migration must declare a migration backfill contract");
}
if (backfillContract.id !== "product-category-closure-storage-retirement") {
  throw new Error("CAT-34 backfill contract must use the stable Product Category storage retirement id");
}
if (backfillContract.mode !== "none" || backfillContract.owner !== "rustok-product") {
  throw new Error("CAT-34 backfill contract must be rustok-product mode none");
}
if (backfillContract.setup_sql !== undefined || backfillContract.assertion_sql !== undefined) {
  throw new Error("CAT-34 mode-none backfill contract must not fabricate fixture SQL");
}
for (const marker of [
  "no forward row backfill",
  "derived closure storage",
  "reconstructs exact closure rows",
]) {
  requireIncludes(
    backfillContract.reason,
    marker,
    `CAT-34 backfill reason is missing marker: ${marker}`,
  );
}

requireIncludes(
  retainedInvariantWorkflow,
  "CAT-33 migration/verifier unchanged; PR-wide CAT-33 slice restriction is not applicable.",
  "retained CAT-33 bounded scope must be progression-safe for CAT-34",
);

console.log("Product Category closure storage retirement contract verified.");
