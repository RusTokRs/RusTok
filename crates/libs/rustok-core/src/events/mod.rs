pub mod backpressure;
mod bus;
mod consumer;
mod handler;
mod memory;
mod schema;
mod transport;
mod types;
pub mod validation;

pub use backpressure::{
    BackpressureConfig, BackpressureController, BackpressureError, BackpressureMetrics,
    BackpressureState,
};
pub use bus::{EventBus, EventBusStats};
pub use consumer::EventConsumerRuntime;
pub use handler::{
    DispatcherConfig, EventDispatcher, EventHandler, HandlerBuilder, HandlerResult,
    RunningDispatcher,
};
pub use memory::MemoryTransport;
pub use schema::{EVENT_SCHEMAS, EventSchema, FieldSchema, event_schema};
pub use transport::{EventTransport, ReliabilityLevel};
pub use types::{DomainEvent, EventEnvelope};
pub use validation::{EventValidationError, ValidateEvent};
