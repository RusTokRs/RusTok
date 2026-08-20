use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::idempotency::{self, Admission};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetProvider, TranslationTargetProviderDescriptor, TranslationValueProfile,
    provider_support::{
        contract_validation_error, decode_application_receipt, field_hash, merged_patch_values,
        normalize_optional_target_value, read_request_from_patch, required_target_value,
        validate_patch_against_snapshot, validation_to_port_error,
    },
    validate_translation_apply_context, validate_translation_read_context,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait,
    sea_query::{Expr, Query, SelectStatement},
};
use uuid::Uuid;

use crate::{
    ForumError,
    entities::{
        forum_category::{
            Column as CategoryColumn, Entity as CategoryEntity, Model as CategoryModel,
        },
        forum_category_lifecycle::{Column as LifecycleColumn, Entity as LifecycleEntity},
        forum_category_translation::{
            Column as TranslationColumn, Entity as TranslationEntity, Model as TranslationModel,
        },
    },
};

use super::{
    category_route::ForumCategoryRouteService,
    projection_invalidation::publish_forum_projection_scope_direct_in_tx,
};

const TRANSLATION_OWNER_SLUG: &str = "forum";
const TRANSLATION_RESOURCE_KIND: &str = "category";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_patch";

#[derive(Clone)]
/// Forum-owned adapter for exact category copy.
///
/// Translation never receives direct ownership of Forum route identity. The
/// adapter exposes only plain-text copy (`name` and `description`), preserves
/// an existing target locale slug, and materializes a missing locale with the
/// source slug only after reserving that locale/slug key through the canonical
/// Forum route owner.
pub struct ForumCategoryTranslationTargetProvider {
    db: DatabaseConnection,
}

impl ForumCategoryTranslationTargetProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Forum owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Forum resource kind must satisfy the target contract"),
            display_name: "Forum category".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
            ]),
            read_permission_floor: BTreeSet::from(["forum_categories:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["forum_categories:update".to_string()]),
        }
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let category_id = parse_identity(&request.identity)?;
        let category = CategoryEntity::find_by_id(category_id)
            .filter(CategoryColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(forum_database_error_to_port_error)?
            .ok_or_else(|| forum_category_not_found(category_id))?;
        ensure_category_is_active(&self.db, tenant_id, category_id).await?;
        let translations = TranslationEntity::find()
            .filter(TranslationColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::CategoryId.eq(category_id))
            .order_by_asc(TranslationColumn::Locale)
            .order_by_asc(TranslationColumn::Id)
            .all(&self.db)
            .await
            .map_err(forum_database_error_to_port_error)?;
        snapshot_from_models(category, translations, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(&self.db, lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Forum translation-target failure receipt"
            );
        }
    }

    async fn apply_exact_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        actor_id: Option<Uuid>,
        request: &TranslationPatchRequest,
        target: MergedTarget,
    ) -> Result<(OpaqueRevision, OpaqueRevision), PortError> {
        let category = CategoryEntity::find_by_id(category_id)
            .filter(CategoryColumn::TenantId.eq(tenant_id))
            .one(txn)
            .await
            .map_err(forum_database_error_to_port_error)?
            .ok_or_else(|| forum_category_not_found(category_id))?;
        ensure_category_is_active_in_tx(txn, tenant_id, category_id).await?;

        let source = TranslationEntity::find()
            .filter(TranslationColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::CategoryId.eq(category_id))
            .filter(TranslationColumn::Locale.eq(request.source_locale.as_str()))
            .one(txn)
            .await
            .map_err(forum_database_error_to_port_error)?
            .ok_or_else(|| {
                PortError::not_found(
                    "forum.translation_source_not_found",
                    "Exact source Forum category translation was not found",
                )
            })?;
        let existing_target = TranslationEntity::find()
            .filter(TranslationColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::CategoryId.eq(category_id))
            .filter(TranslationColumn::Locale.eq(request.target_locale.as_str()))
            .one(txn)
            .await
            .map_err(forum_database_error_to_port_error)?;

        let live_resource_revision = category_revision(&category);
        let live_source_revision = translation_revision(&source);
        let live_target_revision = existing_target.as_ref().map(translation_revision);
        if request.expected_resource_revision != live_resource_revision
            || request.expected_source_revision != live_source_revision
            || request.expected_target_revision != live_target_revision
        {
            return Err(PortError::conflict(
                "forum.translation_owner_conflict",
                "Forum category translation state changed before apply",
            ));
        }

        if target.name.trim().is_empty() {
            return Err(PortError::validation(
                "forum.translation_name_required",
                "Forum category translation name cannot be empty",
            ));
        }

        let previous_updated_at = category.updated_at;
        let mut next_updated_at = Utc::now().fixed_offset();
        if next_updated_at <= previous_updated_at {
            next_updated_at = previous_updated_at + chrono::Duration::microseconds(1);
        }
        let category_update = CategoryEntity::update_many()
            .col_expr(CategoryColumn::UpdatedAt, Expr::value(next_updated_at))
            .filter(CategoryColumn::Id.eq(category_id))
            .filter(CategoryColumn::TenantId.eq(tenant_id))
            .filter(CategoryColumn::UpdatedAt.eq(previous_updated_at))
            .exec(txn)
            .await
            .map_err(forum_database_error_to_port_error)?;
        if category_update.rows_affected != 1 {
            return Err(PortError::conflict(
                "forum.translation_owner_conflict",
                "Forum category changed before translation apply could commit",
            ));
        }

        let applied_target = match existing_target {
            Some(existing_target) => {
                let update = TranslationEntity::update_many()
                    .col_expr(TranslationColumn::Name, Expr::value(target.name.clone()))
                    .col_expr(
                        TranslationColumn::Description,
                        Expr::value(target.description.clone()),
                    )
                    .filter(TranslationColumn::Id.eq(existing_target.id))
                    .filter(TranslationColumn::TenantId.eq(tenant_id))
                    .filter(TranslationColumn::CategoryId.eq(category_id))
                    .filter(TranslationColumn::Locale.eq(request.target_locale.as_str()))
                    .filter(TranslationColumn::Name.eq(existing_target.name.clone()))
                    .filter(TranslationColumn::Slug.eq(existing_target.slug.clone()))
                    .filter(TranslationColumn::Description.eq(existing_target.description.clone()))
                    .exec(txn)
                    .await
                    .map_err(forum_database_error_to_port_error)?;
                if update.rows_affected != 1 {
                    return Err(PortError::conflict(
                        "forum.translation_owner_conflict",
                        "Forum target locale changed before translation apply could commit",
                    ));
                }
                TranslationModel {
                    name: target.name,
                    description: target.description,
                    ..existing_target
                }
            }
            None => {
                ForumCategoryRouteService::ensure_current_route_key_available_in_tx(
                    txn,
                    tenant_id,
                    category_id,
                    request.target_locale.as_str(),
                    &source.slug,
                )
                .await
                .map_err(forum_error_to_port_error)?;
                let inserted = crate::entities::forum_category_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    category_id: Set(category_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(request.target_locale.as_str().to_string()),
                    name: Set(target.name),
                    slug: Set(source.slug.clone()),
                    description: Set(target.description),
                }
                .insert(txn)
                .await;
                match inserted {
                    Ok(model) => model,
                    Err(error) if is_unique_constraint(&error) => {
                        return Err(PortError::conflict(
                            "forum.translation_owner_conflict",
                            "Forum target locale was created before translation apply could commit",
                        ));
                    }
                    Err(error) => return Err(forum_database_error_to_port_error(error)),
                }
            }
        };

        publish_forum_projection_scope_direct_in_tx(txn, tenant_id, actor_id)
            .await
            .map_err(forum_error_to_port_error)?;

        let applied_category = CategoryModel {
            updated_at: next_updated_at,
            ..category
        };
        Ok((
            category_revision(&applied_category),
            translation_revision(&applied_target),
        ))
    }
}

