#!/usr/bin/env node
/**
 * Contract tests for the storefront attribute projection.
 *
 * The projection itself is Rust (owner side), so these tests lock the invariants a refactor would
 * silently break across layers: one request shape for the attribute block in both storefronts, the
 * serde names of the Rust model matching the selection, the boolean vocabulary identical in the
 * Leptos catalogs and the Next labels, and the Next row builder dropping valueless attributes
 * without re-formatting anything the owner already formatted. The Next row builder is executed for
 * real (Node type stripping), so its behaviour is asserted, not just its source.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const read = (...segments) => readFileSync(path.join(repoRoot, ...segments), "utf8");

const ownerProjection = read(
  "crates/modules/rustok-product/src/services/catalog/storefront_attributes.rs",
);
const ownerDto = read("crates/modules/rustok-product/src/dto/product.rs");
const commerceTypes = read("crates/modules/rustok-commerce/src/graphql/types.rs");
const storefrontModel = read("crates/modules/rustok-product/storefront/src/model.rs");
const storefrontCore = read("crates/modules/rustok-product/storefront/src/core.rs");
const storefrontGraphql = read(
  "crates/modules/rustok-product/storefront/src/transport/graphql_adapter.rs",
);
const storefrontNative = read(
  "crates/modules/rustok-product/storefront/src/transport/native_server_adapter.rs",
);
const storefrontEn = read("crates/modules/rustok-product/storefront/locales/en.ftl");
const storefrontRu = read("crates/modules/rustok-product/storefront/locales/ru.ftl");
const nextProducts = read(
  "apps/next-frontend/packages/rustok-product/src/api/products.ts",
);
const nextTypes = read("apps/next-frontend/packages/rustok-product/src/api/types.ts");
const nextDetailView = read(
  "apps/next-frontend/packages/rustok-product/src/components/product-detail-view.tsx",
);
const specificationsPath = path.join(
  repoRoot,
  "apps/next-frontend/packages/rustok-product/src/catalog/specifications.ts",
);
const nextSpecifications = readFileSync(specificationsPath, "utf8");

/** Named fields of a Rust struct, in declaration order. */
function structFields(source, name) {
  const match = source.match(new RegExp(`pub struct ${name} \\{([\\s\\S]*?)\\n\\}`));
  assert.ok(match, `struct ${name} not found`);
  return [...match[1].matchAll(/^\s*pub\s+([a-z_][a-z0-9_]*):/gm)].map((field) => field[1]);
}

/** `field: value` pairs of a Rust struct literal that starts at `marker`, brace-balanced. */
function literalFields(source, marker) {
  const start = source.indexOf(marker);
  assert.notEqual(start, -1, `literal ${marker} not found`);
  const body = source.slice(source.indexOf("{", start) + 1);
  let depth = 1;
  let end = 0;
  for (; end < body.length && depth > 0; end += 1) {
    if (body[end] === "{") depth += 1;
    if (body[end] === "}") depth -= 1;
  }
  return [...body.slice(0, end).matchAll(/^\s*([a-z_][a-z0-9_]*):/gm)].map((field) => field[1]);
}

/** `key = value` entries of an FTL catalog, keeping only keys starting with `prefix`. */
function ftlValues(source, prefix) {
  const values = {};
  for (const line of source.split("\n")) {
    const separator = line.indexOf(" = ");
    if (separator < 0) continue;
    const key = line.slice(0, separator).trim();
    if (!key.startsWith(prefix)) continue;
    values[key] = line.slice(separator + 3).trim();
  }
  return values;
}

