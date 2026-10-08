#!/usr/bin/env node
// RusTok product attribute validation rule-engine guard.
//
// `product_attributes.validation` and the schema/category `validation_overrides`
// columns existed since the catalog-attribute migration but no runtime path read
// them, so declared constraints (lengths, patterns, numeric/date bounds, allowed
// selections, JSON size, requiredness, required locales) never protected data.
// This guard keeps the complete chain in place:
//   1. one rule engine (parse + merge + enforce + bounded public copy),
//   2. attribute-level base rules merged under schema/category overrides into the
//      effective form so enforcement and reads share one effective object,
//   3. enforcement on both Product and Variant EAV writes,
//   4. `required`/`requiredLocales` gating product publication,
//   5. authoring reachability (owner commands + Commerce GraphQL inputs),
//   6. a structured, bounded public error for rule violations,
//   7. a unit test for every declared rule.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");

const failures = [];

function readRepo(relativePath) {
  try {
    return readFileSync(path.join(repoRoot, relativePath), "utf8");
  } catch (error) {
    failures.push(`${relativePath}: expected readable source (${error.message})`);
    return "";
  }
}

function assertContains(source, value, description) {
  if (!source.includes(value)) {
    failures.push(description);
  }
}

function assertAbsent(source, value, description) {
  if (source.includes(value)) {
    failures.push(description);
  }
}

function countOccurrences(source, value) {
  return source.split(value).length - 1;
}

function functionBody(source, signature) {
  const start = source.indexOf(signature);
  if (start < 0) {
    failures.push(`missing function ${signature}`);
    return "";
  }
  const openBrace = source.indexOf("{", start);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, index + 1);
    }
  }
  failures.push(`unterminated function ${signature}`);
  return "";
}

const paths = {
  engine:
    "crates/modules/rustok-product/src/services/catalog_schema_service/attribute_validation.rs",
  schemaService: "crates/modules/rustok-product/src/services/catalog_schema_service.rs",
  effectiveForms:
    "crates/modules/rustok-product/src/services/catalog_schema_service/effective_forms.rs",
  values: "crates/modules/rustok-product/src/services/catalog_schema_service/values.rs",
  variantValues:
    "crates/modules/rustok-product/src/services/catalog_schema_service/values/variant.rs",
  bindings: "crates/modules/rustok-product/src/services/catalog_schema.rs",
  readPort: "crates/modules/rustok-product/src/catalog_schema_read_port.rs",
  publicError: "crates/modules/rustok-product/src/public_error.rs",
  manifest: "crates/modules/rustok-product/Cargo.toml",
  commerceTypes: "crates/modules/rustok-commerce/src/graphql/types.rs",
  commerceMutations: "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs",
  commerceQuery: "crates/modules/rustok-commerce/src/graphql/query.rs",
  registry: "docs/modules/registry.md",
  audit: "docs/audits/product-module-engineering-audit-2026-10-07.md",
};

const engine = readRepo(paths.engine);
const schemaService = readRepo(paths.schemaService);
const effectiveForms = readRepo(paths.effectiveForms);
const values = readRepo(paths.values);
const variantValues = readRepo(paths.variantValues);
const bindings = readRepo(paths.bindings);
const readPort = readRepo(paths.readPort);
const publicError = readRepo(paths.publicError);
const manifest = readRepo(paths.manifest);
const commerceTypes = readRepo(paths.commerceTypes);
const commerceMutations = readRepo(paths.commerceMutations);
const commerceQuery = readRepo(paths.commerceQuery);

