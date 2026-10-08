#!/usr/bin/env node
// Fixture coverage for verify-product-attribute-validation-rules.mjs.
//
// Each fixture is a synthetic repository root: one canonical tree that must pass,
// plus targeted regressions that must fail for the right reason (a missing rule,
// a dropped merge, an unenforced write path, a discarded GraphQL rule object, or
// a public error that stops being structured).

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-attribute-validation-rules.mjs"
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function runVerifier(root) {
  return spawnSync(process.execPath, [scriptPath], {
    encoding: "utf8",
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
  });
}

const RULE_NAMES = [
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

const ENGINE_TESTS = [
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

function engineSource() {
  const ruleList = RULE_NAMES.map((rule) => `    "${rule}",`).join("\n");
  const failures = RULE_NAMES.map(
    (rule) => `        return Err(rule_failure(attribute_id, "${rule}", "detail"));`
  ).join("\n");
  const tests = ENGINE_TESTS.map(
    (name) => `    #[test]\n    fn ${name}() { assert!(true); }`
  ).join("\n");
  return `use super::*;
pub(crate) const ATTRIBUTE_VALIDATION_RULE_PREFIX: &str = "attribute validation rule";
const DECLARED_RULE_KEYS: [&str; 15] = [
${ruleList}
];
pub(crate) struct ProductAttributeValidationRules {
    pub(crate) max_length: Option<u32>,
}
pub(crate) fn validate_declared_rule_keys(value: &Value) -> CommerceResult<()> { Ok(()) }
pub(crate) fn merge_product_attribute_validation(base: &Value, overrides: &Value) -> Value {
    let mut merged = base.as_object().cloned().unwrap_or_default();
    if let Some(overrides) = overrides.as_object() {
        for (key, value) in overrides {
            if value.is_null() {
                merged.remove(key);
            }
        }
    }
    Value::Object(merged)
}
pub(crate) fn parse_product_attribute_validation(value: &Value) -> CommerceResult<ProductAttributeValidationRules> { todo!() }
pub(crate) fn validate_attribute_value_rules(attribute_id: Uuid) -> CommerceResult<()> {
${failures}
    Ok(())
}
pub(crate) fn attribute_validation_rule_of(message: &str) -> Option<&str> {
    let rest = message.strip_prefix(ATTRIBUTE_VALIDATION_RULE_PREFIX)?;
    let rest = rest.strip_prefix(" \`")?;
    let (rule, rest) = rest.split_once('\`')?;
    rest.starts_with(" failed for attribute ").then_some(rule)
}
pub(crate) fn attribute_validation_public_message(rule: &str) -> &'static str {
    match rule {
        _ => "Product attribute value violates the schema validation rules",
    }
}
pub(crate) fn rule_failure(attribute_id: Uuid, rule: &str, detail: impl std::fmt::Display) -> CommerceError { todo!() }
${tests}
`;
}

const CANONICAL = {
  "crates/modules/rustok-product/src/services/catalog_schema_service/attribute_validation.rs":
    null,
  "crates/modules/rustok-product/src/services/catalog_schema_service.rs": `struct ProductAttributeWriteDefinitionRow {
    id: Uuid,
    value_type: String,
    scope: String,
    is_localized: bool,
    validation: Value,
}
fn load() -> String {
    "SELECT id, value_type, scope, is_localized, validation FROM product_attributes".to_string()
}
impl CreateProductAttributeInput {
    fn validate(&self) -> CommerceResult<()> {
        attribute_validation::validate_declared_rule_keys(&self.validation)?;
        Ok(())
    }
}
impl BindSchemaAttributeInput {
    fn validate(&self) -> CommerceResult<()> {
        attribute_validation::validate_declared_rule_keys(&self.validation_overrides)?;
        Ok(())
    }
}
impl BindCategoryAttributeInput {
    fn validate(&self) -> CommerceResult<()> {
        attribute_validation::validate_declared_rule_keys(&self.validation_overrides)?;
        Ok(())
    }
}
fn validate_product_value_patch(
    definition: &ProductAttributeWriteDefinitionRow,
    patch: &ProductAttributeValuePatch,
    options: &HashMap<Uuid, Uuid>,
    validation: &Value,
) -> CommerceResult<()> {
    attribute_validation::validate_attribute_value_rules(patch.attribute_id, validation)?;
    Ok(())
}
`,
  "crates/modules/rustok-product/src/services/catalog_schema_service/effective_forms.rs": `async fn load_effective_attribute_validation<C>(
    db: &C,
    tenant_id: Uuid,
    form: &mut EffectiveProductForm,
) -> CommerceResult<()> {
    let _ = "SELECT id, validation FROM product_attributes WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({placeholders})";
    binding.validation = attribute_validation::merge_product_attribute_validation(
        &base,
        &binding.validation_overrides,
    );
    Ok(())
}
async fn load_effective_form_for_category_in<C>() {
    load_effective_attribute_validation(db, tenant_id, &mut form).await?;
}
`,
  "crates/modules/rustok-product/src/services/catalog_schema_service/values.rs": `async fn validate_product_publish_requirements_in<C>() {
    let rules = attribute_validation::parse_product_attribute_validation(&binding.validation)?;
    if binding.is_required || rules.required || !rules.required_locales.is_empty() {
        required_attribute_ids.push(binding.attribute_id);
    }
    let missing_locales = load_missing_required_locales(conn, tenant_id, product_id).await?;
    return Err(attribute_validation::rule_failure(
        attribute_id,
        "requiredLocales",
        "missing locales",
    ));
}
async fn load_missing_required_locales<C>(conn: &C) -> CommerceResult<()> {
    let _ = "JOIN product_attribute_value_translations pavt";
    Ok(())
}
pub async fn validate_new_product_publish_requirements(&self) -> CommerceResult<()> {
    if binding.is_required || rules.required || !rules.required_locales.is_empty() {
        required_attribute_ids.push(binding.attribute_id);
    }
    Ok(())
}
pub async fn save_product_attribute_values(&self) -> CommerceResult<()> {
    let validation_by_attribute = form
        .attributes
        .iter()
        .map(|binding| (binding.attribute_id, binding.validation.clone()))
        .collect::<HashMap<_, _>>();
    validate_product_value_patch(definition, patch, &options, validation)?;
    Ok(())
}
`,
  "crates/modules/rustok-product/src/services/catalog_schema_service/values/variant.rs": `fn load_variant_patch_definitions() -> &'static str {
    "SELECT id, value_type, scope, is_localized, validation FROM product_attributes"
}
pub async fn save_variant_attribute_values(&self) -> CommerceResult<()> {
    let validation_by_attribute = form
        .attributes
        .iter()
        .map(|binding| (binding.attribute_id, binding.validation.clone()))
        .collect::<HashMap<_, _>>();
    validate_variant_value_patch(definition, patch, &options, validation)?;
    Ok(())
}
fn validate_variant_value_patch(
    definition: &ProductAttributeWriteDefinitionRow,
    patch: &ProductAttributeValuePatch,
    options: &HashMap<Uuid, Uuid>,
    validation: &Value,
) -> CommerceResult<()> {
    attribute_validation::validate_attribute_value_rules(patch.attribute_id, validation)?;
    Ok(())
}
`,
  "crates/modules/rustok-product/src/services/catalog_schema.rs": `pub struct AttributeBinding {
    pub validation_overrides: Value,
    pub validation: Value,
    pub source: EffectiveAttributeSource,
}
fn apply_local_category_bindings(bindings: &mut Vec<AttributeBinding>) {
    bindings.push(AttributeBinding {
        validation: Value::Object(Default::default()),
        source: EffectiveAttributeSource::CategoryLocal,
    });
}
`,
  "crates/modules/rustok-product/src/catalog_schema_read_port.rs": `pub struct ProductEffectiveFormAttributeProjection {
    pub validation: serde_json::Value,
}
fn project() {
    validation: binding.validation,
}
`,
  "crates/modules/rustok-product/src/public_error.rs": `use crate::services::catalog_schema_service::attribute_validation::{
    attribute_validation_public_message, attribute_validation_rule_of,
};
pub fn map_product_public_error(error: &CommerceError) -> ProductPublicError {
    let (message, code, retryable) = match error {
        CommerceError::Validation(message) if attribute_validation_rule_of(message).is_some() => {
            let rule = attribute_validation_rule_of(message).unwrap_or_default();
            (
                attribute_validation_public_message(rule),
                "PRODUCT_ATTRIBUTE_VALIDATION",
                false,
            )
        }
        CommerceError::Validation(_) => ("Product request is invalid", "PRODUCT_VALIDATION", false),
        _ => ("Product operation could not be completed safely", "PRODUCT_OPERATION_FAILED", false),
    };
    let _ = (message, code, retryable);
}
fn facts(error: &CommerceError) -> &'static str {
    match error {
        CommerceError::Validation(message) => {
            if attribute_validation_rule_of(message).is_some() {
                "attribute_validation"
            } else {
                "validation"
            }
        }
        _ => "core",
    }
}
`,
  "crates/modules/rustok-product/Cargo.toml": `[dependencies]
serde.workspace = true
regex = "1.12"
`,
  "crates/modules/rustok-commerce/src/graphql/types.rs": `pub struct CreateProductAttributeInput {
    pub code: String,
    pub validation: Option<Json<serde_json::Value>>,
}
pub struct BindSchemaAttributeInput {
    pub schema_id: Uuid,
    pub validation_overrides: Option<Json<serde_json::Value>>,
}
pub struct BindCategoryAttributeInput {
    pub category_id: Uuid,
    pub validation_overrides: Option<Json<serde_json::Value>>,
}
pub struct GqlProductEffectiveFormAttribute {
    pub validation: Json<serde_json::Value>,
}
`,
  "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs": `async fn create_product_attribute(&self) {
    let validation = match input.validation {
        Some(value) => value.0,
        None => serde_json::Value::Object(Default::default()),
    };
    let domain_input = rustok_product::services::CreateProductAttributeInput {
        validation,
        default_value: None,
    };
}
async fn bind_product_attribute_schema_attribute(&self) {
    let validation_overrides = match input.validation_overrides {
        Some(value) => value.0,
        None => serde_json::Value::Object(Default::default()),
    };
    let domain_input = rustok_product::services::BindSchemaAttributeInput {
        validation_overrides,
        metadata: serde_json::Value::Object(Default::default()),
    };
}
async fn bind_catalog_category_attribute(&self) {
    let validation_overrides = match input.validation_overrides {
        Some(value) => value.0,
        None => serde_json::Value::Object(Default::default()),
    };
    let domain_input = rustok_product::services::BindCategoryAttributeInput {
        validation_overrides,
        metadata: serde_json::Value::Object(Default::default()),
    };
}
`,
  "crates/modules/rustok-commerce/src/graphql/query.rs": `fn project(attribute: ProductEffectiveFormAttributeProjection) {
    let _ = GqlProductEffectiveFormAttribute {
        validation: Json(attribute.validation),
    };
}
`,
  "docs/modules/registry.md": `- scripts/verify/verify-product-attribute-validation-rules.mjs
`,
  "docs/audits/product-module-engineering-audit-2026-10-07.md": `#### PROD-VALID-001 (P2). validation / validation_overrides не применяются
`,
};

function writeFixture(root, overrides = {}) {
  for (const [relativePath, content] of Object.entries(CANONICAL)) {
    const resolved =
      relativePath in overrides
        ? overrides[relativePath]
        : content === null
          ? engineSource()
          : content;
    if (resolved === undefined) continue;
    write(root, relativePath, resolved);
  }
  for (const [relativePath, content] of Object.entries(overrides)) {
    if (relativePath in CANONICAL && content === undefined) {
      rmSync(path.join(root, relativePath), { force: true });
    }
  }
}

function withFixture(overrides, body) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-attribute-rules-"));
  try {
    writeFixture(root, overrides);
    body(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("canonical fixture passes", () => {
  withFixture({}, (root) => {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr + result.stdout);
    assert.match(result.stdout, /declared rules are merged by override priority/);
  });
});

test("a declared rule that is never enforced fails", () => {
  const engine = engineSource().replace('rule_failure(attribute_id, "maxJsonBytes", "detail")', "");
  withFixture(
    {
      "crates/modules/rustok-product/src/services/catalog_schema_service/attribute_validation.rs":
        engine,
    },
    (root) => {
      const result = runVerifier(root);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /maxJsonBytes/);
    }
  );
});

test("a declared rule without unit coverage fails", () => {
  const engine = engineSource().replace(
    "fn required_rules_parse_for_publish_gating() { assert!(true); }",
    "fn uncovered() { assert!(true); }"
  );
  withFixture(
    {
      "crates/modules/rustok-product/src/services/catalog_schema_service/attribute_validation.rs":
        engine,
    },
    (root) => {
      const result = runVerifier(root);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /missing unit test required_rules_parse_for_publish_gating/);
    }
  );
});

test("an effective form that skips the rule merge fails", () => {
  withFixture(
    {
      "crates/modules/rustok-product/src/services/catalog_schema_service/effective_forms.rs": `async fn load_effective_attribute_validation<C>() {
    Ok(())
}
`,
    },
    (root) => {
      const result = runVerifier(root);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /merge declared rules into every binding/);
    }
  );
});

