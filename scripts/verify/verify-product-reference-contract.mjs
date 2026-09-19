import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(new URL("../..", import.meta.url).pathname);
const failures = [];

function read(path) { return readFileSync(join(root, path), "utf8"); }
function requireAll(path, markers) {
  const source = read(path);
  for (const marker of markers) if (!source.includes(marker)) failures.push(`${path}: missing ${marker}`);
}
function forbid(path, markers) {
  const source = read(path);
  for (const marker of markers) if (source.includes(marker)) failures.push(`${path}: forbidden ${marker}`);
}
function rustFiles(path) {
  const dir = join(root, path), out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(path, entry.name);
    if (entry.isDirectory()) out.push(...rustFiles(p));
    else if (entry.name.endsWith(".rs")) out.push(p);
  }
  return out;
}

forbid("crates/modules/rustok-product/src/lib.rs", [
  "pub mod entities;",
  "pub mod migrations;",
  "pub use entities::",
]);
requireAll("crates/modules/rustok-product/src/lib.rs", [
  "mod domain;",
  "mod module;",
  "pub use module::{ProductModule, ProductRuntimeSelected};",
  "pub use dto::ProductStatus;",
]);
requireAll("crates/modules/rustok-product/src/module.rs", [
  "pub struct ProductModule;",
  "impl RusToKModule for ProductModule",
  "impl MigrationSource for ProductModule",
]);
requireAll("crates/modules/rustok-product/src/domain/product_status.rs", [
  "pub enum ProductStatus",
  'enum_name = "product_status_enum"',
]);
forbid("crates/modules/rustok-product/src/dto/product.rs", [
  "use crate::entities::product::ProductStatus;",
]);
requireAll("crates/modules/rustok-product/src/dto/product.rs", [
  "use crate::domain::ProductStatus;",
]);
requireAll("crates/modules/rustok-product/src/dto/product.rs", [
  "pub seller_id: Patch<String>",
  "pub vendor: Patch<String>",
  "pub product_type: Patch<String>",
  "pub shipping_profile_slug: Patch<String>",
  "pub primary_category_id: Patch<Uuid>",
  "serde(default, skip_serializing_if = " + JSON.stringify("Patch::is_keep") + ")",
]);
forbid("crates/modules/rustok-product/src/dto/product.rs", [
  "pub seller_id: Option<String>",
  "pub vendor: Option<String>",
  "pub product_type: Option<String>",
  "pub shipping_profile_slug: Option<String>",
  "pub primary_category_id: Option<Uuid>",
]);
requireAll("crates/modules/rustok-product/src/services/catalog/commands.rs", [
  "Patch::Keep => {}",
  "Patch::Clear => product_active.seller_id = Set(None)",
  "requested_primary_category.flatten()",
]);
for (const path of rustFiles("crates/modules/rustok-product/src")) {
  forbid(path, ["pub mod entities;", "pub mod migrations;"]);
}
console.log(failures.length ? failures.join("\n") : "Product public persistence API verification passed.");
if (failures.length) process.exit(1);