// 1. The engine owns the documented rule schema, parsing, merging and enforcement.
for (const marker of [
  'pub(crate) const ATTRIBUTE_VALIDATION_RULE_PREFIX: &str = "attribute validation rule";',
  "const DECLARED_RULE_KEYS: [&str; 15] = [",
  "pub(crate) struct ProductAttributeValidationRules {",
  "pub(crate) fn validate_declared_rule_keys(",
  "pub(crate) fn merge_product_attribute_validation(",
  "pub(crate) fn parse_product_attribute_validation(",
  "pub(crate) fn validate_attribute_value_rules(",
  "pub(crate) fn attribute_validation_rule_of(",
  "pub(crate) fn attribute_validation_public_message(",
  "pub(crate) fn rule_failure(",
]) {
  assertContains(engine, marker, `${paths.engine}: rule engine surface missing ${marker}`);
}
assertContains(
  engine,
  "merged.remove(key);",
  `${paths.engine}: a null override must be able to clear an inherited rule`
);
assertContains(
  engine,
  'rest.starts_with(" failed for attribute ")',
  `${paths.engine}: the canonical failure message must stay parseable`
);
assertContains(
  engine,
  '"Product attribute value violates the schema validation rules"',
  `${paths.engine}: a bounded fallback public message is required`
);
assertContains(
  manifest,
  'regex = "1.12"',
  `${paths.manifest}: the pattern rule must use the workspace regex engine`
);

const declaredRules = [
  "minLength",
  "maxLength",
  "pattern",
  "min",
  "max",
  "minDate",
  "maxDate",
  "minDatetime",
  "maxDatetime",
  "minSelections",
  "maxSelections",
  "options",
  "required",
  "requiredLocales",
  "maxJsonBytes",
];
for (const rule of declaredRules) {
  const declared = countOccurrences(engine, `"${rule}"`);
  if (declared < 2) {
    failures.push(
      `${paths.engine}: rule \`${rule}\` must be declared and enforced from the shared schema (found ${declared})`
    );
  }
}
const ruleFailureCalls = countOccurrences(engine, "rule_failure(");
if (ruleFailureCalls < 12) {
  failures.push(
    `${paths.engine}: every value rule must fail through rule_failure (found ${ruleFailureCalls})`
  );
}

// 2. Every declared rule carries unit-test coverage inside the engine module.
const engineTests = [
  "declared_rule_keys_reject_typos_and_non_objects",
  "parse_rejects_malformed_known_rules",
  "merge_layers_later_wins_and_null_clears",
  "text_rules_enforce_length_and_pattern",
  "numeric_and_date_rules_enforce_declared_bounds",
  "datetime_rules_enforce_declared_bounds",
  "selection_and_json_rules_enforce_declared_bounds",
  "failure_messages_expose_the_failing_rule_to_the_public_mapper",
  "required_rules_parse_for_publish_gating",
  "empty_rules_skip_enforcement_entirely",
];
if (countOccurrences(engine, "#[test]") < engineTests.length) {
  failures.push(`${paths.engine}: expected at least ${engineTests.length} engine unit tests`);
}
for (const testName of engineTests) {
  assertContains(engine, `fn ${testName}()`, `${paths.engine}: missing unit test ${testName}`);
}
// 3. Attribute-level rules are merged under schema/category overrides once per effective form.
assertContains(
  bindings,
  "pub validation: Value,",
  `${paths.bindings}: the effective binding must carry the merged rule object`
);
assertContains(
  effectiveForms,
  "load_effective_attribute_validation(db, tenant_id, &mut form).await?;",
  `${paths.effectiveForms}: the effective form must merge declared rules into every binding`
);
assertContains(
  effectiveForms,
  "async fn load_effective_attribute_validation<C>(",
  `${paths.effectiveForms}: the merged rule loader is missing`
);
assertContains(
  effectiveForms,
  "attribute_validation::merge_product_attribute_validation(",
  `${paths.effectiveForms}: the merge must go through the shared engine`
);
assertContains(
  effectiveForms,
  "&binding.validation_overrides,",
  `${paths.effectiveForms}: binding overrides must win over the attribute base rules`
);
assertContains(
  effectiveForms,
  "SELECT id, validation FROM product_attributes WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({placeholders})",
  `${paths.effectiveForms}: declared rules must be loaded for the effective bindings`
);
const additionBinding = functionBody(bindings, "fn apply_local_category_bindings(");
assertContains(
  additionBinding,
  "validation: Value::Object(Default::default()),",
  `${paths.bindings}: category-local additions start from the attribute base rules`
);

