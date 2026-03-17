//! Flex attached-mode shared contracts.
//! Extracted from `apps/server` as part of Phase 4.5.

pub mod errors;
pub mod orchestration;
pub mod registry;

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
