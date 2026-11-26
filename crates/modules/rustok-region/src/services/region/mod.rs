mod commands;
mod queries;
pub(crate) mod read_port;
mod repository;

use sea_orm::DatabaseConnection;

pub struct RegionService {
    db: DatabaseConnection,
}

impl RegionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
