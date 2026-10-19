#!/usr/bin/env node
/**
 * Contract tests for the admin facet panel slice.
 *
 * The admin facet logic is Rust, so these tests cannot execute it; instead they lock the
 * cross-file invariants a refactor would silently break: the operator-visible copy has to exist in
 * both admin locales and match the Rust fallbacks, the admin label copy has to mirror the shared
 * `FacetPanelLabels` field-for-field, the admin facet model has to describe exactly what both
 * transports return, and the panel view has to stay a pure function of the URL.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");

const read = (...segments) => readFileSync(path.join(repoRoot, ...segments), "utf8");

const adminFacets = read("crates/modules/rustok-product/admin/src/facets.rs");
const adminModel = read("crates/modules/rustok-product/admin/src/model.rs");
const adminGraphql = read(
  "crates/modules/rustok-product/admin/src/transport/admin_catalog_graphql.rs",
);
const adminNative = read(
  "crates/modules/rustok-product/admin/src/transport/admin_catalog_native.rs",
);
const sharedPanel = read("crates/ui/rustok-grid/src/facet_panel.rs");
const panelUi = read("crates/modules/rustok-product/admin/src/ui/catalog_facets.rs");

/** Parses `key = value` entries of an FTL file, keeping only `prefix`. */
function ftlValues(locale, prefix) {
  const values = {};
  for (const line of read("crates/modules/rustok-product/admin/locales", locale).split("\n")) {
    const separator = line.indexOf(" = ");
    if (separator < 0) continue;
    const key = line.slice(0, separator).trim();
    if (!key.startsWith(prefix)) continue;
    values[key] = line.slice(separator + 3).trim();
  }
  return values;
}