/** `t(locale, "…", "…")` pairs of a Rust source, as `key -> fallback`. */
function rustLabels(source, prefix) {
  const values = {};
  const pattern = /t\(\s*locale,\s*"([^"]+)",\s*\n?\s*"((?:[^"\\]|\\.)*)"/g;
  for (const match of source.matchAll(pattern)) {
    if (!match[1].startsWith(prefix)) continue;
    values[match[1]] = match[2];
  }
  return values;
}

const ftlKey = (key) => key.replaceAll(".", "-");

/** Attribute block of a GraphQL detail query, as `outer` and `inner` field lists. */
/**
 * Attribute block of a GraphQL detail query: the scalar fields, the nested collection name and the
 * fields of one collection entry. Declaration order is preserved because both storefronts must
 * request the same shape.
 */
function attributeSelection(source) {
  const match = source.match(
    /attributes \{\s*([\s\S]*?)\s*([A-Za-z][A-Za-z0-9]*) \{\s*([\s\S]*?)\s*\}\s*\}/,
  );
  assert.ok(match, "attribute selection not found");
  return {
    fields: match[1].trim().split(/\s+/),
    nestedField: match[2],
    valueFields: match[3].trim().split(/\s+/),
  };
}

const snakeToCamel = (name) => name.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());

/** Runs `body` in a child process that imports the Next specifications module. */
function runSpecifications(body) {
  const directory = mkdtempSync(path.join(tmpdir(), "rustok-specifications-"));
  const harness = path.join(directory, "harness.mjs");
  writeFileSync(
    harness,
    [
      `import * as specifications from ${JSON.stringify(specificationsPath)};`,
      "const checks = {};",
      body,
      "process.stdout.write(JSON.stringify(checks));",
      "",
    ].join("\n"),
  );
  try {
    const result = spawnSync(process.execPath, ["--experimental-strip-types", harness], {
      encoding: "utf8",
    });
    assert.equal(result.status, 0, `module harness failed: ${result.stderr || result.stdout}`);
    return JSON.parse(result.stdout);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("storefronts never read the storefront flag or the attribute tables", () => {
  const storefrontSources = [
    ["crate", ownerProjection, "storefront_attributes.rs"],
    ["storefront", storefrontModel, "model.rs"],
    ["storefront", storefrontCore, "core.rs"],
    ["storefront", storefrontGraphql, "graphql_adapter.rs"],
    ["storefront", storefrontNative, "native_server_adapter.rs"],
    ["next", nextProducts, "products.ts"],
    ["next", nextTypes, "types.ts"],
    ["next", nextDetailView, "product-detail-view.tsx"],
    ["next", nextSpecifications, "specifications.ts"],
  ];
  for (const [layer, source, file] of storefrontSources) {
    if (file === "storefront_attributes.rs") continue;
    assert.ok(
      !source.includes("show_on_storefront"),
      `${layer}/${file} must not know the storefront flag; the owner resolves it`,
    );
  }
  // The owner merges the definition flag with the category binding and drops disabled bindings.
  assert.match(ownerProjection, /show_on_storefront/);
  assert.match(ownerProjection, /binding\.visibility_overrides\.show_on_storefront/);
  assert.match(ownerProjection, /binding\.is_disabled/);
});

test("one attribute request shape across Rust and Next", () => {
  const rust = attributeSelection(storefrontGraphql);
  const next = attributeSelection(nextProducts);
  assert.deepEqual(next.fields, rust.fields);
  assert.deepEqual(next.valueFields, rust.valueFields);

  // The TypeScript contract mirrors the same shape, so a field renamed on the wire fails here.
  assert.match(nextTypes, /attributes\?: StorefrontProductAttribute\[\];/);
  const declaredFields = [...rust.fields, rust.nestedField];
  const typescriptFields = [...nextTypes
    .match(/export type StorefrontProductAttribute = \{([\s\S]*?)\n\};/)[1]
    .matchAll(/^\s*([A-Za-z][A-Za-z0-9]*)\??:/gm)].map((field) => field[1]);
  assert.deepEqual(typescriptFields, declaredFields);
  const typescriptValueFields = [...nextTypes
    .match(/export type StorefrontProductAttributeValue = \{([\s\S]*?)\n\};/)[1]
    .matchAll(/^\s*([A-Za-z][A-Za-z0-9]*)\??:/gm)].map((field) => field[1]);
  assert.deepEqual(typescriptValueFields, rust.valueFields);

  // The commerce wire type must expose exactly these fields, in the same order.
  const wireFields = structFields(commerceTypes, "GqlStorefrontProductAttribute").map(snakeToCamel);
  assert.deepEqual(wireFields, [...rust.fields, rust.nestedField]);
  const wireValueFields = structFields(
    commerceTypes,
    "GqlStorefrontProductAttributeValue",
  ).map(snakeToCamel);
  assert.deepEqual(wireValueFields, rust.valueFields);

  // The storefront model is the serde view of the same selection: a field without a rename keeps
  // its Rust name, so every camelCase request field needs its rename.
  const modelFields = structFields(storefrontModel, "ProductAttribute");
  for (const field of [...rust.fields, rust.nestedField]) {
    if (modelFields.includes(field)) continue;
    const snake = field.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
    assert.ok(
      modelFields.includes(snake),
      `ProductAttribute must declare ${field} (as ${snake}) so serde can read the selection`,
    );
    assert.match(
      storefrontModel,
      new RegExp(`#\\[serde\\(rename = "${field}"\\)\\]`),
      `ProductAttribute must rename the ${snake} field to ${field}`,
    );
  }
  assert.deepEqual(structFields(storefrontModel, "ProductAttributeValue"), rust.valueFields);
});

test("the owner block is mapped once per transport and starts empty on admin reads", () => {
  assert.match(
    commerceTypes,
    /attributes: product\n\s+\.storefront_attributes\n\s+\.into_iter\(\)/,
    "GqlProduct must map the owner's storefront attributes",
  );
  assert.ok(
    ownerDto.includes(
      "#[serde(default)]\n    pub storefront_attributes: Vec<StorefrontProductAttributeResponse>",
    ),
    "the detail contract must default the block so older payloads stay readable",
  );
  assert.deepEqual(
    literalFields(storefrontNative, "crate::model::ProductAttribute {"),
    ["code", "label", "value_type", "is_localized", "values"],
  );
  assert.deepEqual(
    literalFields(storefrontNative, "crate::model::ProductAttributeValue {"),
    ["text"],
  );
  assert.match(
    storefrontNative,
    /let attributes = map_product_attributes\(value\.storefront_attributes\);/,
    "the native adapter must fill the model from the owner block",
  );
});

test("boolean vocabulary is identical in the Leptos catalogs and the Next labels", () => {
  const labels = rustLabels(storefrontCore, "product.selected.attribute");
  assert.equal(labels["product.selected.attributeYes"], "Yes");
  assert.equal(labels["product.selected.attributeNo"], "No");

  const english = ftlValues(storefrontEn, "product-selected-attribute");
  const russian = ftlValues(storefrontRu, "product-selected-attribute");
  assert.equal(english["product-selected-attributeYes"], labels["product.selected.attributeYes"]);
  assert.equal(english["product-selected-attributeNo"], labels["product.selected.attributeNo"]);
  assert.equal(russian["product-selected-attributeYes"], "Да");
  assert.equal(russian["product-selected-attributeNo"], "Нет");

  // The section header is translated the same way in both storefronts.
  const heading = rustLabels(storefrontCore, "product.selected.attributes");
  assert.equal(heading["product.selected.attributes"], "Specifications");
  assert.equal(
    ftlValues(storefrontEn, "product-selected-attributes")["product-selected-attributes"],
    heading["product.selected.attributes"],
  );
  assert.equal(
    ftlValues(storefrontRu, "product-selected-attributes")["product-selected-attributes"],
    "Характеристики",
  );

  const nextLabels = runSpecifications(`
    checks.en = specifications.buildProductSpecificationLabels("en-US");
    checks.ru = specifications.buildProductSpecificationLabels("ru");
    checks.fallback = specifications.buildProductSpecificationLabels(null);
  `);
  assert.deepEqual(nextLabels.en, {
    title: heading["product.selected.attributes"],
    yes: labels["product.selected.attributeYes"],
    no: labels["product.selected.attributeNo"],
  });
  assert.deepEqual(nextLabels.ru, {
    title: "Характеристики",
    yes: "Да",
    no: "Нет",
  });
  assert.deepEqual(nextLabels.fallback, nextLabels.en);
});

test("every attribute key used by the view model exists in both catalogs", () => {
  const keys = Object.keys(rustLabels(storefrontCore, "product.selected.attribute"));
  assert.ok(keys.length >= 3, "the view model must translate the block label and the boolean pair");
  for (const key of keys) {
    const catalogKey = ftlKey(key);
    assert.ok(storefrontEn.includes(`${catalogKey} =`), `en.ftl is missing ${catalogKey}`);
    assert.ok(storefrontRu.includes(`${catalogKey} =`), `ru.ftl is missing ${catalogKey}`);
  }
});

test("Next rows localize only the boolean vocabulary and drop valueless attributes", () => {
  const behaviour = runSpecifications(`
    const labels = specifications.buildProductSpecificationLabels("ru");
    const attribute = (code, label, valueType, values) => ({
      code,
      label,
      valueType,
      isLocalized: false,
      values: values.map((text) => ({ text }))
    });
    checks.rows = specifications.buildProductSpecifications(
      {
        attributes: [
          attribute("color", "Цвет", "select", ["Синий"]),
          attribute("waterproof", "Водозащита", "boolean", ["true", "false"]),
          attribute("size", "", "select", ["M", "L"]),
          attribute("weight", "Вес", "decimal", []),
          attribute("note", "Заметка", "text", ["  "])
        ]
      },
      labels
    );
    checks.passthrough = specifications.formatSpecificationValue(
      { valueType: "select" },
      "true",
      labels
    );
    checks.unexpected = specifications.formatSpecificationValue(
      { valueType: "boolean" },
      "maybe",
      labels
    );
    checks.case = specifications.formatSpecificationValue(
      { valueType: "BOOLEAN" },
      " TRUE ",
      labels
    );
    checks.empty = specifications.buildProductSpecifications({ attributes: undefined }, labels);
  `);

  assert.deepEqual(behaviour.rows, [
    { code: "color", label: "Цвет", value: "Синий" },
    { code: "waterproof", label: "Водозащита", value: "Да · Нет" },
    { code: "size", label: "size", value: "M · L" },
  ]);
  // A dictionary value literally named "true" is not a boolean payload.
  assert.equal(behaviour.passthrough, "true");
  // An unexpected boolean payload is printed, not dropped.
  assert.equal(behaviour.unexpected, "maybe");
  assert.equal(behaviour.case, "Да");
  assert.deepEqual(behaviour.empty, []);
});

test("the specification module stays framework-free and the detail view delegates to it", () => {
  assert.ok(
    !/from\s+["'](react|next|@rustok\/)/.test(nextSpecifications),
    "specifications.ts must stay framework-free so both hosts can reuse it",
  );
  assert.match(
    nextDetailView,
    /buildProductSpecifications\(product, specificationLabels\)/,
    "the detail view must render the shared rows",
  );
  assert.ok(
    !nextDetailView.includes("values.join"),
    "the detail view must not join owner values itself",
  );
  // No fallback feature block: the page shows what the cataloguer filled or nothing at all.
  for (const forbidden of ["Fast delivery", "Original quality", "Быстрая доставка"]) {
    assert.ok(
      !nextDetailView.includes(forbidden),
      `the static feature row ${forbidden} must be gone`,
    );
  }
});
