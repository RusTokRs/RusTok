//! Flex capability contracts shared across attached and standalone modes.
//! Extracted from `apps/server` as part of Phase 4.5 and formalized as a
//! capability-only runtime module during Phase 4.6.

use async_trait::async_trait;
use rustok_core::{MigrationSource, Permission, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod attached;
pub mod errors;
pub mod events;
pub mod orchestration;
pub mod parsing;
pub mod registry;
pub mod standalone;

pub struct FlexModule;

pub use attached::{
    delete_attached_localized_values, load_exact_locale_values, load_localized_values_by_locale,
    persist_localized_values, prepare_attached_values_create, prepare_attached_values_update,
    resolve_attached_payload, AttachedEntityRef, PreparedAttachedValuesWrite,
};
pub use errors::{map_flex_error, FlexMappedError, FlexMappedErrorKind};
pub use orchestration::{
    create_field_definition, deactivate_field_definition, find_field_definition,
    invalidate_field_definition_cache, list_field_definitions, list_field_definitions_with_cache,
    reorder_field_definitions, update_field_definition, FieldDefinitionCachePort,
};
pub use parsing::{parse_field_definitions_config, FieldDefinitionsConfigParseError};
pub use registry::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionService, FieldDefinitionView,
    UpdateFieldDefinitionCommand,
};
pub use standalone::{
    create_entry, create_entry_with_event, create_schema, create_schema_with_event, delete_entry,
    delete_entry_with_event, delete_schema, delete_schema_with_event, find_entry, find_schema,
    list_entries, list_schemas, update_entry, update_entry_with_event, update_schema,
    update_schema_with_event, validate_create_entry_command, validate_create_schema_command,
    validate_update_entry_command, validate_update_schema_command, CreateFlexEntryCommand,
    CreateFlexSchemaCommand, FlexEntryView, FlexSchemaView, FlexStandaloneService,
    UpdateFlexEntryCommand, UpdateFlexSchemaCommand,
};

pub use events::{
    flex_entry_created_event, flex_entry_deleted_event, flex_entry_updated_event,
    flex_schema_created_event, flex_schema_deleted_event, flex_schema_updated_event,
};

impl MigrationSource for FlexModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
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