/** Extracts `t(locale, "<key>", "<default>")` calls from the admin facet module. */
function rustLabelCalls() {
  const calls = [];
  const pattern = /t\(\s*locale,\s*"([^"]+)",\s*\n?\s*"((?:[^"\\]|\\.)*)"/g;
  for (const match of adminFacets.matchAll(pattern)) {
    calls.push({ key: match[1], fallback: match[2] });
  }
  return calls;
}

/** FTL key for a dotted message key, e.g. `product.list.facetsLabel` -> `product-list-facetsLabel`. */
const ftlKey = (key) => key.replaceAll(".", "-");

/** Field names of a Rust named-field struct, in declaration order. */
function structFields(source, name) {
  const match = source.match(new RegExp(`pub struct ${name} \\{([\\s\\S]*?)\\n\\}`));
  assert.ok(match, `struct ${name} not found`);
  return [...match[1].matchAll(/^\s*pub\s+([a-z_][a-z0-9_]*):/gm)].map((field) => field[1]);
}

/** `#[serde(rename = "…")]` names declared on the fields of a struct, in order. */
function structSerdeNames(source, name) {
  const match = source.match(new RegExp(`pub struct ${name} \\{([\\s\\S]*?)\\n\\}`));
  assert.ok(match, `struct ${name} not found`);
  return [...match[1].matchAll(/#\[serde\(rename = "([^"]+)"\)\]/g)].map((rename) => rename[1]);
}

/** Column names of the GraphQL selection set of a field call. */
function graphqlSelection(source, field) {
  const match = source.match(
    new RegExp(`${field}\\([^)]*\\) \\{ ([^{}]*?)(?: values \\{ ([^{}]*?) \\})? \\}`),
  );
  assert.ok(match, `GraphQL selection for ${field} not found`);
  return {
    fields: match[1].trim().split(/\s+/),
    valueFields: (match[2] ?? "").trim().split(/\s+/).filter(Boolean),
  };
}

test("admin facet copy exists in both locales", () => {
  const english = ftlValues("en.ftl", "product-list-facets");
  const russian = ftlValues("ru.ftl", "product-list-facets");
  const calls = rustLabelCalls();

  assert.equal(calls.length, 7, `expected 7 localized facet labels, saw ${calls.length}`);
  for (const { key } of calls) {
    const localized = ftlKey(key);
    assert.ok(localized in english, `en.ftl is missing ${localized}`);
    assert.ok(localized in russian, `ru.ftl is missing ${localized}`);
  }
  assert.deepEqual(
    Object.keys(english).sort(),
    Object.keys(russian).sort(),
    "admin facet locales drifted apart",
  );
});

test("Rust fallbacks equal the English locale", () => {
  const english = ftlValues("en.ftl", "product-list-facets");
  for (const { key, fallback } of rustLabelCalls()) {
    assert.equal(fallback, english[ftlKey(key)], `${key}: Rust fallback and en.ftl disagree`);
  }
  assert.match(
    adminFacets,
    /count_template:\s*"\(\{count\}\)"\.to_string\(\)/,
    "the numeric bucket template is code-owned and must use the shared textual placeholder",
  );
});

test("locale-independent copy stays shared", () => {
  const english = ftlValues("en.ftl", "product-list-facets");
  const russian = ftlValues("ru.ftl", "product-list-facets");

  assert.ok(!("product-list-facetsCount" in english) && !("product-list-facetsCount" in russian));
  for (const key of ["product-list-facetsSelected", "product-list-facetsUnselected"]) {
    assert.equal(english[key], russian[key], `${key} must not be translated`);
  }
  for (const key of [
    "product-list-facetsLabel",
    "product-list-facetsClear",
    "product-list-facetsEmpty",
    "product-list-facetsUnbounded",
    "product-list-facetsTruncated",
  ]) {
    assert.notEqual(english[key], russian[key], `${key} must be translated`);
  }
});

test("admin labels mirror the shared panel labels", () => {
  assert.deepEqual(
    structFields(adminFacets, "ProductAdminFacetLabels"),
    structFields(sharedPanel, "FacetPanelLabels"),
    "admin facet labels drifted from the shared grid panel labels",
  );
});

test("admin facet model describes what both transports return", () => {
  const { fields, valueFields } = graphqlSelection(adminGraphql, "adminProductCatalogFacets");
  assert.deepEqual(
    fields,
    ["code", "label", "valueType", "isLocalized", "isEnumerable", "isTruncated", "totalProducts"],
    "admin facet GraphQL selection drifted",
  );
  assert.deepEqual(valueFields, ["value", "label", "count"]);

  assert.deepEqual(
    structFields(adminModel, "AdminCatalogFacet").filter(
      (field) => !["code", "label", "values"].includes(field),
    ),
    ["value_type", "is_localized", "is_enumerable", "is_truncated", "total_products"],
    "admin facet model fields drifted",
  );
  assert.deepEqual(
    structSerdeNames(adminModel, "AdminCatalogFacet"),
    fields.slice(2),
    "serde names must match the GraphQL selection names",
  );
  assert.deepEqual(structFields(adminModel, "AdminCatalogFacetValue"), valueFields);

  assert.match(
    adminNative,
    /fn map_admin_catalog_facets\(/,
    "native transport must map the owner rows into the admin model",
  );
  assert.match(
    adminNative,
    /AdminCatalogFacet \{/,
    "native transport must build the shared admin facet model",
  );
});

test("admin panel view stays a function of the URL", () => {
  assert.doesNotMatch(
    panelUi,
    /use_state|RwSignal|create_signal|use_context/,
    "the admin facet panel must render from its props only",
  );
  for (const marker of ["href=value.href", "aria-pressed=", "marker"]) {
    assert.ok(panelUi.includes(marker), `panel view is missing ${marker}`);
  }
});


// Source scenarios only; intentionally unrun during the no-tests remediation session.
test("both native catalog reads propagate trusted context rejection before owner reads", () => {
  const blocks = adminNative.split(/async fn product_admin_catalog_(?:list|facets)_native\(/).slice(1);
  assert.equal(blocks.length, 2);
  for (const [index, block] of blocks.entries()) {
    const operation = index === 0 ? "admin_catalog_list" : "admin_catalog_facets";
    assert.ok(block.includes(`require_request_context("${operation}").await?;`));
    assert.ok(block.indexOf("require_request_context(") < block.indexOf("CatalogService::new("));
    assert.ok(block.includes("requested_tenant_id != tenant.id || auth.tenant_id != tenant.id"));
    assert.ok(block.includes(".unwrap_or(request_context.locale)"));
    assert.ok(!block.includes(".ok()"));
  }
});
test("admin context rejection diagnostics use only static fields and fresh owner reference", () => {
  const helper = adminNative.slice(adminNative.indexOf("async fn require_request_context("), adminNative.indexOf("fn map_product_service_error("));
  assert.ok(helper.includes(".map_err(|_|"));
  assert.ok(helper.includes("ProductPublicError::internal()"));
  assert.ok(helper.includes("correlation_id = %public.correlation_id"));
  assert.ok(!/error = [?%]error|context\.correlation_id|\.ok\(\)/.test(helper));
});
