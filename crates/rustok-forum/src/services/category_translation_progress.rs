use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OpaqueCursor, ReadTranslationResourceRequest,
    TranslationApplicationReceipt, TranslationPatchRequest, TranslationPatchValidation,
    TranslationResourcePage, TranslationResourceSnapshot, TranslationTargetCapability,
    TranslationTargetChangePage, TranslationTargetChangesRequest, TranslationTargetProgressFacts,
    TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, provider_support::contract_validation_error,
    validate_translation_read_context,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    sea_query::{Expr, Query, SelectStatement},
};
use uuid::Uuid;

use crate::entities::{
    forum_category::{Column as CategoryColumn, Entity as CategoryEntity},
    forum_category_lifecycle::{Column as LifecycleColumn, Entity as LifecycleEntity},
    forum_category_translation::{Column as TranslationColumn, Entity as TranslationEntity},
};

const TRANSLATION_RESOURCE_KIND: &str = "category";
const REQUIRED_FIELD_COUNT: u64 = 1;
const OPTIONAL_FIELD_COUNT: u64 = 1;
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

mod change_row {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "forum_translation_changes")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub tenant_id: Uuid,
        pub resource_kind: String,
        pub resource_id: Uuid,
        pub resource_revision: String,
        pub operation: String,
        pub lifecycle: String,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[derive(Clone)]
pub struct ForumCategoryTranslationTargetProvider {
    inner: super::category_translation_target::ForumCategoryTranslationTargetProvider,
    db: DatabaseConnection,
}

impl ForumCategoryTranslationTargetProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: super::category_translation_target::ForumCategoryTranslationTargetProvider::new(
                db.clone(),
            ),
            db,
        }
    }

    async fn latest_change_cursor(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<OpaqueCursor>, PortError> {
        change_row::Entity::find()
            .filter(change_row::Column::TenantId.eq(tenant_id))
            .filter(change_row::Column::ResourceKind.eq(TRANSLATION_RESOURCE_KIND))
            .order_by_desc(change_row::Column::Id)
            .one(&self.db)
            .await
            .map_err(forum_database_error_to_port_error)?
            .map(|change| {
                OpaqueCursor::new(change.id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "forum.translation_change_cursor_invalid",
                        error.to_string(),
                    )
                })
            })
            .transpose()
    }

    async fn progress_facts(
        &self,
        tenant_id: Uuid,
        request: &TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        let categories = CategoryEntity::find()
            .inner_join(TranslationEntity)
            .filter(CategoryColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::Locale.eq(request.source_locale.as_str()))
            .filter(
                Expr::col((CategoryEntity, CategoryColumn::Id))
                    .not_in_subquery(archived_category_ids_subquery(tenant_id)),
            )
            .order_by_asc(CategoryColumn::Id)
            .all(&self.db)
            .await
            .map_err(forum_database_error_to_port_error)?;
        let category_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<Vec<_>>();
        let targets = if category_ids.is_empty() {
            Vec::new()
        } else {
            TranslationEntity::find()
                .filter(TranslationColumn::TenantId.eq(tenant_id))
                .filter(TranslationColumn::CategoryId.is_in(category_ids))
                .filter(TranslationColumn::Locale.eq(request.target_locale.as_str()))
                .all(&self.db)
                .await
                .map_err(forum_database_error_to_port_error)?
        };
        let targets = targets
            .into_iter()
            .map(|translation| (translation.category_id, translation))
            .collect::<BTreeMap<_, _>>();
        let resources = u64::try_from(categories.len()).map_err(|_| {
            PortError::invariant_violation(
                "forum.translation_progress_overflow",
                "Forum category resource count exceeds the progress contract",
            )
        })?;
        let required_units = resources.checked_mul(REQUIRED_FIELD_COUNT).ok_or_else(|| {
            PortError::invariant_violation(
                "forum.translation_progress_overflow",
                "Forum category required progress count overflow",
            )
        })?;
        let optional_units = resources.checked_mul(OPTIONAL_FIELD_COUNT).ok_or_else(|| {
            PortError::invariant_violation(
                "forum.translation_progress_overflow",
                "Forum category optional progress count overflow",
            )
        })?;

        let mut exact_required_units = 0_u64;
        let mut exact_optional_units = 0_u64;
        let mut complete_resources = 0_u64;
        for category in categories {
            let Some(target) = targets.get(&category.id) else {
                continue;
            };
            let has_name = !target.name.trim().is_empty();
            if has_name {
                exact_required_units = exact_required_units.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "forum.translation_progress_overflow",
                        "Forum category exact required progress count overflow",
                    )
                })?;
                complete_resources = complete_resources.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "forum.translation_progress_overflow",
                        "Forum category complete resource count overflow",
                    )
                })?;
            }
            if target
                .description
                .as_deref()
                .is_some_and(|description| !description.trim().is_empty())
            {
                exact_optional_units = exact_optional_units.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "forum.translation_progress_overflow",
                        "Forum category exact optional progress count overflow",
                    )
                })?;
            }
        }

        Ok(TranslationTargetProgressFacts {
            required_units,
            exact_required_units,
            optional_units,
            exact_optional_units,
            resources,
            complete_resources,
            owner_change_cursor: None,
        })
    }
}

#[async_trait]
impl TranslationTargetProvider for ForumCategoryTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        let mut descriptor = self.inner.descriptor();
        descriptor
            .capabilities
            .insert(TranslationTargetCapability::AggregateProgress);
        descriptor
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        self.inner.list_resources(context, request).await
    }

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        self.inner.read_resource(context, request).await
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        self.inner.validate_patch(context, request).await
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        self.inner.apply_patch(context, request).await
    }

    async fn read_progress(
        &self,
        context: PortContext,
        request: TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;

        for _ in 0..PROGRESS_STABILITY_ATTEMPTS {
            let cursor_before = self.latest_change_cursor(tenant_id).await?;
            let mut facts = self.progress_facts(tenant_id, &request).await?;
            let cursor_after = self.latest_change_cursor(tenant_id).await?;
            if cursor_before == cursor_after {
                facts.owner_change_cursor = cursor_after;
                facts.validate().map_err(|error| {
                    PortError::invariant_violation(
                        "forum.translation_progress_invalid",
                        error.to_string(),
                    )
                })?;
                return Ok(facts);
            }
        }

        Err(PortError::unavailable(
            "forum.translation_progress_unstable",
            "Forum category translation progress changed while it was being aggregated",
        ))
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        self.inner.read_changes(context, request).await
    }
}

fn archived_category_ids_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(LifecycleColumn::CategoryId)
        .from(LifecycleEntity)
        .and_where(Expr::col((LifecycleEntity, LifecycleColumn::TenantId)).eq(tenant_id))
        .to_owned()
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "forum.invalid_tenant_id",
            "Forum translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::ForumCategories, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "forum.translation_permission_denied",
            format!("forum_categories:{action} permission is required"),
        ));
    }
    Ok(security)
}

fn forum_database_error_to_port_error(_error: sea_orm::DbErr) -> PortError {
    PortError::unavailable(
        "forum.translation_owner_unavailable",
        "Forum translation storage is unavailable",
    )
}
