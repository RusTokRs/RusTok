//! Shared local persistence for the platform Outbox. Event delivery belongs to the relay.

pub mod consumers;
pub mod entity;
pub mod idempotency;
pub mod migration;
pub mod transactional;
pub mod transport;

pub use consumers::OutboxConsumer;
pub use entity::{Entity as SysEvents, Model as SysEvent};
pub use migration::SysEventsMigration;
pub use transactional::{OutboxRootEventWriter, TransactionalEventBus};
pub use transport::{ContractEventWriteOnceError, OutboxTransport};

mod clock;
pub use clock::{database_now, observe_database_clock};
