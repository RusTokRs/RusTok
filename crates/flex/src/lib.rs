//! Flex attached-mode shared contracts.
//! Extracted from `apps/server` as part of Phase 4.5.

pub mod orchestration;
pub mod registry;

pub use orchestration::{
    create_field_definition, deactivate_field_definition, find_field_definition,
    list_field_definitions, reorder_field_definitions, update_field_definition,
    FieldDefinitionCachePort,
};
pub use registry::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionService, FieldDefinitionView,
    UpdateFieldDefinitionCommand,
};
