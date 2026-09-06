use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_index::{
    FieldName, FieldPath, FilterExpr, IndexQuery, IndexQueryExecutionError, IndexQueryPort,
    IndexQueryScope, IndexValue, Pagination, SharedIndexQueryRuntime,
};
use uuid::Uuid;

use crate::index::social_graph_relation_index_schema;
use crate::{
    MAX_SOCIAL_GRAPH_FOLLOW_TARGETS, SocialGraphFollowBatchRequest, SocialGraphFollowBatchResult,
    SocialGraphPairRequest, SocialGraphPrivacyReadPort, SocialRelationKind,
};

const SOURCE_USER_ID_FIELD: &str = "source_user_id";
const TARGET_USER_ID_FIELD: &str = "target_user_id";
const RELATION_KIND_FIELD: &str = "relation_kind";

#[derive(Clone)]
pub struct IndexSocialGraphPrivacyReadPort {
    port: Arc<dyn IndexQueryPort>,
}

impl IndexSocialGraphPrivacyReadPort {
    pub fn new(runtime: SharedIndexQueryRuntime) -> Self {
        Self {
            port: runtime.shared_port(),
        }
    }

    #[cfg(test)]
    fn from_port(port: Arc<dyn IndexQueryPort>) -> Self {
        Self { port }
    }

    async fn relation_exists(
        &self,
        tenant_id: Uuid,
        filter: FilterExpr,
    ) -> Result<bool, PortError> {
        let contract = relation_contract()?;
        let page = self
            .port
            .execute_query(IndexQuery {
                scope: IndexQueryScope {
                    tenant_id,
                    locale: None,
                },
                schema: contract.schema,
                fields: vec![contract.target],
                filter: Some(filter),
                order_by: Vec::new(),
                pagination: Pagination::Offset {
                    limit: 1,
                    offset: 0,
                },
                include_exact_count: false,
            })
            .await
            .map_err(map_index_error)?;
        Ok(!page.items.is_empty())
    }

    async fn followed_targets(
        &self,
        tenant_id: Uuid,
        source_user_id: Uuid,
        target_user_ids: BTreeSet<Uuid>,
    ) -> Result<Vec<Uuid>, PortError> {
        if target_user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let contract = relation_contract()?;
        let target_values = target_user_ids
            .iter()
            .copied()
            .map(IndexValue::Uuid)
            .collect::<Vec<_>>();
        let page = self
            .port
            .execute_query(IndexQuery {
                scope: IndexQueryScope {
                    tenant_id,
                    locale: None,
                },
                schema: contract.schema,
                fields: vec![contract.target.clone()],
                filter: Some(FilterExpr::And(vec![
                    FilterExpr::Eq(contract.source, IndexValue::Uuid(source_user_id)),
                    FilterExpr::In(contract.target.clone(), target_values),
                    FilterExpr::Eq(
                        contract.kind,
                        IndexValue::String(SocialRelationKind::Follow.as_str().to_owned()),
                    ),
                ])),
                order_by: Vec::new(),
                pagination: Pagination::Offset {
                    limit: u32::try_from(target_user_ids.len())
                        .map_err(|_| contract_invariant())?,
                    offset: 0,
                },
                include_exact_count: false,
            })
            .await
            .map_err(map_index_error)?;
        if page.has_more {
            return Err(contract_invariant());
        }

        let mut followed = BTreeSet::new();
        for item in page.items {
            let projected = item
                .fields
                .iter()
                .find(|projected| projected.path == contract.target)
                .ok_or_else(contract_invariant)?;
            let IndexValue::Uuid(target_user_id) = &projected.value else {
                return Err(contract_invariant());
            };
            if !target_user_ids.contains(target_user_id) {
                return Err(contract_invariant());
            }
            followed.insert(*target_user_id);
        }
        Ok(followed.into_iter().collect())
    }
}