// 4. Product and Variant writes enforce the merged rules.
assertContains(
  schemaService,
  "is_localized: bool,\n    validation: Value,",
  `${paths.schemaService}: write definitions must expose the declared rule object`
);
assertContains(
  schemaService,
  "SELECT id, value_type, scope, is_localized, validation",
  `${paths.schemaService}: the definition loader must select the validation column`
);
const productValidator = functionBody(schemaService, "fn validate_product_value_patch(");
assertContains(
  productValidator,
  "validation: &Value,",
  `${paths.schemaService}: validate_product_value_patch must accept the merged rules`
);
assertContains(
  productValidator,
  "attribute_validation::validate_attribute_value_rules(",
  `${paths.schemaService}: product attribute writes must enforce the merged rules`
);
assertContains(
  values,
  "let validation_by_attribute = form",
  `${paths.values}: the Product write path must read rules from the effective form`
);
assertContains(
  values,
  "validate_product_value_patch(definition, patch, &options, validation)?;",
  `${paths.values}: the Product write path must pass the merged rules to the validator`
);
const variantValidator = functionBody(variantValues, "fn validate_variant_value_patch(");
assertContains(
  variantValidator,
  "validation: &Value,",
  `${paths.variantValues}: validate_variant_value_patch must accept the merged rules`
);
assertContains(
  variantValidator,
  "attribute_validation::validate_attribute_value_rules(",
  `${paths.variantValues}: Variant attribute writes must enforce the merged rules`
);
assertContains(
  variantValues,
  "validate_variant_value_patch(definition, patch, &options, validation)?;",
  `${paths.variantValues}: the Variant write path must pass the merged rules to the validator`
);
assertContains(
  variantValues,
  "SELECT id, value_type, scope, is_localized, validation FROM product_attributes",
  `${paths.variantValues}: the Variant definition loader must select the validation column`
);
for (const source of [schemaService, values, variantValues]) {
  assertAbsent(
    source,
    "SELECT id, value_type, scope, is_localized FROM product_attributes",
    "attribute definition loads must not drop the declared rule object"
  );
}

// 5. `required` and `requiredLocales` gate publication through the same engine.
const publishRequirements = functionBody(
  values,
  "async fn validate_product_publish_requirements_in"
);
assertContains(
  publishRequirements,
  "attribute_validation::parse_product_attribute_validation(&binding.validation)?",
  `${paths.values}: publish requirements must read the effective rules`
);
assertContains(
  publishRequirements,
  "binding.is_required || rules.required || !rules.required_locales.is_empty()",
  `${paths.values}: required/requiredLocales must make an attribute mandatory at publish`
);
assertContains(
  publishRequirements,
  "load_missing_required_locales(",
  `${paths.values}: declared required locales must be verified before publish`
);
assertContains(
  values,
  "async fn load_missing_required_locales<C>(",
  `${paths.values}: the required-locale probe is missing`
);
assertContains(
  values,
  "JOIN product_attribute_value_translations pavt",
  `${paths.values}: the required-locale probe must read the localized value rows`
);
assertContains(
  values,
  "return Err(attribute_validation::rule_failure(",
  `${paths.values}: required-locale failures must use the canonical rule failure`
);
assertContains(
  values,
  '"requiredLocales",',
  `${paths.values}: required-locale failures must name the failing rule`
);
const newProductRequirements = functionBody(
  values,
  "pub(crate) async fn validate_new_product_publish_requirements_in<C>("
);
assertContains(
  values,
  "Self::validate_new_product_publish_requirements_in(&self.db, tenant_id, primary_category_id)",
  `${paths.values}: public publish-requirement check must delegate to the connection-scoped variant`
);
assertContains(
  newProductRequirements,
  "binding.is_required || rules.required || !rules.required_locales.is_empty()",
  `${paths.values}: new-product publish requirements must honour declared rules`
);