test("a Product write path that drops the merged rules fails", () => {
  withFixture(
    {
      "crates/modules/rustok-product/src/services/catalog_schema_service/values.rs": `${CANONICAL["crates/modules/rustok-product/src/services/catalog_schema_service/values.rs"].replace(
        "validate_product_value_patch(definition, patch, &options, validation)?;",
        "validate_product_value_patch(definition, patch, &options)?;"
      )}`,
    },
    (root) => {
      const result = runVerifier(root);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /pass the merged rules to the validator/);
    }
  );
});

test("an attribute definition load without the rule column fails", () => {
  withFixture(
    {
      "crates/modules/rustok-product/src/services/catalog_schema_service/values/variant.rs": `fn load_variant_patch_definitions() -> &'static str {
    "SELECT id, value_type, scope, is_localized FROM product_attributes"
}
fn validate_variant_value_patch(
    validation: &Value,
) -> CommerceResult<()> {
    attribute_validation::validate_attribute_value_rules(patch.attribute_id, validation)?;
    Ok(())
}
`,
    },
    (root) => {
      const result = runVerifier(root);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /must not drop the declared rule object/);
    }
  );
});

test("a GraphQL create mutation that discards declared rules fails", () => {
  withFixture(
    {
      "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs": CANONICAL[
        "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs"
      ].replace(
        "        validation,\n        default_value: None,",
        "        validation: serde_json::Value::Object(Default::default()),\n        default_value: None,"
      ),
    },
    (root) => {
      const result = runVerifier(root);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /must not discard declared rules/);
    }
  );
});

