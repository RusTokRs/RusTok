//! Flex attached-mode shared contracts.
//! Extracted from `apps/server` as part of Phase 4.5.

pub mod registry;

pub use registry::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionService, FieldDefinitionView,
    UpdateFieldDefinitionCommand,
};
