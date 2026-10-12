//! Docs module persistence entities (internal to crate).

pub mod entity_link;
pub mod page;
pub mod snapshot;
pub mod update;

pub use entity_link::Entity as EntityLink;
pub use page::Entity as Page;
pub use snapshot::Entity as Snapshot;
pub use update::Entity as Update;
