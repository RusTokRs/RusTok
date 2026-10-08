#!/usr/bin/env node
// RusTok product admin operation-result guardrails.
//
// The mounted Product admin must never discard a lifecycle result: every status
// change, variant update, image write, and product deletion has to surface its
// failure and refresh what the operator sees.

import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");

const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  try {
    return readFileSync(repoPath(relativePath), "utf8");
  } catch (error) {
    failures.push(`${relativePath}: expected readable source (${error.message})`);
    return "";
  }
}

function assert(condition, description) {
  if (!condition) {
    failures.push(description);
  }
}

function assertContains(text, value, description) {
  if (!text.includes(value)) {
    failures.push(description);
  }
}

function assertAbsent(text, value, description) {
  if (text.includes(value)) {
    failures.push(description);
  }
}

function assertCount(text, value, expected, description) {
  const actual = text.split(value).length - 1;
  if (actual !== expected) {
    failures.push(`${description} (expected ${expected}, found ${actual})`);
  }
}

const uiDir = "crates/modules/rustok-product/admin/src/ui";
const editorPath = `${uiDir}/product_editor.rs`;
const gridPath = `${uiDir}/product_grid.rs`;
const rootPath = `${uiDir}/root.rs`;
const libPath = "crates/modules/rustok-product/admin/src/lib.rs";

const editor = readRepo(editorPath);
const grid = readRepo(gridPath);
const root = readRepo(rootPath);
const lib = readRepo(libPath);

// 1. No lifecycle result may be discarded anywhere in the Product admin UI.
for (const entry of readdirSync(repoPath(uiDir))) {
  if (!entry.endsWith(".rs")) continue;
  const relativePath = `${uiDir}/${entry}`;
  const source = readRepo(relativePath);
  assertAbsent(
    source,
    "let _ = transport::",
    `${relativePath}: lifecycle results must not be discarded (let _ = transport::)`
  );
  assertAbsent(
    source,
    "let _ = crate::transport::",
    `${relativePath}: lifecycle results must not be discarded (let _ = crate::transport::)`
  );
  assertAbsent(
    source,
    "let _ = catalog_transport::",
    `${relativePath}: lifecycle results must not be discarded (let _ = catalog_transport::)`
  );
  assertAbsent(
    source,
    "let _ = crate::catalog_transport::",
    `${relativePath}: lifecycle results must not be discarded (let _ = crate::catalog_transport::)`
  );
}

