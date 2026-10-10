//! SeaORM backend for revision storage.

mod backend;
mod entities;

pub use backend::SeaOrmBackend;
pub use entities::Entity as RevisionEntity;