#[async_trait]
impl SocialGraphPrivacyReadPort for IndexSocialGraphPrivacyReadPort {
    async fn blocks_between(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_pair(request.source_user_id, request.target_user_id)?;
        let tenant_id = parse_tenant_id(&context)?;
        let contract = relation_contract()?;
        self.relation_exists(
            tenant_id,
            FilterExpr::And(vec![
                FilterExpr::Eq(
                    contract.kind,
                    IndexValue::String(SocialRelationKind::Block.as_str().to_owned()),
                ),
                FilterExpr::Or(vec![
                    FilterExpr::And(vec![
                        FilterExpr::Eq(
                            contract.source.clone(),
                            IndexValue::Uuid(request.source_user_id),
                        ),
                        FilterExpr::Eq(
                            contract.target.clone(),
                            IndexValue::Uuid(request.target_user_id),
                        ),
                    ]),
                    FilterExpr::And(vec![
                        FilterExpr::Eq(contract.source, IndexValue::Uuid(request.target_user_id)),
                        FilterExpr::Eq(contract.target, IndexValue::Uuid(request.source_user_id)),
                    ]),
                ]),
            ]),
        )
        .await
    }

    async fn source_mutes_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_pair(request.source_user_id, request.target_user_id)?;
        let tenant_id = parse_tenant_id(&context)?;
        let contract = relation_contract()?;
        self.relation_exists(
            tenant_id,
            FilterExpr::And(vec![
                FilterExpr::Eq(contract.source, IndexValue::Uuid(request.source_user_id)),
                FilterExpr::Eq(contract.target, IndexValue::Uuid(request.target_user_id)),
                FilterExpr::Eq(
                    contract.kind,
                    IndexValue::String(SocialRelationKind::Mute.as_str().to_owned()),
                ),
            ]),
        )
        .await
    }

    async fn source_follows_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_source_actor(&context, request.source_user_id)?;
        validate_pair(request.source_user_id, request.target_user_id)?;
        let tenant_id = parse_tenant_id(&context)?;
        let contract = relation_contract()?;
        self.relation_exists(
            tenant_id,
            FilterExpr::And(vec![
                FilterExpr::Eq(contract.source, IndexValue::Uuid(request.source_user_id)),
                FilterExpr::Eq(contract.target, IndexValue::Uuid(request.target_user_id)),
                FilterExpr::Eq(
                    contract.kind,
                    IndexValue::String(SocialRelationKind::Follow.as_str().to_owned()),
                ),
            ]),
        )
        .await
    }

    async fn source_follows_targets(
        &self,
        context: PortContext,
        request: SocialGraphFollowBatchRequest,
    ) -> Result<SocialGraphFollowBatchResult, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_source_actor(&context, request.source_user_id)?;
        if request.target_user_ids.len() > MAX_SOCIAL_GRAPH_FOLLOW_TARGETS {
            return Err(PortError::validation(
                "social_graph.follow_batch_too_large",
                "social graph follow reads accept at most 100 target users",
            ));
        }
        let tenant_id = parse_tenant_id(&context)?;
        let target_user_ids = request.target_user_ids.into_iter().collect::<BTreeSet<_>>();
        for target_user_id in &target_user_ids {
            validate_pair(request.source_user_id, *target_user_id)?;
        }

        Ok(SocialGraphFollowBatchResult {
            followed_target_user_ids: self
                .followed_targets(tenant_id, request.source_user_id, target_user_ids)
                .await?,
        })
    }
}

struct RelationQueryContract {
    schema: rustok_index::SchemaRef,
    source: FieldPath,
    target: FieldPath,
    kind: FieldPath,
}

fn relation_contract() -> Result<RelationQueryContract, PortError> {
    let schema = social_graph_relation_index_schema()
        .map_err(|_| contract_invariant())?
        .reference;
    Ok(RelationQueryContract {
        schema,
        source: field_path(SOURCE_USER_ID_FIELD)?,
        target: field_path(TARGET_USER_ID_FIELD)?,
        kind: field_path(RELATION_KIND_FIELD)?,
    })
}

fn field_path(name: &str) -> Result<FieldPath, PortError> {
    FieldName::new(name)
        .map(FieldPath::new)
        .map_err(|_| contract_invariant())
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "social_graph.tenant_id_invalid",
            "social graph ports require a valid tenant identifier",
        )
    })
}