// 2. The mounted editor reports image, status, and variant failures.
assertContains(
  editor,
  "Could not add the image: {err}",
  `${editorPath}: a rejected image upload must reach the operator`
);
assertContains(
  editor,
  "Could not delete the image: {err}",
  `${editorPath}: a rejected image deletion must reach the operator`
);
assertContains(
  editor,
  "let status_result = catalog_transport::change_product_status(",
  `${editorPath}: the save flow must consume the status transition result`
);
assertContains(
  editor,
  "variant_result = Some(",
  `${editorPath}: the save flow must consume the variant update result`
);
assertContains(
  editor,
  "the status was not changed: {err}",
  `${editorPath}: a failed status transition must be reported as a partial save`
);
assertContains(
  editor,
  "the variant was not saved: {err}",
  `${editorPath}: a failed variant update must be reported as a partial save`
);
assertContains(
  editor,
  "The product was saved, but not everything was applied",
  `${editorPath}: partial saves must not be reported as a plain success`
);
assertCount(
  editor,
  '<Show when=move || error_msg.get().is_some()>',
  1,
  `${editorPath}: the editor must keep exactly one error surface`
);
assert(
  (editor.match(/set_error_msg\.set\(/g) ?? []).length >= 6,
  `${editorPath}: every lifecycle failure path must set the editor error surface`
);

// 3. The mounted grid reports bulk and quick mutations.
assertContains(
  grid,
  "The status was not changed for {failed} of {total} products: {err}",
  `${gridPath}: bulk status failures must be reported with the affected count`
);
assertContains(
  grid,
  "{failed} of {total} products were not deleted: {err}",
  `${gridPath}: bulk delete failures must be reported with the affected count`
);
assertContains(
  grid,
  "The product status was not changed: {err}",
  `${gridPath}: a quick status change failure must reach the operator`
);
assertContains(
  grid,
  "The product was not deleted: {err}",
  `${gridPath}: a quick delete failure must reach the operator`
);
assertCount(
  grid,
  "Failed to authenticate bootstrap",
  2,
  `${gridPath}: bulk operations must report a failed bootstrap instead of returning silently`
);
assertCount(
  grid,
  '<Show when=move || error_msg.get().is_some()>',
  1,
  `${gridPath}: the grid must keep exactly one error surface`
);
assert(
  (grid.match(/set_error_msg\.set\(/g) ?? []).length >= 8,
  `${gridPath}: bulk and quick failure paths must set the grid error surface`
);

// 4. The mounted surface composes every read and write through the canonical
//    transport facade, and the typed attribute editor is mounted on the editor
//    page instead of staying behind an unmounted surface.
assertContains(
  lib,
  "pub use ui::root::ProductAdmin;",
  `${libPath}: the mounted admin entry point must stay exported`
);
assertContains(
  lib,
  "pub mod catalog_transport;",
  `${libPath}: the canonical Product admin transport facade must stay wired`
);
assertAbsent(
  root,
  "let _ = transport::",
  `${rootPath}: the mounted router must not discard lifecycle results`
);
assertAbsent(
  lib,
  "pub use ui::leptos::ProductAdmin as LeptosProductAdmin;",
  `${libPath}: the unmounted reference composition must not be advertised as a crate-root admin`
);
assertAbsent(
  lib,
  "pub use ui::catalog_admin::ProductAdmin as CatalogProductAdmin;",
  `${libPath}: the unmounted catalog composition must not be advertised as a crate-root admin`
);

const gridSource = readRepo(gridPath);
const editorSource = readRepo(editorPath);

for (const marker of [
  "catalog_transport::product_admin_list_input_from_route()",
  "provide_context(catalog_controls.clone())",
  "catalog_transport::fetch_products(",
  "catalog_transport::fetch_catalog_search_options(",
  'name="category_id"',
  'name="sort_by"',
  'name="sort_direction"',
  'name="attribute_filters"',
]) {
  assertContains(gridSource, marker, `${gridPath}: the mounted list must consume the owner catalog contract (${marker})`);
}
for (const marker of [
  "catalog_transport::fetch_effective_product_form(",
  "<ProductAttributeValuesSection",
]) {
  assertContains(
    editorSource,
    marker,
    `${editorPath}: the mounted editor must own the typed attribute-value flow (${marker})`
  );
}

const leptosAdapter = readRepo(`${uiDir}/leptos.rs`);
for (const marker of [
  "pub fn ProductAttributeValuesSection(",
  "catalog_transport::save_product_attribute_values(",
  "catalog_transport::clear_detached_product_attribute_values(",
  "TypedProductAttributeField",
]) {
  assertContains(
    leptosAdapter,
    marker,
    `${uiDir}/leptos.rs: the shared Leptos attribute adapter must stay complete (${marker})`
  );
}


// --- Authoring surfaces: variant axes and category-schema authoring must stay
// reachable from the mounted admin on both surfaces (PROD-AXES-001,
// PROD-SCHEMA-UI-001). A read-only placeholder here is a regression.
assertContains(
  editorSource,
  "<ProductVariantAxesSection",
  `${editorPath}: the mounted editor must own the variant-axis authoring surface`
);
assertAbsent(
  editorSource,
  "variant_axes_str",
  `${editorPath}: the read-only variant-axis placeholder must not come back`
);
assertAbsent(
  editorSource,
  "Comma-separated attribute axes",
  `${editorPath}: the read-only variant-axis placeholder must not come back`
);

const lifecycleIdentity = readRepo(
  "crates/modules/rustok-product/admin/src/lifecycle_retry_identity.rs"
);
assertContains(
  lifecycleIdentity,
  "SetVariantAxes,",
  "lifecycle_retry_identity.rs: the axis write must keep a retry-identity slot"
);

const axisGraphql = readRepo(
  "crates/modules/rustok-product/admin/src/transport/product_lifecycle_graphql.rs"
);
for (const marker of [
  "const SET_VARIANT_AXES_MUTATION",
  "setProductVariantAxes(idempotencyKey: $idempotencyKey, productId: $productId, input: $input)",
  "pub(crate) async fn set_variant_axes(",
]) {
  assertContains(
    axisGraphql,
    marker,
    `product_lifecycle_graphql.rs: the axis write must keep its idempotent GraphQL contract (${marker})`
  );
}
assertContains(
  readRepo("crates/modules/rustok-product/admin/src/transport/retry.rs"),
  "ProductAdminLifecycleOperation::SetVariantAxes",
  "transport/retry.rs: the axis write must keep its retained caller identity"
);
for (const facadePath of [
  "crates/modules/rustok-product/admin/src/transport.rs",
  "crates/modules/rustok-product/admin/src/catalog_transport.rs",
]) {
  assertContains(
    readRepo(facadePath),
    "set_variant_axes,",
    `${facadePath}: the axis write must stay re-exported through both admin import paths`
  );
}
for (const marker of [
  "pub fn ProductVariantAxesSection(",
  "catalog_transport::set_variant_axes(",
  "SetVariantAxesDraft { axes }",
  "attribute.variant_axis_policy != \"forbidden\"",
  "attribute.default_variant_axis",
]) {
  assertContains(
    leptosAdapter,
    marker,
    `${uiDir}/leptos.rs: the shared variant-axis section must stay complete (${marker})`
  );
}

const attributesPage = readRepo(`${uiDir}/attributes.rs`);
for (const marker of [
  "use super::leptos::ProductSchemaAuthoringCard;",
  "<ProductSchemaAuthoringCard locale=",
]) {
  assertContains(
    attributesPage,
    marker,
    `${uiDir}/attributes.rs: the mounted attribute page must expose schema authoring (${marker})`
  );
}
for (const marker of [
  "pub fn ProductSchemaAuthoringCard(locale: Option<String>)",
  "fn apply_schema_authoring_result(",
  "catalog_transport::set_category_schema_mode(",
  "catalog_transport::create_product_attribute_schema_group(",
  "catalog_transport::create_category_attribute_group(",
  "catalog_transport::bind_schema_attribute(",
  "catalog_transport::bind_category_attribute(",
]) {
  assertContains(
    leptosAdapter,
    marker,
    `${uiDir}/leptos.rs: the schema authoring card must drive the five owner commands (${marker})`
  );
}
assertAbsent(
  leptosAdapter,
  "let _ = catalog_transport::",
  `${uiDir}/leptos.rs: owner command results must not be discarded`
);

// Next admin must expose the same authoring surfaces.
const nextApi = readRepo("apps/next-admin/packages/rustok-product/src/api/products.ts");
for (const marker of [
  "export const SET_VARIANT_AXES_MUTATION",
  "export async function setVariantAxes(",
  "setProductVariantAxes: VariantAxisConfig[]",
]) {
  assertContains(
    nextApi,
    marker,
    `apps/next-admin/packages/rustok-product/src/api/products.ts: the Next admin must keep the axis write (${marker})`
  );
}
assertContains(
  readRepo("apps/next-admin/packages/rustok-product/src/api/attributes.ts"),
  "export async function createProductAttributeSchemaGroup(",
  "apps/next-admin/packages/rustok-product/src/api/attributes.ts: the Next admin must keep schema-group creation"
);
assertContains(
  readRepo("apps/next-admin/packages/rustok-product/src/index.ts"),
  "export * from './components/attributes/schema-authoring-card';",
  "apps/next-admin/packages/rustok-product/src/index.ts: the schema authoring card must stay exported"
);
assertContains(
  readRepo("apps/next-admin/packages/rustok-product/src/pages/attributes-page.tsx"),
  "<SchemaAuthoringCard",
  "apps/next-admin/packages/rustok-product/src/pages/attributes-page.tsx: the schema authoring card must stay mounted"
);
const nextAuthoringActions = readRepo(
  "apps/next-admin/src/app/dashboard/product/attributes/actions.ts"
);
for (const marker of [
  "export async function setCategorySchemaModeAction(",
  "export async function createSchemaAttributeGroupAction(",
  "export async function bindSchemaAttributeAction(",
  "export async function createCategoryAttributeGroupAction(",
  "export async function bindCategoryAttributeAction(",
]) {
  assertContains(
    nextAuthoringActions,
    marker,
    `attributes/actions.ts: every schema authoring command needs a server action (${marker})`
  );
}
for (const marker of [
  "onSetSchemaMode={setCategorySchemaModeAction}",
  "onCreateSchemaGroup={createSchemaAttributeGroupAction}",
  "onCreateCategoryGroup={createCategoryAttributeGroupAction}",
  "onBindSchemaAttribute={bindSchemaAttributeAction}",
  "onBindCategoryAttribute={bindCategoryAttributeAction}",
]) {
  assertContains(
    readRepo("apps/next-admin/src/app/dashboard/product/attributes/page.tsx"),
    marker,
    `apps/next-admin/.../attributes/page.tsx: the schema authoring actions must stay wired (${marker})`
  );
}

if (failures.length > 0) {
  console.error("[verify-product-admin-operation-results] failures:");
  for (const failure of failures) {
    console.error(`  - ${failure}`);
  }
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-product-admin-operation-results] Mounted Product admin surfaces every lifecycle result, consumes the canonical transport facade, and owns the typed attribute-value flow"
);
