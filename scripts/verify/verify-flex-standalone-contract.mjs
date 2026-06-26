import fs from "node:fs";
import path from "node:path";

const workspaceRoot = process.cwd();
const failures = [];

function read(relativePath) {
  return fs.readFileSync(path.join(workspaceRoot, relativePath), "utf8");
}

function expectContains(relativePath, snippet, description) {
  const content = read(relativePath);
  if (!content.includes(snippet)) {
    failures.push(`${relativePath}: expected ${description}`);
  }
}

function expectNotContains(relativePath, snippet, description) {
  const content = read(relativePath);
  if (content.includes(snippet)) {
    failures.push(`${relativePath}: found ${description}`);
  }
}

function expectMatch(relativePath, pattern, description) {
  const content = read(relativePath);
  if (!pattern.test(content)) {
    failures.push(`${relativePath}: expected ${description}`);
  }
}

expectContains(
  "crates/flex/src/standalone.rs",
  "const MAX_STANDALONE_FIELDS_PER_SCHEMA: usize = 50;",
  "standalone schema field-count persistence guardrail",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "validate_standalone_uuid(input.schema_id, \"schema_id\")?;",
  "create-entry schema_id nil UUID validation",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "validate_optional_standalone_uuid(actor_id, \"actor_id\")?;",
  "actor_id nil UUID validation on orchestration boundaries",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "standalone flex entries cannot attach to flex_entry; max relation depth is 1",
  "max relation depth guardrail for standalone Flex entries",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "validate_definition_shape(def)?;",
  "field-definition shape validation before adapter writes",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "regex::Regex::new(pattern)",
  "regex validation for pattern rules",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "definition.field_type.requires_options()",
  "select option presence validation",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "CustomFieldsSchema::new(vec![definition.clone()])",
  "default value validation through core schema rules",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "if !is_valid_locale_key(locale) || value.trim().is_empty()",
  "localized map locale/value normalization validation",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "position must be unique within standalone schema fields_config",
  "unique field position validation for deterministic standalone schema ordering",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "status must already be normalized without surrounding whitespace",
  "status normalization guardrail",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  ".filter(flex_entry_localized_values::Column::TenantId.eq(tenant_id))",
  "tenant-scoped localized entry row lookup",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "let merged_data = Self::merge_entry_patch(",
  "PATCH-style entry update merge preserving omitted values",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "validate_update_schema_command(&input)?;",
  "direct SeaORM schema update validation",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "validate_update_entry_command(&input)?;",
  "direct SeaORM entry update validation",
);
expectNotContains(
  "crates/flex/src/standalone.rs",
  ".update_schema(tenant_id, actor_id, schema_id, input)\n        .update_schema(",
  "duplicate update_schema delegation chain",
);
expectMatch(
  "crates/flex/docs/implementation-plan.md",
  /standalone contract validators now .*schema descriptions/s,
  "Phase 5 implementation-plan checkpoint for latest no-compile validator hardening",
);

if (failures.length > 0) {
  console.error("flex standalone contract drift detected:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("OK  flex standalone contract");
