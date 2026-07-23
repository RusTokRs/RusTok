mod registry;
mod validation;

#[cfg(test)]
mod reference;

pub use registry::{
    LinkPathStep, RegisteredSchema, RegistrationOutcome, SchemaRegistry, SchemaRegistryError,
};
pub use validation::{QueryValidationError, RecordValidationError};