test("a public mapper without the structured rule arm fails", () => {
  const mapper = CANONICAL["crates/modules/rustok-product/src/public_error.rs"]
    .replace(
      "CommerceError::Validation(message) if attribute_validation_rule_of(message).is_some() => {",
      "CommerceError::Validation(message) if false => {"
    )
    .replace('"PRODUCT_ATTRIBUTE_VALIDATION"', '"PRODUCT_VALIDATION"');
  withFixture(
    { "crates/modules/rustok-product/src/public_error.rs": mapper },
    (root) => {
      const result = runVerifier(root);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /dedicated public arm/);
      assert.match(result.stderr, /stable public code/);
    }
  );
});

test("a publish gate without the required-locale probe fails", () => {
  const values = CANONICAL[
    "crates/modules/rustok-product/src/services/catalog_schema_service/values.rs"
  ]
    .replace("load_missing_required_locales(conn, tenant_id, product_id).await?", "Ok(())")
    .replace("async fn load_missing_required_locales<C>(conn: &C)", "async fn unused_probe<C>(conn: &C)");
  withFixture(
    {
      "crates/modules/rustok-product/src/services/catalog_schema_service/values.rs": values,
    },
    (root) => {
      const result = runVerifier(root);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /required locales must be verified before publish/);
    }
  );
});

test("an unregistered verifier fails the registry check", () => {
  withFixture(
    { "docs/modules/registry.md": "- no verifier listed\n" },
    (root) => {
      const result = runVerifier(root);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /registry must list the rule-engine verifier/);
    }
  );
});
