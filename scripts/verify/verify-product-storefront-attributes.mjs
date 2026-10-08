#!/usr/bin/env node
/**
 * Source gate for the storefront attribute projection (`PROD-ATTR-STORE-001`).
 *
 * Attribute values are typed, localized and permission-graded in the owner's EAV tables, so the
 * storefront detail contract may only carry what the owner resolved: storefront-safe attributes,
 * localized labels and formatted values. This verifier locks the whole chain — owner projection →
 * GraphQL contract → both storefront adapters → both detail views — and rejects the two ways the
 * chain can silently regress: a storefront that starts reading attribute tables itself, and an
 * owner that stops honouring the storefront flag or starts leaking service attributes.
 */

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireAll(source, markers, description) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
  }
}

function reject(source, markers, description) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${description}: forbidden ${marker}`);
  }
}

/** Every file under a directory, by extension, as `relativePath -> source`. */
function tree(directory, extensions) {
  const absolute = path.join(repoRoot, directory);
  const collected = [];
  if (!existsSync(absolute)) return collected;
  const walk = (current) => {
    for (const entry of readdirSync(current)) {
      if (entry === "node_modules" || entry === "target" || entry === ".next") continue;
      const entryPath = path.join(current, entry);
      if (statSync(entryPath).isDirectory()) {
        walk(entryPath);
        continue;
      }
      if (extensions.some((extension) => entry.endsWith(extension))) {
        collected.push({
          file: path.relative(repoRoot, entryPath),
          source: readFileSync(entryPath, "utf8"),
        });
      }
    }
  };
  walk(absolute);
  return collected;
}

const OWNER_PROJECTION = "crates/modules/rustok-product/src/services/catalog/storefront_attributes.rs";
const OWNER_DTO = "crates/modules/rustok-product/src/dto/product.rs";
const OWNER_DETAIL_QUERY = "crates/modules/rustok-product/src/services/catalog/queries.rs";
const OWNER_ADMIN_PROJECTION = "crates/modules/rustok-product/src/services/catalog/projection.rs";
const OWNER_FORM_LOADER = "crates/modules/rustok-product/src/services/catalog_schema_service/effective_forms.rs";
const COMMERCE_TYPES = "crates/modules/rustok-commerce/src/graphql/types.rs";
const STOREFRONT_MODEL = "crates/modules/rustok-product/storefront/src/model.rs";
const STOREFRONT_CORE = "crates/modules/rustok-product/storefront/src/core.rs";
const STOREFRONT_UI = "crates/modules/rustok-product/storefront/src/ui/leptos.rs";
const STOREFRONT_GRAPHQL = "crates/modules/rustok-product/storefront/src/transport/graphql_adapter.rs";
const STOREFRONT_NATIVE = "crates/modules/rustok-product/storefront/src/transport/native_server_adapter.rs";
const NEXT_TYPES = "apps/next-frontend/packages/rustok-product/src/api/types.ts";
const NEXT_PRODUCTS = "apps/next-frontend/packages/rustok-product/src/api/products.ts";
const NEXT_SPECIFICATIONS = "apps/next-frontend/packages/rustok-product/src/catalog/specifications.ts";
const NEXT_DETAIL_VIEW = "apps/next-frontend/packages/rustok-product/src/components/product-detail-view.tsx";

const ownerProjection = read(OWNER_PROJECTION);
const ownerDto = read(OWNER_DTO);
const ownerDetailQuery = read(OWNER_DETAIL_QUERY);
const ownerAdminProjection = read(OWNER_ADMIN_PROJECTION);
const ownerFormLoader = read(OWNER_FORM_LOADER);
const commerceTypes = read(COMMERCE_TYPES);
const storefrontModel = read(STOREFRONT_MODEL);
const storefrontCore = read(STOREFRONT_CORE);
const storefrontUi = read(STOREFRONT_UI);
const storefrontGraphql = read(STOREFRONT_GRAPHQL);
const storefrontNative = read(STOREFRONT_NATIVE);
const nextTypes = read(NEXT_TYPES);
const nextProducts = read(NEXT_PRODUCTS);
const nextSpecifications = read(NEXT_SPECIFICATIONS);
const nextDetailView = read(NEXT_DETAIL_VIEW);

const storefrontEn = read("crates/modules/rustok-product/storefront/locales/en.ftl");
const storefrontRu = read("crates/modules/rustok-product/storefront/locales/ru.ftl");

// ── Owner: the projection resolves visibility, labels and values itself ───────────────────────────
requireAll(
  ownerDto,
  [
    "#[serde(default)]\n    pub storefront_attributes: Vec<StorefrontProductAttributeResponse>",
    "pub struct StorefrontProductAttributeResponse {",
    "pub struct StorefrontProductAttributeValueResponse {",
  ],
  "owner detail contract",
);
requireAll(
  ownerProjection,
  [
    "pav.detached_at IS NULL",
    "pa.archived_at IS NULL",
    "pa.scope IN ('product', 'both')",
    "binding.visibility_overrides.show_on_storefront",
    "binding.is_disabled",
    "AttributeValueType::Json",
    "product_attribute_translations",
    "product_attribute_value_translations",
    "product_attribute_option_translations",
    "ORDER BY pa.position, pa.code",
  ],
  "owner storefront attribute projection",
);
reject(
  ownerProjection,
  ["is_filterable", "is_searchable", "is_sortable", "is_comparable"],
  "owner storefront attribute projection must not depend on admin facet flags",
);
// The flag is merged in Rust (definition default, category override), never filtered in SQL: a
// category binding may enable a definition-hidden attribute and the other way round. The column is
// allowed in a selection list and forbidden in a predicate.
for (const [index, statement] of [
  ...ownerProjection.matchAll(/r#"\n([\s\S]*?)"#/g),
].entries()) {
  const sql = statement[1];
  const predicate = sql.indexOf("WHERE");
  if (predicate < 0) continue;
  if (sql.slice(predicate).includes("show_on_storefront")) {
    failures.push(
      `${OWNER_PROJECTION}: statement ${index + 1} filters show_on_storefront in SQL; the category binding override must be merged in code`,
    );
  }
}
requireAll(
  ownerDetailQuery,
  [
    // rustfmt wraps the assignment, so the marker is split into the assignment and the call: the
    // rule is "the detail path loads the storefront projection", not one line of it.
    "product.storefront_attributes =",
    "super::storefront_attributes::load_storefront_product_attributes(",
  ],
  "published storefront detail",
);
requireAll(
  ownerAdminProjection,
  ["storefront_attributes: Vec::new()"],
  "admin product projection",
);
requireAll(
  ownerFormLoader,
  ["pub(crate) async fn load_effective_form_for_product_in<C>("],
  "effective form loader visibility (the storefront projection reads the binding overrides)",
);

// ── Transport: the wire contract carries the block ────────────────────────────────────────────────
requireAll(
  commerceTypes,
  [
    "pub struct GqlStorefrontProductAttribute {",
    "pub struct GqlStorefrontProductAttributeValue {",
    "pub attributes: Vec<GqlStorefrontProductAttribute>,",
    "attributes: product\n                .storefront_attributes",
  ],
  "commerce GraphQL product attributes",
);

// ── Storefront adapters: both transports fill the same model ──────────────────────────────────────
requireAll(
  storefrontModel,
  [
    "pub struct ProductAttribute {",
    "pub struct ProductAttributeValue {",
    '#[serde(rename = "valueType")]',
    '#[serde(rename = "isLocalized")]',
    "pub attributes: Vec<ProductAttribute>,",
  ],
  "storefront detail model",
);
requireAll(
  storefrontGraphql,
  ["attributes { code label valueType isLocalized values { text } }"],
  "storefront GraphQL selection",
);
if (!storefrontGraphql.includes("attributes { code label valueType isLocalized values { text } }")) {
  failures.push(`${STOREFRONT_GRAPHQL}: storefront detail query must request the attribute block`);
}
requireAll(
  storefrontNative,
  [
    "fn map_product_attributes(",
    "let attributes = map_product_attributes(value.storefront_attributes);",
  ],
  "storefront native adapter",
);

// ── Detail views: both render the owner's rows and localize only the boolean vocabulary ───────────
requireAll(
  storefrontCore,
  [
    "pub struct SelectedProductAttributeViewModel {",
    "pub attributes: Vec<SelectedProductAttributeViewModel>,",
    '"product.selected.attributeYes"',
    '"product.selected.attributeNo"',
  ],
  "storefront detail view model",
);
requireAll(
  storefrontUi,
  ["view_model.attributes.is_empty()", "<dl class=", "attribute.values.join"],
  "leptos specifications block",
);
requireAll(storefrontEn, ["product-selected-attributes =", "product-selected-attributeYes =", "product-selected-attributeNo ="], "english storefront catalog");
requireAll(storefrontRu, ["product-selected-attributes =", "product-selected-attributeYes =", "product-selected-attributeNo ="], "russian storefront catalog");

requireAll(
  nextTypes,
  [
    "export type StorefrontProductAttribute = {",
    "export type StorefrontProductAttributeValue = {",
    "attributes?: StorefrontProductAttribute[];",
  ],
  "next storefront attribute types",
);
requireAll(nextProducts, ["attributes {\n        code\n        label\n        valueType\n        isLocalized\n        values {\n          text\n        }\n      }"], "next storefront detail query");
requireAll(
  nextSpecifications,
  [
    "export function buildProductSpecificationLabels(",
    "export function formatSpecificationValue(",
    "export function buildProductSpecifications(",
    'case "true":',
    'case "false":',
  ],
  "next specification rows",
);
requireAll(
  nextDetailView,
  ["buildProductSpecifications(product, specificationLabels)", "{specifications.map((row) => ("],
  "next detail view",
);
reject(
  nextDetailView,
  ['"Быстрая доставка"', '"Fast delivery"', '"Гарантия качества"', '"Original quality"'],
  "next detail view must render owner specifications instead of static marketing rows",
);

// ── Boundary: no storefront joins the attribute tables itself ─────────────────────────────────────
const STOREFRONT_PACKAGES = [
  ["crates/modules/rustok-product/storefront/src", [".rs"]],
  ["apps/next-frontend/packages/rustok-product/src", [".ts", ".tsx"]],
];
const ATTRIBUTE_TABLES = [
  "product_attribute_values",
  "product_attribute_translations",
  "product_attribute_option_translations",
  "product_attribute_value_translations",
  "product_attribute_value_options",
];
for (const [directory, extensions] of STOREFRONT_PACKAGES) {
  for (const { file, source } of tree(directory, extensions)) {
    for (const table of ATTRIBUTE_TABLES) {
      if (source.includes(table)) {
        failures.push(
          `${file}: storefront code must not touch ${table}; the owner resolves attribute rows`,
        );
      }
    }
  }
}

if (failures.length > 0) {
  console.error("storefront attribute verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("storefront attribute verification passed");