// 6. Authoring commands reject unknown rules and the GraphQL surface can carry them.
assertContains(
  schemaService,
  "attribute_validation::validate_declared_rule_keys(&self.validation)?;",
  `${paths.schemaService}: create_attribute must reject unknown rule keys`
);
assertContains(
  schemaService,
  "attribute_validation::validate_declared_rule_keys(&self.validation_overrides)?;",
  `${paths.schemaService}: schema and category bindings must reject unknown rule keys`
);
if (countOccurrences(schemaService, "validate_declared_rule_keys(&self.validation_overrides)") !== 2) {
  failures.push(
    `${paths.schemaService}: both bind commands must validate their rule overrides`
  );
}
assertContains(
  commerceTypes,
  "pub validation: Option<Json<serde_json::Value>>,",
  `${paths.commerceTypes}: CreateProductAttributeInput must accept declared rules`
);
if (
  countOccurrences(commerceTypes, "pub validation_overrides: Option<Json<serde_json::Value>>,") !== 2
) {
  failures.push(
    `${paths.commerceTypes}: both bind inputs must accept rule overrides`
  );
}
const createAttribute = functionBody(commerceMutations, "async fn create_product_attribute(");
assertContains(
  createAttribute,
  "let validation = match input.validation {",
  `${paths.commerceMutations}: createProductAttribute must read the declared rules`
);
assertContains(
  createAttribute,
  "validation,",
  `${paths.commerceMutations}: createProductAttribute must pass the declared rules to the owner`
);
assertAbsent(
  createAttribute,
  "validation: serde_json::Value::Object(Default::default()),",
  `${paths.commerceMutations}: createProductAttribute must not discard declared rules`
);
for (const signature of [
  "async fn bind_product_attribute_schema_attribute(",
  "async fn bind_catalog_category_attribute(",
]) {
  const body = functionBody(commerceMutations, signature);
  assertContains(
    body,
    "let validation_overrides = match input.validation_overrides {",
    `${paths.commerceMutations}: ${signature} must read the rule overrides`
  );
  assertContains(
    body,
    "validation_overrides,",
    `${paths.commerceMutations}: ${signature} must pass the rule overrides to the owner`
  );
  assertAbsent(
    body,
    "validation_overrides: serde_json::Value::Object(Default::default()),",
    `${paths.commerceMutations}: ${signature} must not discard rule overrides`
  );
}

// 7. Read surfaces expose the merged rules.
assertContains(
  readPort,
  "pub validation: serde_json::Value,",
  `${paths.readPort}: the effective-form projection must expose merged rules`
);
assertContains(
  readPort,
  "validation: binding.validation,",
  `${paths.readPort}: the projection must copy the merged rules`
);
assertContains(
  commerceTypes,
  "pub validation: Json<serde_json::Value>,",
  `${paths.commerceTypes}: the GraphQL effective-form attribute must expose merged rules`
);
assertContains(
  commerceQuery,
  "validation: Json(attribute.validation),",
  `${paths.commerceQuery}: the GraphQL effective-form mapping must pass merged rules`
);

// 8. Rule violations keep a structured, bounded public failure.
assertContains(
  publicError,
  "use crate::services::catalog_schema_service::attribute_validation::{",
  `${paths.publicError}: the public mapper must use the shared rule classifier`
);
assertContains(
  publicError,
  "CommerceError::Validation(message) if attribute_validation_rule_of(message).is_some() => {",
  `${paths.publicError}: rule violations need a dedicated public arm`
);
assertContains(
  publicError,
  '"PRODUCT_ATTRIBUTE_VALIDATION"',
  `${paths.publicError}: rule violations need a stable public code`
);
assertContains(
  publicError,
  "attribute_validation_public_message(rule)",
  `${paths.publicError}: the public copy must stay bounded to the closed rule set`
);
assertContains(
  publicError,
  'CommerceError::Validation(_) => ("Product request is invalid", "PRODUCT_VALIDATION", false),',
  `${paths.publicError}: the existing generic validation mapping must stay intact`
);
assertContains(
  publicError,
  '"attribute_validation"',
  `${paths.publicError}: bounded diagnostics must classify rule violations`
);
assertContains(
  publicError,
  "attribute_validation_rule_of(message).unwrap_or_default()",
  `${paths.publicError}: the guard and the public copy must use one classification`
);

// 9. The remediation stays documented.
assertContains(
  readRepo(paths.registry),
  "verify-product-attribute-validation-rules.mjs",
  `${paths.registry}: registry must list the rule-engine verifier`
);
assertContains(
  readRepo(paths.audit),
  "PROD-VALID-001",
  `${paths.audit}: the audit must keep the finding record`
);

if (failures.length > 0) {
  console.error("product attribute validation rule verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "product attribute validation: declared rules are merged by override priority, enforced on Product and Variant writes, gated at publish, authorable through the API, and reported through a bounded public error"
);