fn validate_pair(source_user_id: Uuid, target_user_id: Uuid) -> Result<(), PortError> {
    if source_user_id == target_user_id {
        return Err(PortError::validation(
            "social_graph.self_relation",
            "social graph relation cannot target the source user",
        ));
    }
    Ok(())
}

fn validate_source_actor(context: &PortContext, source_user_id: Uuid) -> Result<(), PortError> {
    if matches!(&context.actor.kind, PortActorKind::User)
        && Uuid::parse_str(&context.actor.id).ok() != Some(source_user_id)
    {
        return Err(PortError::forbidden(
            "social_graph.source_actor_mismatch",
            "user actors may mutate or read only relations they own",
        ));
    }
    Ok(())
}

fn map_index_error(error: IndexQueryExecutionError) -> PortError {
    match error {
        IndexQueryExecutionError::SchemaNotReady { .. }
        | IndexQueryExecutionError::Storage { .. } => PortError::unavailable(
            "social_graph.index_privacy_unavailable",
            "social graph Index privacy state is temporarily unavailable",
        ),
        IndexQueryExecutionError::Plan(_)
        | IndexQueryExecutionError::Build(_)
        | IndexQueryExecutionError::LocalizedBuild(_)
        | IndexQueryExecutionError::Decode(_)
        | IndexQueryExecutionError::LocalizedDecode(_)
        | IndexQueryExecutionError::UnsupportedBackend
        | IndexQueryExecutionError::MissingExactCountRow
        | IndexQueryExecutionError::InvalidRowColumn { .. }
        | IndexQueryExecutionError::ContractPreparation { .. } => contract_invariant(),
    }
}

