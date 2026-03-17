//! Flex attached-mode shared contracts.
//! Extracted from `apps/server` as part of Phase 4.5.

pub mod errors;
pub mod events;
pub mod orchestration;
pub mod registry;
pub mod standalone;

pub use errors::{map_flex_error, FlexMappedError, FlexMappedErrorKind};
pub use orchestration::{
    create_field_definition, deactivate_field_definition, find_field_definition,
    invalidate_field_definition_cache, list_field_definitions, list_field_definitions_with_cache,
    reorder_field_definitions, update_field_definition, FieldDefinitionCachePort,
};
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
