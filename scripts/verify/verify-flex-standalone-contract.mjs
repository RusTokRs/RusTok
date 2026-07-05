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

function expectNotExists(relativePath, description) {
  if (fs.existsSync(path.join(workspaceRoot, relativePath))) {
    failures.push(`${relativePath}: found ${description}`);
  }
}

expectNotExists(
  "apps/server/docs/flex-phase45-migration-guide.md",
  "server-local Flex migration guide",
);

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
  "crates/flex/src/standalone.rs",
  "pub fn normalize_and_validate_standalone_entry",
  "owner-owned standalone entry normalization and validation helper",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "schema.apply_defaults(&mut data);",
  "standalone entry default application in owner helper",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "schema.strip_unknown(&mut data);",
  "standalone entry unknown-key stripping in owner helper",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "pub fn split_standalone_entry_data",
  "owner-owned standalone entry shared/localized payload split helper",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "pub fn effective_standalone_entry_data",
  "owner-owned standalone entry read payload resolver",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "pub fn merge_standalone_entry_patch",
  "owner-owned PATCH-style standalone entry merge helper",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "pub fn parse_standalone_fields_config",
  "owner-owned standalone fields_config parser",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "pub fn build_standalone_custom_fields_schema",
  "owner-owned standalone custom fields schema builder",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "pub fn serialize_standalone_fields_config",
  "owner-owned standalone fields_config serializer",
);
expectContains(
  "crates/flex/src/standalone.rs",
  "pub fn standalone_localized_field_keys",
  "owner-owned standalone localized field-key resolver",
);
for (const snippet of [
  "pub trait StandaloneSchemaViewSource",
  "pub trait StandaloneSchemaTranslationSource",
  "pub trait StandaloneEntryViewSource",
  "pub fn standalone_schema_view_from_source",
  "pub fn standalone_entry_view_from_source",
]) {
  expectContains(
    "crates/flex/src/standalone.rs",
    snippet,
    "owner-owned standalone row-to-view mapping helper",
  );
}
expectContains(
  "apps/server/src/models/flex_schemas.rs",
  "flex::parse_standalone_fields_config(self.fields_config.clone())",
  "server Flex schema model delegates fields_config parsing to owner helper",
);
expectContains(
  "apps/server/src/models/flex_schemas.rs",
  "flex::build_standalone_custom_fields_schema(self.fields_config.clone())",
  "server Flex schema model delegates CustomFieldsSchema build to owner helper",
);
expectContains(
  "apps/server/src/models/flex_schemas.rs",
  "impl flex::StandaloneSchemaViewSource for Model",
  "server Flex schema model exposes owner-owned view source contract",
);
expectContains(
  "apps/server/src/models/flex_entries.rs",
  "impl flex::StandaloneEntryViewSource for Model",
  "server Flex entry model exposes owner-owned view source contract",
);
expectContains(
  "apps/server/src/models/flex_schema_translations.rs",
  "impl flex::StandaloneSchemaTranslationSource for Model",
  "server Flex schema translation model exposes owner-owned view source contract",
);
expectNotContains(
  "apps/server/src/models/flex_schemas.rs",
  "serde_json::from_value(self.fields_config.clone())",
  "server-owned standalone fields_config parser",
);
expectNotContains(
  "apps/server/src/models/flex_schemas.rs",
  "CustomFieldsSchema::new(self.parse_field_definitions()?)",
  "server-owned standalone CustomFieldsSchema builder",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  ".filter(flex_entry_localized_values::Column::TenantId.eq(tenant_id))",
  "tenant-scoped localized entry row lookup",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "let merged_data = flex::merge_standalone_entry_patch(",
  "SeaORM adapter delegates PATCH-style entry update merge to owner helper",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "flex::split_standalone_entry_data(&normalized, &localized_keys)",
  "SeaORM adapter delegates standalone entry shared/localized split to owner helper",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "flex::standalone_entry_view_from_source(",
  "SeaORM adapter delegates standalone entry view mapping to owner helper",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "flex::standalone_schema_view_from_source(",
  "SeaORM adapter delegates standalone schema view mapping to owner helper",
);
expectNotContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "fn merge_entry_patch(",
  "server-owned standalone entry merge helper",
);
expectNotContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "fn split_entry_data(",
  "server-owned standalone entry split helper",
);
expectNotContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "fn effective_entry_data(",
  "server-owned standalone entry read payload resolver",
);
for (const snippet of [
  "fn schema_to_view(",
  "fn entry_to_view(",
  "flex::FlexSchemaView {",
  "flex::FlexEntryView {",
]) {
  expectNotContains(
    "apps/server/src/services/flex_standalone_service.rs",
    snippet,
    "server-owned standalone row-to-view mapping",
  );
}
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
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "flex::normalize_and_validate_standalone_entry(&custom_fields_schema, data)?;",
  "SeaORM adapter delegates standalone entry normalization to owner helper",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "flex::serialize_standalone_fields_config(",
  "SeaORM adapter delegates standalone fields_config serialization to owner helper",
);
expectContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "flex::standalone_localized_field_keys(&custom_fields_schema)",
  "SeaORM adapter delegates standalone localized field-key resolution to owner helper",
);
expectNotContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "fn localized_field_keys(",
  "server-owned standalone localized field-key resolver",
);
expectNotContains(
  "apps/server/src/services/flex_standalone_service.rs",
  "serde_json::to_value(input.fields_config).unwrap_or_default()",
  "server-owned standalone fields_config serialization for create",
);
if (fs.existsSync(path.join(workspaceRoot, "apps/server/src/services/flex_standalone_validation_service.rs"))) {
  failures.push(
    "apps/server/src/services/flex_standalone_validation_service.rs: server-local standalone validation service must not exist",
  );
}
expectContains(
  "crates/flex/src/rest.rs",
  "pub struct CreateFlexSchemaRequest",
  "owner-owned standalone REST request DTOs",
);
expectContains(
  "crates/flex/src/rest.rs",
  "impl From<FlexSchemaView> for FlexSchemaResponse",
  "owner-owned standalone REST schema response mapping",
);
expectContains(
  "crates/flex/src/rest.rs",
  "impl From<FlexEntryView> for FlexEntryResponse",
  "owner-owned standalone REST entry response mapping",
);
expectContains(
  "crates/flex/src/rest.rs",
  "impl DeleteFlexResponse",
  "owner-owned standalone REST delete response helper",
);
expectContains(
  "crates/flex/src/rest.rs",
  "pub fn success() -> Self",
  "owner-owned standalone REST delete success factory",
);
expectContains(
  "crates/flex/src/rest.rs",
  "pub fn into_command(self) -> Result<CreateFlexSchemaCommand, FieldDefinitionsConfigParseError>",
  "owner-owned standalone REST create-schema command mapping",
);
expectContains(
  "crates/flex/src/rest.rs",
  "pub fn into_command(self, schema_id: Uuid) -> CreateFlexEntryCommand",
  "owner-owned standalone REST create-entry command mapping",
);
expectNotContains(
  "apps/server/src/controllers/flex.rs",
  "pub struct FlexSchemaResponse",
  "server-owned standalone REST response DTO",
);
expectNotContains(
  "apps/server/src/controllers/flex.rs",
  "fn map_schema(",
  "server-owned standalone REST schema response mapping",
);
expectNotContains(
  "apps/server/src/controllers/flex.rs",
  "flex::CreateFlexSchemaCommand",
  "server-owned standalone REST create-schema command mapping",
);
expectNotContains(
  "apps/server/src/controllers/flex.rs",
  "parse_fields_config(",
  "server-owned standalone REST fields_config parser",
);
expectNotContains(
  "apps/server/src/controllers/flex.rs",
  "DeleteFlexResponse { success: true }",
  "server-owned standalone REST delete response construction",
);
expectContains(
  "apps/server/src/controllers/swagger.rs",
  "flex::rest::FlexSchemaResponse",
  "OpenAPI schema registration uses owner-owned Flex REST DTOs",
);
expectNotContains(
  "crates/flex/src/standalone.rs",
  ".update_schema(tenant_id, actor_id, schema_id, input)\n        .update_schema(",
  "duplicate update_schema delegation chain",
);
for (const snippet of [
  "pub trait FieldDefinitionSource",
  "pub fn field_definition_from_source",
  "macro_rules! impl_field_definition_source",
  "pub fn validate_field_definition_create",
  "pub fn field_definition_position_or_next",
  "pub fn field_definition_type_name",
  "pub fn field_definition_label_json",
  "pub fn field_definition_description_json",
  "pub fn field_definition_validation_json",
  "pub fn field_definition_cache_invalidation_target",
  "pub fn field_definition_created_event",
  "pub fn field_definition_updated_event",
  "pub fn field_definition_deleted_event",
]) {
  expectContains(
    "crates/flex/src/registry.rs",
    snippet,
    "owner-owned attached field-definition lifecycle helper",
  );
}
for (const service of [
  "apps/server/src/services/user_field_service.rs",
  "apps/server/src/services/order_field_service.rs",
  "apps/server/src/services/product_field_service.rs",
  "apps/server/src/services/topic_field_service.rs",
]) {
  const production = read(service).split("#[cfg(test)]")[0];
  for (const snippet of [
    "flex::validate_field_definition_create",
    "flex::field_definition_position_or_next",
    "flex::field_definition_type_name",
    "flex::field_definition_label_json",
    "flex::field_definition_description_json",
    "flex::field_definition_validation_json",
    "flex::field_definition_created_event",
    "flex::field_definition_updated_event",
    "flex::field_definition_deleted_event",
  ]) {
    if (!production.includes(snippet)) {
      failures.push(`${service}: expected attached field-definition adapter delegation to ${snippet}`);
    }
  }
  for (const snippet of [
    "is_valid_field_key",
    "DomainEvent::FieldDefinition",
    "EventEnvelope::new(",
    "serde_json::to_value(input.field_type)",
    "serde_json::to_value(&input.label)",
    "serde_json::to_value(d)",
    "serde_json::to_value(v)",
    "serde_json::to_value(label)",
    "serde_json::to_value(desc)",
    "serde_json::to_value(val)",
    "FlexError::TooManyFields",
    "FlexError::DuplicateFieldKey",
    "FlexError::InvalidFieldKey",
  ]) {
    if (production.includes(snippet)) {
      failures.push(`${service}: found server-owned attached field-definition lifecycle policy ${snippet}`);
    }
  }
}
expectContains(
  "apps/server/src/services/field_definition_cache.rs",
  "flex::field_definition_cache_invalidation_target(&envelope.event)",
  "server field-definition cache delegates event taxonomy to flex",
);
for (const snippet of [
  "DomainEvent::FieldDefinitionCreated",
  "DomainEvent::FieldDefinitionUpdated",
  "DomainEvent::FieldDefinitionDeleted",
]) {
  const production = read("apps/server/src/services/field_definition_cache.rs").split("#[cfg(test)]")[0];
  if (production.includes(snippet)) {
    failures.push(
      `apps/server/src/services/field_definition_cache.rs: found server-owned field-definition cache event taxonomy ${snippet}`,
    );
  }
}
for (const model of [
  "apps/server/src/models/user_field_definitions.rs",
  "apps/server/src/models/order_field_definitions.rs",
  "apps/server/src/models/product_field_definitions.rs",
  "apps/server/src/models/topic_field_definitions.rs",
]) {
  const content = read(model);
  for (const snippet of [
    "flex::impl_field_definition_source!(Model);",
    "flex::field_definition_from_source(&self)",
  ]) {
    if (!content.includes(snippet)) {
      failures.push(`${model}: expected field-definition row mapping delegation to ${snippet}`);
    }
  }
  for (const snippet of [
    "serde_json::from_value(serde_json::Value::String",
    "let label: HashMap<String, String> = serde_json::from_value",
    "let validation: Option<ValidationRule> =",
    "Some(FieldDefinition {",
  ]) {
    if (content.includes(snippet)) {
      failures.push(`${model}: found server-owned field-definition row-to-core mapping ${snippet}`);
    }
  }
}
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
