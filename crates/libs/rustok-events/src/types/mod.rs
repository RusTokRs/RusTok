pub mod domain_event;
pub mod envelope;
mod validation;

#[cfg(test)]
mod tests;

pub use domain_event::DomainEvent;
pub(crate) use envelope::timestamp_serde;
pub use envelope::{EventEnvelope, EventEnvelopeError};
