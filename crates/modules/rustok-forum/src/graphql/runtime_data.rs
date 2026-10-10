use std::sync::Arc;
use uuid::Uuid;

use rustok_api::{
    SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
    graphql::GraphqlRuntimeInputs,
};
use rustok_media::MediaAssetReadPort;
use rustok_notifications_api::{
    NotificationInboxReconciliationInspectPort, NotificationInboxReconciliationInspectPortFactory,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::{
    ForumCategoryAudienceReadService, ForumReadModelService, ForumReplyAudienceReadService,
    ForumSettingsProviders, ForumStorefrontReadStateService, ForumTopicAudienceListService,
    ForumTopicAudienceReadService, ForumVisibilityScopedReadStateService, ModerationService,
    ReplyService, SharedForumAudienceFactsPort, SubscriptionService, TopicService, VoteService,
};
use crate::moderation_report::SharedForumModerationReportPort;
use crate::services::moderation_report::ForumModerationReportService;

/// Manifest-attached Forum GraphQL runtime capabilities.
///
/// The optional facts port is published by the host runtime extension registry.
/// Its absence is preserved so locally decidable create, moderation, and read
/// policies continue to work while trust, Channel, or Groups facts fail closed
/// in owners.
#[derive(Clone, Default)]
pub struct ForumGraphqlRuntimeData {
    audience_facts: Option<SharedForumAudienceFactsPort>,
    moderation_report: Option<SharedForumModerationReportPort>,
    settings_providers: ForumSettingsProviders,
    attachment_hold_media: Option<Arc<dyn MediaAssetReadPort>>,
    notification_reconciliation: Option<Arc<dyn NotificationInboxReconciliationInspectPort>>,
}

pub fn attach_schema_data(
    inputs: &GraphqlRuntimeInputs,
) -> Result<ForumGraphqlRuntimeData, String> {
    let settings_providers = match (
        inputs.shared_get::<SharedStaticModuleSettingsReader>(),
        inputs.shared_get::<SharedStaticModuleSettingsTransactionReader>(),
    ) {
        (Some(reader), Some(transactional_reader)) => {
            ForumSettingsProviders::default().with_static_readers(reader, transactional_reader)
        }
        _ => ForumSettingsProviders::default(),
    };

    let attachment_hold_media = inputs.shared_get::<Arc<dyn MediaAssetReadPort>>();
    let notification_reconciliation = inputs
        .shared_get::<Arc<dyn NotificationInboxReconciliationInspectPortFactory>>()
        .and_then(|factory| factory.build(inputs.host()).ok());

    Ok(ForumGraphqlRuntimeData {
        audience_facts: inputs.shared_get::<SharedForumAudienceFactsPort>(),
        moderation_report: inputs.shared_get::<SharedForumModerationReportPort>(),
        settings_providers,
        attachment_hold_media,
        notification_reconciliation,
    })
}

impl ForumGraphqlRuntimeData {
    pub(crate) fn attachment_hold_reconciliation_media(
        &self,
    ) -> Option<Arc<dyn MediaAssetReadPort>> {
        self.attachment_hold_media.clone()
    }

    pub(crate) fn notification_reconciliation_port(
        &self,
    ) -> Option<Arc<dyn NotificationInboxReconciliationInspectPort>> {
        self.notification_reconciliation.clone()
    }

    pub(crate) fn read_model_service(&self, db: DatabaseConnection) -> ForumReadModelService {
        ForumReadModelService::new(db).with_settings_providers(self.settings_providers.clone())
    }

    pub(crate) fn category_audience_read_service(
        &self,
        db: DatabaseConnection,
    ) -> ForumCategoryAudienceReadService {
        match self.audience_facts.clone() {
            Some(facts) => ForumCategoryAudienceReadService::with_audience_facts(db, facts),
            None => ForumCategoryAudienceReadService::new(db),
        }
    }

    pub(crate) fn topic_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> TopicService {
        let service = match self.audience_facts.clone() {
            Some(facts) => TopicService::with_audience_facts(db, event_bus, facts),
            None => TopicService::new(db, event_bus),
        };
        service.with_settings_providers(self.settings_providers.clone())
    }

    pub(crate) fn moderation_report_service(
        &self,
        db: DatabaseConnection,
    ) -> ForumModerationReportService {
        ForumModerationReportService::new(
            db,
            self.audience_facts.clone(),
            self.moderation_report.clone(),
        )
    }

    pub(crate) fn vote_service(&self, db: DatabaseConnection) -> VoteService {
        let service =
            VoteService::new(db).with_settings_providers(self.settings_providers.clone());
        match self.audience_facts.clone() {
            Some(facts) => service.with_audience_facts(facts),
            None => service,
        }
    }

    pub(crate) fn subscription_service(&self, db: DatabaseConnection) -> SubscriptionService {
        let service = SubscriptionService::new(db);
        match self.audience_facts.clone() {
            Some(facts) => service.with_audience_facts(facts),
            None => service,
        }
    }

    pub(crate) fn reply_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> ReplyService {
        let service = match self.audience_facts.clone() {
            Some(facts) => ReplyService::with_audience_facts(db, event_bus, facts),
            None => ReplyService::new(db, event_bus),
        };
        service.with_settings_providers(self.settings_providers.clone())
    }

    pub(crate) fn reply_audience_read_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> ForumReplyAudienceReadService {
        match self.audience_facts.clone() {
            Some(facts) => ForumReplyAudienceReadService::with_audience_facts(db, event_bus, facts),
            None => ForumReplyAudienceReadService::new(db, event_bus),
        }
    }

    pub(crate) fn moderation_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> ModerationService {
        match self.audience_facts.clone() {
            Some(facts) => ModerationService::with_audience_facts(db, event_bus, facts),
            None => ModerationService::new(db, event_bus),
        }
    }

    pub(crate) fn topic_audience_read_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> ForumTopicAudienceReadService {
        match self.audience_facts.clone() {
            Some(facts) => ForumTopicAudienceReadService::with_audience_facts(db, event_bus, facts),
            None => ForumTopicAudienceReadService::new(db, event_bus),
        }
    }

    /// Tenant default page size for topic lists when a request omits `perPage`.
    pub(crate) async fn default_topics_per_page(
        &self,
        tenant_id: Uuid,
    ) -> crate::error::ForumResult<u64> {
        self.settings_providers.default_topics_per_page(tenant_id).await
    }

    /// Tenant default page size for reply lists when a request omits `perPage`.
    pub(crate) async fn default_replies_per_page(
        &self,
        tenant_id: Uuid,
    ) -> crate::error::ForumResult<u64> {
        self.settings_providers.default_replies_per_page(tenant_id).await
    }

    pub(crate) fn topic_audience_list_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> ForumTopicAudienceListService {
        let service = match self.audience_facts.clone() {
            Some(facts) => ForumTopicAudienceListService::with_audience_facts(db, event_bus, facts),
            None => ForumTopicAudienceListService::new(db, event_bus),
        };
        service.with_settings_providers(self.settings_providers.clone())
    }

    pub(crate) fn storefront_read_state_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> ForumStorefrontReadStateService {
        let service = match self.audience_facts.clone() {
            Some(facts) => {
                ForumStorefrontReadStateService::with_audience_facts(db, event_bus, facts)
            }
            None => ForumStorefrontReadStateService::new(db, event_bus),
        };
        service.with_settings_providers(self.settings_providers.clone())
    }

    pub(crate) fn visibility_scoped_read_state_service(
        &self,
        db: DatabaseConnection,
    ) -> ForumVisibilityScopedReadStateService {
        match self.audience_facts.clone() {
            Some(facts) => ForumVisibilityScopedReadStateService::with_audience_facts(db, facts),
            None => ForumVisibilityScopedReadStateService::new(db),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_api::{HostRuntimeContext, PortContext, PortError};
    use sea_orm::Database;

    use crate::{ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest};

    use super::*;

    struct StaticFactsPort;

    #[async_trait]
    impl ForumAudienceFactsPort for StaticFactsPort {
        async fn resolve_forum_audience_facts(
            &self,
            _context: PortContext,
            request: ForumAudienceFactsRequest,
        ) -> Result<ForumAudienceFacts, PortError> {
            Ok(ForumAudienceFacts {
                tenant_id: request.tenant_id,
                user_id: request.user_id,
                ..ForumAudienceFacts::default()
            })
        }
    }

    #[tokio::test]
    async fn schema_factory_consumes_host_published_audience_facts_without_db_discovery() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("GraphQL runtime test database should connect");
        let facts: SharedForumAudienceFactsPort = Arc::new(StaticFactsPort);
        let inputs =
            GraphqlRuntimeInputs::new(HostRuntimeContext::new(db).with_shared_value(facts.clone()));

        let runtime =
            attach_schema_data(&inputs).expect("Forum GraphQL runtime should materialize");
        assert!(runtime.audience_facts.is_some());
    }

    #[tokio::test]
    async fn schema_factory_preserves_optional_provider_absence() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("GraphQL runtime test database should connect");
        let inputs = GraphqlRuntimeInputs::new(HostRuntimeContext::new(db));

        let runtime =
            attach_schema_data(&inputs).expect("Forum GraphQL runtime should materialize");
        assert!(runtime.audience_facts.is_none());
        assert!(runtime.attachment_hold_media.is_none());
        assert!(runtime.notification_reconciliation.is_none());
    }

    #[tokio::test]
    async fn schema_factory_consumes_host_published_media_reader() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let storage = rustok_storage::StorageRuntime::local(&rustok_storage::LocalStorageConfig {
            base_dir: std::env::temp_dir()
                .join(format!(
                    "rustok-forum-media-runtime-{}",
                    uuid::Uuid::new_v4()
                ))
                .display()
                .to_string(),
            base_url: String::new(),
            fsync: false,
        })
        .expect("local storage runtime should initialize");
        let media: Arc<dyn MediaAssetReadPort> =
            Arc::new(rustok_media::MediaService::new(db.clone(), storage));
        let host = HostRuntimeContext::new(db).with_shared_value(media);
        let inputs = GraphqlRuntimeInputs::new(host);

        let runtime =
            attach_schema_data(&inputs).expect("Forum GraphQL runtime should materialize");
        assert!(runtime.attachment_hold_media.is_some());
    }
}
