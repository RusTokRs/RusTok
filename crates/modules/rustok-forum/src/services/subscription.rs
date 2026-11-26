pub(crate) mod auto_subscribe;
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
    pub(super) taxonomy_port: std::sync::Arc<dyn crate::ForumTaxonomyPort>,
}

impl SubscriptionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            audience_facts: None,
            taxonomy_port: std::sync::Arc::new(crate::NoopForumTaxonomyPort),
        }
    }

    /// Uses the selected hierarchy provider for topic subscription audience checks.
    pub fn with_taxonomy_port(
        mut self,
        port: std::sync::Arc<dyn crate::ForumTaxonomyPort>,
    ) -> Self {
        self.taxonomy_port = port;
        self
    }

    /// Supplies the host audience facts port. Topic subscription writes need it when a
    /// required trust, channel, or group layer cannot be decided from the caller alone.
    pub fn with_audience_facts(mut self, facts_port: SharedForumAudienceFactsPort) -> Self {
        self.audience_facts = Some(facts_port);
        self
    }
}
