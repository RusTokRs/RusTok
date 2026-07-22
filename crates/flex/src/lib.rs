//! Flex capability contracts shared across attached and standalone modes.
//! Extracted from `apps/server` as part of Phase 4.5 and formalized as a
//! capability-only runtime module during Phase 4.6.

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod attached;
pub mod cache_generation;
pub mod errors;
pub mod events;
pub mod graphql;
mod migrations;
pub mod orchestration;
pub mod parsing;
pub mod registry;
pub mod rest;
pub mod standalone;

pub struct FlexModule;

pub use attached::{
    AttachedEntityRef, PreparedAttachedValuesWrite, delete_attached_localized_values,
    load_exact_locale_values, load_localized_values_by_locale, persist_localized_values,
    prepare_attached_values_create, prepare_attached_values_update, resolve_attached_payload,
};
pub use errors::{FlexMappedError, FlexMappedErrorKind, map_flex_error};
pub use orchestration::{
    FieldDefinitionCachePort, create_field_definition, deactivate_field_definition,
    find_field_definition, invalidate_field_definition_cache, list_field_definitions,
    list_field_definitions_with_cache, reorder_field_definitions, update_field_definition,
};
pub use parsing::{FieldDefinitionsConfigParseError, parse_field_definitions_config};
pub use registry::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionService, FieldDefinitionSource,
    FieldDefinitionView, FieldDefinitionViewSource, UpdateFieldDefinitionCommand,
    field_definition_cache_invalidation_target, field_definition_created_event,
    field_definition_deleted_event, field_definition_description_json,
    field_definition_from_source, field_definition_label_json, field_definition_position_or_next,
    field_definition_type_name, field_definition_updated_event, field_definition_validation_json,
    validate_field_definition_create,
};
pub use rest::{
    CreateFlexEntryRequest, CreateFlexSchemaRequest, DeleteFlexResponse, FlexEntryResponse,
    FlexSchemaResponse, UpdateFlexEntryRequest, UpdateFlexSchemaRequest,
};
pub use standalone::{
    CreateFlexEntryCommand, CreateFlexSchemaCommand, FlexEntryView, FlexSchemaView,
    FlexStandaloneService, StandaloneEntryViewSource, StandaloneSchemaTranslationSource,
    StandaloneSchemaViewSource, UpdateFlexEntryCommand, UpdateFlexSchemaCommand,
    build_standalone_custom_fields_schema, create_entry, create_entry_with_event, create_schema,
    create_schema_with_event, delete_entry, delete_entry_with_event, delete_schema,
    delete_schema_with_event, effective_standalone_entry_data, find_entry, find_schema,
    list_entries, list_schemas, merge_standalone_entry_patch,
    normalize_and_validate_standalone_entry, parse_standalone_fields_config,
    serialize_standalone_fields_config, split_standalone_entry_data,
    standalone_entry_view_from_source, standalone_localized_field_keys,
    standalone_schema_view_from_source, update_entry, update_entry_with_event, update_schema,
    update_schema_with_event, validate_create_entry_command, validate_create_schema_command,
    validate_optional_standalone_uuid, validate_standalone_uuid, validate_update_entry_command,
    validate_update_schema_command,
};

pub use events::{
    flex_entry_created_event, flex_entry_deleted_event, flex_entry_updated_event,
    flex_schema_created_event, flex_schema_deleted_event, flex_schema_updated_event,
};

impl MigrationSource for FlexModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[async_trait]
impl RusToKModule for FlexModule {
    fn slug(&self) -> &'static str {
        "flex"
    }

    fn name(&self) -> &'static str {
        "Flex"
    }

    fn description(&self) -> &'static str {
        "Capability-only custom fields runtime for attached and standalone extension flows"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::FLEX_SCHEMAS_CREATE,
            Permission::FLEX_SCHEMAS_READ,
            Permission::FLEX_SCHEMAS_UPDATE,
            Permission::FLEX_SCHEMAS_DELETE,
            Permission::FLEX_SCHEMAS_LIST,
            Permission::FLEX_SCHEMAS_MANAGE,
            Permission::FLEX_ENTRIES_CREATE,
            Permission::FLEX_ENTRIES_READ,
            Permission::FLEX_ENTRIES_UPDATE,
            Permission::FLEX_ENTRIES_DELETE,
            Permission::FLEX_ENTRIES_LIST,
            Permission::FLEX_ENTRIES_MANAGE,
        ]
    }
}