#[async_trait]
impl TranslationTargetProvider for ForumCategoryTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        Self::descriptor_value()
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let after = request
            .cursor
            .as_ref()
            .map(|cursor| {
                Uuid::parse_str(cursor.as_str()).map_err(|_| {
                    PortError::validation(
                        "forum.translation_cursor_invalid",
                        "Forum category translation cursor must be a category UUID",
                    )
                })
            })
            .transpose()?;

        let mut query = CategoryEntity::find()
            .inner_join(TranslationEntity)
            .filter(CategoryColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::Locale.eq(request.source_locale.as_str()))
            .filter(
                Expr::col((CategoryEntity, CategoryColumn::Id))
                    .not_in_subquery(archived_category_ids_subquery(tenant_id)),
            )
            .order_by_asc(CategoryColumn::Id);
        if let Some(after) = after {
            query = query.filter(CategoryColumn::Id.gt(after));
        }
        let mut categories = query
            .limit(u64::from(request.limit) + 1)
            .all(&self.db)
            .await
            .map_err(forum_database_error_to_port_error)?;
        let has_more = categories.len() > usize::from(request.limit);
        if has_more {
            categories.truncate(usize::from(request.limit));
        }

        let category_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<Vec<_>>();
        let translations = if category_ids.is_empty() {
            Vec::new()
        } else {
            TranslationEntity::find()
                .filter(TranslationColumn::TenantId.eq(tenant_id))
                .filter(TranslationColumn::CategoryId.is_in(category_ids))
                .order_by_asc(TranslationColumn::CategoryId)
                .order_by_asc(TranslationColumn::Locale)
                .order_by_asc(TranslationColumn::Id)
                .all(&self.db)
                .await
                .map_err(forum_database_error_to_port_error)?
        };
        let mut translations_by_category = BTreeMap::<Uuid, Vec<TranslationModel>>::new();
        for translation in translations {
            translations_by_category
                .entry(translation.category_id)
                .or_default()
                .push(translation);
        }

        let resources = categories
            .iter()
            .map(|category| {
                summary_from_models(
                    category,
                    translations_by_category
                        .get(&category.id)
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                    &request.source_locale,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = has_more
            .then(|| categories.last())
            .flatten()
            .map(|category| {
                OpaqueCursor::new(category.id.to_string())
                    .expect("Forum category UUID must satisfy the opaque cursor contract")
            });

        Ok(TranslationResourcePage {
            resources,
            next_cursor,
        })
    }

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        if request.source_locale == request.target_locale {
            return Err(PortError::validation(
                "translation.equal_source_target_locale",
                "source and target locale must differ",
            ));
        }
        let tenant_id = parse_tenant_id(&context)?;
        self.load_snapshot(tenant_id, &request).await
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let snapshot = self
            .load_snapshot(tenant_id, &read_request_from_patch(&request))
            .await?;
        Ok(validate_patch_against_snapshot(&request, &snapshot))
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        validate_translation_apply_context(&context)?;
        let security = authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let category_id = parse_identity(&request.identity)?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        let lease = match idempotency::admit(
            &self.db,
            tenant_id,
            TRANSLATION_OWNER_SLUG,
            idempotency_key,
            OPERATION_APPLY_PATCH,
            &request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => return decode_application_receipt(value),
            Admission::ReplayError(error) => return Err(error),
        };

        let result = async {
            let snapshot = self
                .load_snapshot(tenant_id, &read_request_from_patch(&request))
                .await?;
            let validation = validate_patch_against_snapshot(&request, &snapshot);
            if !validation.accepted {
                return Err(validation_to_port_error(&validation));
            }
            let target = merged_target(&request, &snapshot)?;
            let txn = self
                .db
                .begin()
                .await
                .map_err(forum_database_error_to_port_error)?;
            let (resource_revision, target_revision) = self
                .apply_exact_translation_in_tx(
                    &txn,
                    tenant_id,
                    category_id,
                    security.user_id,
                    &request,
                    target,
                )
                .await?;
            let receipt = TranslationApplicationReceipt {
                provider_receipt_id: format!("forum:{}", lease.operation_id),
                resource_revision,
                target_revision,
                applied_field_keys: request
                    .fields
                    .iter()
                    .map(|field| field.key.clone())
                    .collect(),
            };
            idempotency::complete(&txn, lease, &receipt).await?;
            txn.commit()
                .await
                .map_err(forum_database_error_to_port_error)?;
            Ok(receipt)
        }
        .await;

        if let Err(error) = &result {
            self.fail_receipt(lease, error).await;
        }
        result
    }
}

