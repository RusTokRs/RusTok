mod category;
mod helpers;
mod policy;
pub mod reconciliation;
mod topic;

use sea_orm::DatabaseConnection;

use crate::audience::SharedForumAudienceFactsPort;

pub struct SubscriptionService {
    pub(super) db: DatabaseConnection,
    pub(super) audience_facts: Option<SharedForumAudienceFactsPort>,
}

impl SubscriptionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            audience_facts: None,
        }
    }

    /// Supplies the host audience facts port. Topic subscription writes need it when a
    /// required trust, channel, or group layer cannot be decided from the caller alone.
    pub fn with_audience_facts(mut self, facts_port: SharedForumAudienceFactsPort) -> Self {
        self.audience_facts = Some(facts_port);
        self
    }
}
