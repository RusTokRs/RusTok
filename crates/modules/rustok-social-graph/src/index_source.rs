use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_events::SocialGraphRelationEvent;
use rustok_index::{
    IndexMutation, IndexMutationEventError, IndexSource, IndexSourceCursor, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    PostgresIndexSourceFactory, PostgresIndexSourceFactoryError, SchemaRef,
    derive_index_source_event_id, register_index_mutation_event, register_index_source,
    register_postgres_index_source_factory,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::relation;
use crate::index::{
    SocialGraphIndexError, social_graph_relation_index_mutation, social_graph_relation_index_schema,
};

pub const SOCIAL_GRAPH_RELATION_INDEX_SOURCE: &str = "social_graph.relation.state_changed.v1";
pub const SOCIAL_GRAPH_RELATION_INDEX_EVENT_DOMAIN: &str = "social_graph.relation.state_changed.v1";
pub const SOCIAL_GRAPH_RELATION_INDEX_SOURCE_FACTORY: &str = "social-graph-relation-index-source";

const SOCIAL_GRAPH_INDEX_OWNER: &str = "social_graph";
const SOCIAL_GRAPH_RELATION_REPLAY_EVENT_DOMAIN: &str = "rustok-social-graph.relation-replay-v1";

#[derive(Debug, Error)]
pub enum SocialGraphIndexSourceRegistrationError {
    #[error("Social Graph Index schema construction failed")]
    Schema(#[source] SocialGraphIndexError),
    #[error("Social Graph PostgreSQL Index source factory registration failed")]
    Factory(#[source] PostgresIndexSourceFactoryError),
    #[error("Social Graph Index mutation event route registration failed")]
    EventRoute(#[source] IndexMutationEventError),
}

/// Registers the database-aware replay source constructor and the exact incremental event route.
///
/// The source and live consumer intentionally share one inbox source identity. Full replay uses a
/// separate deterministic event-id domain, while live deliveries retain the owner event UUID.
/// Both paths therefore converge through the same schema and monotonic source-version contract.
pub fn register_social_graph_index_source_contracts(
    extensions: &mut ModuleRuntimeExtensions,
) -> Result<(), SocialGraphIndexSourceRegistrationError> {
    let schema = relation_schema_ref().map_err(SocialGraphIndexSourceRegistrationError::Schema)?;

    register_postgres_index_source_factory(
        extensions,
        SOCIAL_GRAPH_INDEX_OWNER,
        SOCIAL_GRAPH_RELATION_INDEX_SOURCE_FACTORY,
        SocialGraphRelationPostgresIndexSourceFactory,
    )
    .map_err(SocialGraphIndexSourceRegistrationError::Factory)?;

    register_index_mutation_event(
        extensions,
        SOCIAL_GRAPH_INDEX_OWNER,
        SOCIAL_GRAPH_RELATION_INDEX_EVENT_DOMAIN,
        SOCIAL_GRAPH_RELATION_INDEX_SOURCE,
        schema,
    )
    .map_err(SocialGraphIndexSourceRegistrationError::EventRoute)
}

#[derive(Clone, Copy, Debug)]
struct SocialGraphRelationPostgresIndexSourceFactory;

impl PostgresIndexSourceFactory for SocialGraphRelationPostgresIndexSourceFactory {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), String> {
        let schema = relation_schema_ref().map_err(|error| error.to_string())?;
        register_index_source(
            extensions,
            SOCIAL_GRAPH_INDEX_OWNER,
            SOCIAL_GRAPH_RELATION_INDEX_SOURCE,
            [schema],
            SocialGraphRelationPostgresIndexSource { db },
        )
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug)]
struct SocialGraphRelationPostgresIndexSource {
    db: DatabaseConnection,
}

impl SocialGraphRelationPostgresIndexSource {
    fn validate_schema(&self, schema: &SchemaRef) -> Result<(), IndexSourceFailure> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(permanent("social_graph_index_postgres_required"));
        }
        let expected =
            relation_schema_ref().map_err(|_| permanent("social_graph_index_schema_invalid"))?;
        if schema != &expected {
            return Err(permanent("social_graph_index_schema_mismatch"));
        }
        Ok(())
    }

    async fn scan_models(
        &self,
        request: &IndexSourceScanRequest,
        cursor: Option<&RelationCursor>,
    ) -> Result<Vec<relation::Model>, IndexSourceFailure> {
        let fetch_limit = u64::try_from(request.limit() + 1)
            .expect("Index source page limit is bounded below u64::MAX");
        let mut query = relation::Entity::find()
            .filter(relation::Column::TenantId.eq(request.tenant_id()))
            .order_by_asc(relation::Column::Id)
            .limit(fetch_limit);
        if let Some(cursor) = cursor {
            query = query.filter(relation::Column::Id.gt(cursor.relation_id));
        }
        query
            .all(&self.db)
            .await
            .map_err(|_| retryable("social_graph_index_storage_unavailable"))
    }

    async fn load_models(
        &self,
        request: &IndexSourceLoadRequest,
    ) -> Result<Vec<relation::Model>, IndexSourceFailure> {
        if request.keys().iter().any(|key| key.locale.is_some()) {
            return Err(permanent("social_graph_index_locale_forbidden"));
        }
        let relation_ids = request
            .keys()
            .iter()
            .map(|key| key.entity_id)
            .collect::<Vec<_>>();
        relation::Entity::find()
            .filter(relation::Column::TenantId.eq(request.tenant_id()))
            .filter(relation::Column::Id.is_in(relation_ids))
            .order_by_asc(relation::Column::Id)
            .all(&self.db)
            .await
            .map_err(|_| retryable("social_graph_index_storage_unavailable"))
    }
}

#[async_trait]
impl IndexSource for SocialGraphRelationPostgresIndexSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.validate_schema(request.schema())?;
        let cursor = request.cursor().map(RelationCursor::decode).transpose()?;
        let models = self.scan_models(&request, cursor.as_ref()).await?;
        let has_more = models.len() > request.limit();
        let selected = models.into_iter().take(request.limit()).collect::<Vec<_>>();
        let next_cursor = if has_more {
            let relation_id = selected
                .last()
                .ok_or_else(|| permanent("social_graph_index_page_invalid"))?
                .id;
            Some(RelationCursor { relation_id }.encode()?)
        } else {
            None
        };
        let mutations = selected
            .into_iter()
            .map(|model| model_into_mutation(model, request.tenant_id()))
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| permanent("social_graph_index_page_invalid"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        self.validate_schema(request.schema())?;
        let mutations = self
            .load_models(&request)
            .await?
            .into_iter()
            .map(|model| model_into_mutation(model, request.tenant_id()))
            .collect::<Result<Vec<_>, _>>()?;
        IndexSourceLoadBatch::new(&request, mutations)
            .map_err(|_| permanent("social_graph_index_batch_invalid"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationCursor {
    relation_id: Uuid,
}

impl RelationCursor {
    fn decode(cursor: &IndexSourceCursor) -> Result<Self, IndexSourceFailure> {
        let decoded: Self = serde_json::from_value(cursor.value().clone())
            .map_err(|_| permanent("social_graph_index_cursor_invalid"))?;
        if decoded.relation_id.is_nil() {
            return Err(permanent("social_graph_index_cursor_invalid"));
        }
        Ok(decoded)
    }

    fn encode(&self) -> Result<IndexSourceCursor, IndexSourceFailure> {
        if self.relation_id.is_nil() {
            return Err(permanent("social_graph_index_cursor_invalid"));
        }
        let value = serde_json::to_value(self)
            .map_err(|_| permanent("social_graph_index_cursor_invalid"))?;
        IndexSourceCursor::new(value).map_err(|_| permanent("social_graph_index_cursor_invalid"))
    }
}

fn model_into_mutation(
    model: relation::Model,
    expected_tenant: Uuid,
) -> Result<IndexMutation, IndexSourceFailure> {
    if model.tenant_id != expected_tenant
        || model.tenant_id.is_nil()
        || model.id.is_nil()
        || model.source_user_id.is_nil()
        || model.target_user_id.is_nil()
        || model.source_user_id == model.target_user_id
    {
        return Err(permanent("social_graph_index_record_invalid"));
    }
    let source_version = u64::try_from(model.revision)
        .map_err(|_| permanent("social_graph_index_source_version_invalid"))?;
    if source_version == 0 {
        return Err(permanent("social_graph_index_source_version_invalid"));
    }
    let event_id = derive_index_source_event_id(
        SOCIAL_GRAPH_RELATION_REPLAY_EVENT_DOMAIN,
        model.tenant_id,
        model.id,
        None,
        source_version,
    )
    .map_err(|_| permanent("social_graph_index_event_identity_invalid"))?;
    let event = SocialGraphRelationEvent::RelationStateChanged {
        relation_id: model.id,
        source_user_id: model.source_user_id,
        target_user_id: model.target_user_id,
        relation_kind: model.relation_kind.as_str().to_owned(),
        active: model.active,
        revision: model.revision,
    };
    social_graph_relation_index_mutation(model.tenant_id, event_id, event)
        .map_err(|_| permanent("social_graph_index_record_invalid"))
}

fn relation_schema_ref() -> Result<SchemaRef, SocialGraphIndexError> {
    social_graph_relation_index_schema().map(|schema| schema.reference)
}

fn retryable(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::retryable(code).expect("static retryable source failure code must be valid")
}

fn permanent(code: &'static str) -> IndexSourceFailure {
    IndexSourceFailure::permanent(code).expect("static permanent source failure code must be valid")
}

#[cfg(test)]
mod tests {
    use rustok_index::{IndexMutationEventCatalog, PostgresIndexSourceFactoryCatalog};

    use super::*;

    #[test]
    fn source_contracts_register_one_factory_and_exact_event_route() {
        let mut extensions = ModuleRuntimeExtensions::default();
        register_social_graph_index_source_contracts(&mut extensions).unwrap();

        let factories = extensions
            .get::<PostgresIndexSourceFactoryCatalog>()
            .expect("source factory catalog");
        assert_eq!(factories.len(), 1);
        let factory = factories.iter().next().unwrap();
        assert_eq!(factory.owner_module(), SOCIAL_GRAPH_INDEX_OWNER);
        assert_eq!(
            factory.factory_name(),
            SOCIAL_GRAPH_RELATION_INDEX_SOURCE_FACTORY
        );

        let routes = extensions
            .get::<IndexMutationEventCatalog>()
            .expect("event route catalog");
        let route = routes
            .get(SOCIAL_GRAPH_RELATION_INDEX_EVENT_DOMAIN)
            .expect("Social Graph event route");
        assert_eq!(route.owner_module(), SOCIAL_GRAPH_INDEX_OWNER);
        assert_eq!(route.source_name(), SOCIAL_GRAPH_RELATION_INDEX_SOURCE);
        assert_eq!(route.schema(), &relation_schema_ref().unwrap());
    }

    #[test]
    fn replay_cursor_is_bounded_and_rejects_nil_identity() {
        let relation_id = Uuid::from_u128(42);
        let encoded = RelationCursor { relation_id }.encode().unwrap();
        assert_eq!(
            RelationCursor::decode(&encoded).unwrap().relation_id,
            relation_id
        );
        assert!(
            RelationCursor {
                relation_id: Uuid::nil(),
            }
            .encode()
            .is_err()
        );
    }

    #[cfg(feature = "index-consumer")]
    #[test]
    fn replay_and_live_consumer_share_one_source_identity() {
        assert_eq!(
            SOCIAL_GRAPH_RELATION_INDEX_SOURCE,
            crate::index_consumer::SOCIAL_GRAPH_INDEX_SOURCE
        );
    }
}