fn archived_category_ids_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(LifecycleColumn::CategoryId)
        .from(LifecycleEntity)
        .and_where(Expr::col((LifecycleEntity, LifecycleColumn::TenantId)).eq(tenant_id))
        .to_owned()
}

async fn ensure_category_is_active(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> Result<(), PortError> {
    let archived = LifecycleEntity::find()
        .filter(LifecycleColumn::TenantId.eq(tenant_id))
        .filter(LifecycleColumn::CategoryId.eq(category_id))
        .one(db)
        .await
        .map_err(forum_database_error_to_port_error)?
        .is_some();
    if archived {
        return Err(forum_category_not_found(category_id));
    }
    Ok(())
}

async fn ensure_category_is_active_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> Result<(), PortError> {
    let archived = LifecycleEntity::find()
        .filter(LifecycleColumn::TenantId.eq(tenant_id))
        .filter(LifecycleColumn::CategoryId.eq(category_id))
        .one(txn)
        .await
        .map_err(forum_database_error_to_port_error)?
        .is_some();
    if archived {
        return Err(forum_category_not_found(category_id));
    }
    Ok(())
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

fn parse_identity(identity: &TranslationResourceIdentity) -> Result<Uuid, PortError> {
    if identity.owner_slug.as_str() != TRANSLATION_OWNER_SLUG
        || identity.resource_kind.as_str() != TRANSLATION_RESOURCE_KIND
        || identity.subresource_id.is_some()
    {
        return Err(PortError::validation(
            "forum.translation_identity_invalid",
            "Forum translation identity must address forum/category without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "forum.translation_resource_id_invalid",
            "Forum category translation resource id must be a UUID",
        )
    })
}

fn summary_from_models(
    category: &CategoryModel,
    translations: &[TranslationModel],
    source_locale: &TenantLocale,
) -> Result<TranslationResourceSummary, PortError> {
    let source = translations
        .iter()
        .find(|translation| translation.locale == source_locale.as_str())
        .ok_or_else(|| {
            PortError::invariant_violation(
                "forum.translation_source_missing",
                "Forum category was listed without its exact source locale",
            )
        })?;
    let exact_locales = translations
        .iter()
        .map(|translation| {
            TenantLocale::new(&translation.locale).map_err(|error| {
                PortError::invariant_violation(
                    "forum.translation_locale_invalid",
                    error.to_string(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TranslationResourceSummary {
        identity: forum_category_identity(category.id),
        display_label: source.name.clone(),
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: category_revision(category),
        exact_locales,
    })
}

fn snapshot_from_models(
    category: CategoryModel,
    translations: Vec<TranslationModel>,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let source = translations
        .iter()
        .find(|translation| translation.locale == request.source_locale.as_str())
        .ok_or_else(|| {
            PortError::not_found(
                "forum.translation_source_not_found",
                "Exact source Forum category translation was not found",
            )
        })?;
    let target = translations
        .iter()
        .find(|translation| translation.locale == request.target_locale.as_str());
    let snapshot = TranslationResourceSnapshot {
        summary: summary_from_models(&category, &translations, &request.source_locale)?,
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
        rendered_fallback_locale: None,
        source_revision: translation_revision(source),
        target_revision: target.map(translation_revision),
        fields: translation_fields(source, target),
    };
    snapshot.validate().map_err(|error| {
        PortError::invariant_violation("forum.translation_snapshot_invalid", error.to_string())
    })?;
    Ok(snapshot)
}

fn forum_category_identity(category_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Forum owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Forum resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(category_id.to_string())
            .expect("Forum category UUID must satisfy the resource id contract"),
        subresource_id: None,
    }
}

fn translation_fields(
    source: &TranslationModel,
    target: Option<&TranslationModel>,
) -> Vec<TranslationFieldSnapshot> {
    [
        (
            "name",
            source.name.as_str(),
            target.map(|translation| translation.name.as_str()),
            true,
        ),
        (
            "description",
            source.description.as_deref().unwrap_or_default(),
            target.and_then(|translation| translation.description.as_deref()),
            false,
        ),
    ]
    .into_iter()
    .map(
        |(key, source_value, target_value, required)| TranslationFieldSnapshot {
            descriptor: TranslationFieldDescriptor {
                key: FieldKey::new(key).expect("static Forum field key must satisfy the contract"),
                profile: TranslationValueProfile::PlainText,
                strategy: TranslationStrategy::Translate,
                classification: TranslationDataClassification::Public,
                required,
                ai_export_allowed: true,
                max_characters: None,
                preserves_whitespace: false,
            },
            source_value: source_value.to_string(),
            exact_target_value: target_value.map(str::to_string),
            source_hash: field_hash(source_value),
            protected_tokens: Vec::new(),
        },
    )
    .collect()
}

struct MergedTarget {
    name: String,
    description: Option<String>,
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<MergedTarget, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    let name = required_target_value(values.remove("name").flatten(), "name")?;
    let description = values
        .remove("description")
        .flatten()
        .and_then(normalize_optional_target_value);
    Ok(MergedTarget { name, description })
}

fn category_revision(category: &CategoryModel) -> OpaqueRevision {
    let payload = serde_json::to_string(category)
        .expect("Forum category model must serialize for optimistic revision");
    OpaqueRevision::new(field_hash(&payload))
        .expect("SHA-256 Forum category revision must satisfy the opaque revision contract")
}

fn translation_revision(translation: &TranslationModel) -> OpaqueRevision {
    let payload = serde_json::to_string(translation)
        .expect("Forum category translation model must serialize for optimistic revision");
    OpaqueRevision::new(field_hash(&payload))
        .expect("SHA-256 Forum translation revision must satisfy the opaque revision contract")
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}

fn forum_category_not_found(category_id: Uuid) -> PortError {
    PortError::not_found(
        "forum.translation_resource_not_found",
        format!("Forum category translation resource was not found: {category_id}"),
    )
}

fn forum_database_error_to_port_error(error: sea_orm::DbErr) -> PortError {
    forum_error_to_port_error(ForumError::Database(error))
}

fn forum_error_to_port_error(error: ForumError) -> PortError {
    match error {
        ForumError::CategoryNotFound(_) | ForumError::CategoryRouteNotFound => {
            PortError::not_found(
                "forum.translation_resource_not_found",
                "Forum category translation resource was not found",
            )
        }
        ForumError::RelationRevisionConflict | ForumError::CategoryRouteResolutionConflict => {
            PortError::conflict(
                "forum.translation_owner_conflict",
                "Forum state conflicts with the requested translation mutation",
            )
        }
        ForumError::Forbidden(_) => PortError::forbidden(
            "forum.translation_permission_denied",
            "Forum category permission is required",
        ),
        ForumError::Validation(_) => PortError::validation(
            "forum.translation_owner_validation",
            "Forum rejected the translation mutation",
        ),
        other if other.is_retryable() => PortError::unavailable(
            "forum.translation_owner_unavailable",
            "Forum translation storage is unavailable",
        ),
        _ => PortError::validation(
            "forum.translation_owner_validation",
            "Forum rejected the translation mutation",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_exposes_plain_text_owner_capabilities_without_change_cursor() {
        let descriptor = ForumCategoryTranslationTargetProvider::descriptor_value();
        assert_eq!(descriptor.owner_slug.as_str(), "forum");
        assert_eq!(descriptor.resource_kind.as_str(), "category");
        assert!(
            descriptor
                .capabilities
                .contains(&TranslationTargetCapability::ApplyPatch)
        );
        assert!(
            !descriptor
                .capabilities
                .contains(&TranslationTargetCapability::ChangeCursor)
        );
    }

    #[test]
    fn translation_revision_includes_route_slug_even_though_slug_is_not_translated() {
        let original = TranslationModel {
            id: Uuid::new_v4(),
            category_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            locale: "en".to_string(),
            name: "General".to_string(),
            slug: "general".to_string(),
            description: Some("General discussion".to_string()),
        };
        let changed = TranslationModel {
            slug: "general-chat".to_string(),
            ..original.clone()
        };
        assert_ne!(
            translation_revision(&original),
            translation_revision(&changed)
        );
    }
}
