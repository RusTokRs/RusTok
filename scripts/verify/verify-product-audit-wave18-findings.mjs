// Verifies the wave-18 product audit fixes statically:
// bounded tenant-wide schema lists, static parser error messages, owner-side
// products:manage claim on schema writes, and publish-requirement checks inside
// the product create transaction.
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const read = (rel) => readFileSync(join(root, rel), "utf8");
const failures = [];
const assertContains = (text, needle, message) => {
  if (!text.includes(needle)) failures.push(message);
};
const assertAbsent = (text, needle, message) => {
  if (text.includes(needle)) failures.push(message);
};

const paths = {
  root: "crates/modules/rustok-product/src/services/catalog_schema_service.rs",
  attributes: "crates/modules/rustok-product/src/services/catalog_schema_service/attributes.rs",
  schemas: "crates/modules/rustok-product/src/services/catalog_schema_service/schemas.rs",
  categories: "crates/modules/rustok-product/src/services/catalog_schema_service/categories.rs",
  values: "crates/modules/rustok-product/src/services/catalog_schema_service/values.rs",
  writePort: "crates/modules/rustok-product/src/catalog_schema_write_port.rs",
  commands: "crates/modules/rustok-product/src/services/catalog/commands.rs",
  nativeAdapter: "crates/modules/rustok-product/admin/src/transport/native_server_adapter.rs",
};
const src = Object.fromEntries(Object.entries(paths).map(([k, v]) => [k, read(v)]));

// 1. Tenant-wide schema lists are bounded.
assertContains(src.root, "const MAX_SCHEMA_LIST_ROWS: usize = 1000;", `${paths.root}: list row bound constant missing`);
assertContains(src.root, "fn ensure_schema_list_within_limit(", `${paths.root}: list limit helper missing`);
assertContains(src.attributes, "LIMIT $3", `${paths.attributes}: list_attributes SQL must be bounded`);
assertContains(src.attributes, 'ensure_schema_list_within_limit(rows.len(), "attribute")', `${paths.attributes}: list_attributes must enforce the bound`);
assertContains(src.schemas, "LIMIT $3", `${paths.schemas}: list_schemas SQL must be bounded`);
assertContains(src.schemas, 'ensure_schema_list_within_limit(rows.len(), "schema")', `${paths.schemas}: list_schemas must enforce the bound`);
assertContains(src.categories, "LIMIT $2", `${paths.categories}: taxonomy category SQL must be bounded`);
assertContains(src.categories, 'ensure_schema_list_within_limit(rows.len(), "category")', `${paths.categories}: taxonomy categories must enforce the bound`);

// 2. Native parsers return static messages, not Debug output of internal errors.
for (const needle of [
  'ServerFnError::new("attribute value type is not supported")',
  'ServerFnError::new("category kind is not supported")',
  'ServerFnError::new("category schema mode is not supported")',
  'ServerFnError::new("category attribute binding kind is not supported")',
]) {
  assertContains(src.nativeAdapter, needle, `${paths.nativeAdapter}: static parser message missing: ${needle}`);
}
assertAbsent(src.nativeAdapter, 'ServerFnError::new(format!("{error:?}"))', `${paths.nativeAdapter}: parser errors must not expose Debug output`);

// 3. Owner-side products:manage check runs inside schema_write_scope before admission.
assertContains(src.writePort, "require_products_manage_claim(context, operation)?;", `${paths.writePort}: schema write scope must call the claim check`);
assertContains(src.writePort, "rustok_api::Permission::PRODUCTS_MANAGE.to_string()", `${paths.writePort}: claim check must use PRODUCTS_MANAGE`);
assertContains(src.writePort, '"product.schema_write_forbidden"', `${paths.writePort}: forbidden code missing`);
{
  const wp = src.writePort;
  const scopeStart = wp.indexOf("fn schema_write_scope(");
  const scopeEnd = wp.indexOf("\n}\n", scopeStart);
  const scopeBody = wp.slice(scopeStart, scopeEnd);
  if (!scopeBody.includes("require_products_manage_claim(context, operation)?;")) {
    failures.push(`${paths.writePort}: schema_write_scope must run the products:manage claim check`);
  }
  // Every admission call site must sit in a method that first runs schema_write_scope.
  const admitRe = /\badmit_schema_operation\(/g;
  let m;
  while ((m = admitRe.exec(wp))) {
    if (m.index > scopeStart && m.index < scopeEnd + 3) continue; // definition/body of scope
    const methodStart = wp.lastIndexOf("async fn ", m.index);
    if (methodStart < 0) continue;
    const between = wp.slice(methodStart, m.index);
    if (!between.includes("schema_write_scope(")) {
      failures.push(`${paths.writePort}: admission at offset ${m.index} is not preceded by schema_write_scope in its method`);
    }
  }
}

// 4. Publish requirements for a new product are checked inside the create transaction.
assertContains(src.values, "pub(crate) async fn validate_new_product_publish_requirements_in<C>(", `${paths.values}: connection-scoped publish check missing`);
assertContains(src.values, "Self::validate_new_product_publish_requirements_in(&self.db, tenant_id, primary_category_id)", `${paths.values}: public check must delegate`);
{
  const create = src.commands.slice(src.commands.indexOf("ProductWriteTransaction::begin(&self.db"));
  const checkAt = create.indexOf("validate_new_product_publish_requirements_in(");
  const insertAt = create.indexOf("entities::product::ActiveModel {");
  if (checkAt < 0 || insertAt < 0 || checkAt > insertAt) {
    failures.push(`${paths.commands}: publish check must run inside the create transaction before the product insert`);
  }
}
assertAbsent(src.commands, "validate_new_product_publish_requirements(tenant_id", `${paths.commands}: publish check must not run outside the transaction`);

if (failures.length) {
  console.error("product audit wave 18 verification failed:");
  for (const f of failures) console.error(`- ${f}`);
  process.exit(1);
}
console.log("product audit wave 18: bounded schema lists, static parser errors, owner products:manage check, transactional publish requirements");
