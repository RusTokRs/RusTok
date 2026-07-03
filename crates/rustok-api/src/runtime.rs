use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct HostRuntimeContext {
    db: DatabaseConnection,
}

impl HostRuntimeContext {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }
}