fn contract_invariant() -> PortError {
    PortError::invariant_violation(
        "social_graph.index_privacy_contract_invalid",
        "social graph Index privacy contract is invalid",
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use async_trait::async_trait;
    use rustok_api::{PortActor, PortErrorKind};
    use rustok_index::{
        IndexProjectedValue, IndexQueryItem, IndexQueryPage, PersistedSchemaReadinessFailure,
    };

    use super::*;

    struct FakeIndexPort {
        query: Mutex<Option<IndexQuery>>,
        response: Mutex<Option<Result<IndexQueryPage, IndexQueryExecutionError>>>,
    }

    impl FakeIndexPort {
        fn new(response: Result<IndexQueryPage, IndexQueryExecutionError>) -> Arc<Self> {
            Arc::new(Self {
                query: Mutex::new(None),
                response: Mutex::new(Some(response)),
            })
        }

        fn query(&self) -> IndexQuery {
            self.query
                .lock()
                .expect("query lock")
                .clone()
                .expect("query should be captured")
        }
    }

    #[async_trait]
    impl IndexQueryPort for FakeIndexPort {
        async fn execute_query(
            &self,
            query: IndexQuery,
        ) -> Result<IndexQueryPage, IndexQueryExecutionError> {
            *self.query.lock().expect("query lock") = Some(query);
            self.response
                .lock()
                .expect("response lock")
                .take()
                .expect("one response should be configured")
        }
    }

    fn context(tenant_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::service("privacy-test"),
            "und",
            "privacy-test-correlation",
        )
        .with_deadline(Duration::from_secs(1))
    }

    fn empty_page() -> IndexQueryPage {
        IndexQueryPage {
            items: Vec::new(),
            exact_count: None,
            has_more: false,
            next_cursor: None,
        }
    }

    fn target_item(target_user_id: Uuid) -> IndexQueryItem {
        let contract = relation_contract().unwrap();
        IndexQueryItem {
            entity_id: Uuid::new_v4(),
            relations: Vec::new(),
            fields: vec![IndexProjectedValue {
                path: contract.target,
                value: IndexValue::Uuid(target_user_id),
            }],
            nested_relations: Vec::new(),
        }
    }

    #[tokio::test]
    async fn block_query_preserves_either_direction_semantics() {
        let fake = FakeIndexPort::new(Ok(empty_page()));
        let port = IndexSocialGraphPrivacyReadPort::from_port(fake.clone());
        let tenant_id = Uuid::from_u128(1);
        let left = Uuid::from_u128(2);
        let right = Uuid::from_u128(3);

        assert!(
            !port
                .blocks_between(
                    context(tenant_id),
                    SocialGraphPairRequest {
                        source_user_id: left,
                        target_user_id: right,
                    },
                )
                .await
                .unwrap()
        );

        let query = fake.query();
        let contract = relation_contract().unwrap();
        assert_eq!(query.scope.tenant_id, tenant_id);
        assert_eq!(query.schema, contract.schema);
        assert_eq!(query.fields, vec![contract.target.clone()]);
        assert_eq!(
            query.filter,
            Some(FilterExpr::And(vec![
                FilterExpr::Eq(contract.kind, IndexValue::String("block".to_owned()),),
                FilterExpr::Or(vec![
                    FilterExpr::And(vec![
                        FilterExpr::Eq(contract.source.clone(), IndexValue::Uuid(left)),
                        FilterExpr::Eq(contract.target.clone(), IndexValue::Uuid(right)),
                    ]),
                    FilterExpr::And(vec![
                        FilterExpr::Eq(contract.source, IndexValue::Uuid(right)),
                        FilterExpr::Eq(contract.target, IndexValue::Uuid(left)),
                    ]),
                ]),
            ]))
        );
    }

    #[tokio::test]
    async fn follow_batch_deduplicates_and_sorts_projected_targets() {
        let first = Uuid::from_u128(11);
        let second = Uuid::from_u128(12);
        let fake = FakeIndexPort::new(Ok(IndexQueryPage {
            items: vec![target_item(second), target_item(first)],
            exact_count: None,
            has_more: false,
            next_cursor: None,
        }));
        let port = IndexSocialGraphPrivacyReadPort::from_port(fake.clone());
        let source = Uuid::from_u128(10);

        let result = port
            .source_follows_targets(
                context(Uuid::from_u128(1)),
                SocialGraphFollowBatchRequest {
                    source_user_id: source,
                    target_user_ids: vec![second, first, second],
                },
            )
            .await
            .unwrap();

        assert_eq!(result.followed_target_user_ids, vec![first, second]);
        assert_eq!(
            fake.query().pagination,
            Pagination::Offset {
                limit: 2,
                offset: 0
            }
        );
    }

    #[tokio::test]
    async fn missing_tenant_schema_is_retryable_and_does_not_authorize() {
        let contract = relation_contract().unwrap();
        let fake = FakeIndexPort::new(Err(IndexQueryExecutionError::SchemaNotReady {
            reference: contract.schema,
            reason: PersistedSchemaReadinessFailure::Missing,
        }));
        let port = IndexSocialGraphPrivacyReadPort::from_port(fake);

        let error = port
            .source_mutes_target(
                context(Uuid::from_u128(1)),
                SocialGraphPairRequest {
                    source_user_id: Uuid::from_u128(2),
                    target_user_id: Uuid::from_u128(3),
                },
            )
            .await
            .expect_err("missing schema must fail closed");

        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert_eq!(error.code, "social_graph.index_privacy_unavailable");
        assert!(error.retryable);
    }

    #[tokio::test]
    async fn user_follow_reads_preserve_source_actor_authorization() {
        let fake = FakeIndexPort::new(Ok(empty_page()));
        let port = IndexSocialGraphPrivacyReadPort::from_port(fake);
        let tenant_id = Uuid::from_u128(1);
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(Uuid::from_u128(9).to_string()),
            "und",
            "privacy-test-correlation",
        )
        .with_deadline(Duration::from_secs(1));

        let error = port
            .source_follows_target(
                context,
                SocialGraphPairRequest {
                    source_user_id: Uuid::from_u128(2),
                    target_user_id: Uuid::from_u128(3),
                },
            )
            .await
            .expect_err("user actor mismatch must remain forbidden");

        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(error.code, "social_graph.source_actor_mismatch");
    }
}
