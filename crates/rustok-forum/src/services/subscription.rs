mod category;
mod helpers;
mod policy;
mod topic;

use sea_orm::DatabaseConnection;

pub struct SubscriptionService {
    pub(super) db: DatabaseConnection,
}

impl SubscriptionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
