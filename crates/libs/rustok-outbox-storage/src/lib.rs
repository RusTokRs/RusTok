//! Shared local persistence for the platform Outbox. Event delivery belongs to the relay.

pub mod entity;
pub mod idempotency;
pub mod migration;
pub mod transactional;
pub mod transport;

pub use entity::{Entity as SysEvents, Model as SysEvent};
pub use migration::SysEventsMigration;
pub use transactional::{OutboxRootEventWriter, TransactionalEventBus};
pub use transport::{ContractEventWriteOnceError, OutboxTransport};
