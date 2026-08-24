use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::idempotency::{self, Admission};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetChange, TranslationTargetChangePage, TranslationTargetChangesRequest,
    TranslationTargetProgressFacts, TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationValueProfile,
    provider_support::{
        contract_validation_error, decode_application_receipt, field_hash, merged_patch_values,
        normalize_optional_target_value, opaque_positive_revision, parse_positive_revision,
        parse_resource_lifecycle, read_request_from_patch, required_target_value,
        validate_patch_against_snapshot, validation_to_port_error,
    },
    validate_translation_apply_context, validate_translation_read_context,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait};
use uuid::Uuid;

use crate::{
    BlogError, CategoryService,
    entities::{
        blog_category::{
            Column as CategoryColumn, Entity as CategoryEntity, Model as CategoryModel,
        },
        blog_category_translation::{
            Column as CategoryTranslationColumn, Entity as CategoryTranslationEntity,
            Model as CategoryTranslationModel,
        },
        translation_change::{
            Column as TranslationChangeColumn, Entity as TranslationChangeEntity,
            Model as TranslationChangeModel,
        },
    },
    services::ApplyExactCategoryTranslationInput,
    translation_evidence::{TRANSLATION_OWNER_SLUG, TRANSLATION_RESOURCE_KIND},
};

const OPERATION_APPLY_PATCH: &str = "translation_target_apply_patch";
const REQUIRED_FIELD_COUNT: u64 = 2;
const OPTIONAL_FIELD_COUNT: u64 = 1;
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
/// Owner adapter for exact Blog category localization. It calls the canonical
/// Blog service and never exposes Blog tables to Translation directly.
pub struct BlogCategoryTranslationTargetProvider {
    service: Arc<CategoryService>,
}

impl BlogCategoryTranslationTargetProvider {
    pub fn new(service: Arc<CategoryService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Blog owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Blog resource kind must satisfy the target contract"),
            display_name: "Blog category".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["blog_categories:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["blog_categories:update".to_string()]),
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
            .one(self.service.database())
            .await
            .map_err(blog_database_error_to_port_error)?
            .ok_or_else(|| {
                PortError::not_found(
                    "blog.translation_resource_not_found",
                    "Blog category translation resource was not found",
                )
            })?;
        let translations = CategoryTranslationEntity::find()
            .filter(CategoryTranslationColumn::TenantId.eq(tenant_id))
            .filter(CategoryTranslationColumn::CategoryId.eq(category_id))
            .order_by_asc(CategoryTranslationColumn::Locale)
            .all(self.service.database())
            .await
            .map_err(blog_database_error_to_port_error)?;
        snapshot_from_models(category, translations, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Blog translation-target failure receipt"
            );
        }
    }

    async fn latest_change_cursor(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<OpaqueCursor>, PortError> {
        TranslationChangeEntity::find()
            .filter(TranslationChangeColumn::TenantId.eq(tenant_id))
            .filter(TranslationChangeColumn::ResourceKind.eq(TRANSLATION_RESOURCE_KIND))
            .order_by_desc(TranslationChangeColumn::Id)
            .one(self.service.database())
            .await
            .map_err(blog_database_error_to_port_error)?
            .map(|change| {
                OpaqueCursor::new(change.id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "blog.translation_change_cursor_invalid",
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
            .inner_join(CategoryTranslationEntity)
            .filter(CategoryColumn::TenantId.eq(tenant_id))
            .filter(CategoryTranslationColumn::TenantId.eq(tenant_id))
            .filter(CategoryTranslationColumn::Locale.eq(request.source_locale.as_str()))
            .order_by_asc(CategoryColumn::Id)
            .all(self.service.database())
            .await
            .map_err(blog_database_error_to_port_error)?;
        let category_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<Vec<_>>();
        let targets = if category_ids.is_empty() {
            Vec::new()
        } else {
            CategoryTranslationEntity::find()
                .filter(CategoryTranslationColumn::TenantId.eq(tenant_id))
                .filter(CategoryTranslationColumn::CategoryId.is_in(category_ids))
                .filter(CategoryTranslationColumn::Locale.eq(request.target_locale.as_str()))
                .all(self.service.database())
                .await
                .map_err(blog_database_error_to_port_error)?
        };
        let targets = targets
            .into_iter()
            .map(|translation| (translation.category_id, translation))
            .collect::<BTreeMap<_, _>>();
        let resources = u64::try_from(categories.len()).map_err(|_| {
            PortError::invariant_violation(
                "blog.translation_progress_overflow",
                "Blog category resource count exceeds the progress contract",
            )
        })?;
        let required_units = resources.checked_mul(REQUIRED_FIELD_COUNT).ok_or_else(|| {
            PortError::invariant_violation(
                "blog.translation_progress_overflow",
                "Blog category required progress count overflow",
            )
        })?;
        let optional_units = resources.checked_mul(OPTIONAL_FIELD_COUNT).ok_or_else(|| {
            PortError::invariant_violation(
                "blog.translation_progress_overflow",
                "Blog category optional progress count overflow",
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
            let has_slug = !target.slug.trim().is_empty();
            exact_required_units = exact_required_units
                .checked_add(u64::from(has_name) + u64::from(has_slug))
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "blog.translation_progress_overflow",
                        "Blog category exact required progress count overflow",
                    )
                })?;
            if target
                .description
                .as_deref()
                .is_some_and(|description| !description.trim().is_empty())
            {
                exact_optional_units = exact_optional_units.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "blog.translation_progress_overflow",
                        "Blog category exact optional progress count overflow",
                    )
                })?;
            }
            if has_name && has_slug {
                complete_resources = complete_resources.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "blog.translation_progress_overflow",
                        "Blog category complete resource count overflow",
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
impl TranslationTargetProvider for BlogCategoryTranslationTargetProvider {
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
                        "blog.translation_cursor_invalid",
                        "Blog category translation cursor must be a category UUID",
                    )
                })
            })
            .transpose()?;
        let mut query = CategoryEntity::find()
            .inner_join(CategoryTranslationEntity)
            .filter(CategoryColumn::TenantId.eq(tenant_id))
            .filter(CategoryTranslationColumn::TenantId.eq(tenant_id))
            .filter(CategoryTranslationColumn::Locale.eq(request.source_locale.as_str()))
            .order_by_asc(CategoryColumn::Id);
        if let Some(after) = after {
            query = query.filter(CategoryColumn::Id.gt(after));
        }
        let mut categories = query
            .limit(u64::from(request.limit) + 1)
            .all(self.service.database())
            .await
            .map_err(blog_database_error_to_port_error)?;
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
            CategoryTranslationEntity::find()
                .filter(CategoryTranslationColumn::TenantId.eq(tenant_id))
                .filter(CategoryTranslationColumn::CategoryId.is_in(category_ids))
                .order_by_asc(CategoryTranslationColumn::CategoryId)
                .order_by_asc(CategoryTranslationColumn::Locale)
                .all(self.service.database())
                .await
                .map_err(blog_database_error_to_port_error)?
        };
        let mut translations_by_category = BTreeMap::<Uuid, Vec<CategoryTranslationModel>>::new();
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
                    .expect("Blog category UUID cursor must satisfy the opaque cursor contract")
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
            self.service.database(),
            idempotency::OwnerOperationScope::Tenant(tenant_id),
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
        let actor_id = security.user_id;

        let result = async {
            let snapshot = self
                .load_snapshot(tenant_id, &read_request_from_patch(&request))
                .await?;
            let validation = validate_patch_against_snapshot(&request, &snapshot);
            if !validation.accepted {
                return Err(validation_to_port_error(&validation));
            }
            let target = merged_target(&request, &snapshot)?;
            let transaction = self
                .service
                .database()
                .begin()
                .await
                .map_err(blog_database_error_to_port_error)?;
            let applied = self
                .service
                .apply_exact_translation_in_tx(
                    &transaction,
                    tenant_id,
                    category_id,
                    ApplyExactCategoryTranslationInput {
                        source_locale: request.source_locale.clone(),
                        target_locale: request.target_locale.clone(),
                        name: target.name,
                        slug: target.slug,
                        description: target.description,
                        expected_resource_revision: parse_positive_revision(
                            &request.expected_resource_revision,
                            "expected_resource_revision",
                        )?,
                        expected_source_revision: parse_positive_revision(
                            &request.expected_source_revision,
                            "expected_source_revision",
                        )?,
                        expected_target_revision: request
                            .expected_target_revision
                            .as_ref()
                            .map(|revision| {
                                parse_positive_revision(revision, "expected_target_revision")
                            })
                            .transpose()?,
                        actor_id,
                    },
                )
                .await
                .map_err(blog_error_to_port_error)?;
            let receipt = TranslationApplicationReceipt {
                provider_receipt_id: format!("blog:{}", lease.operation_id),
                resource_revision: opaque_positive_revision(
                    applied.resource_revision,
                    "resource_revision",
                )?,
                target_revision: opaque_positive_revision(
                    applied.target_revision,
                    "target_revision",
                )?,
                applied_field_keys: request
                    .fields
                    .iter()
                    .map(|field| field.key.clone())
                    .collect(),
            };
            idempotency::complete(&transaction, lease, &receipt).await?;
            transaction
                .commit()
                .await
                .map_err(blog_database_error_to_port_error)?;
            Ok(receipt)
        }
        .await;

        if let Err(error) = &result {
            self.fail_receipt(lease, error).await;
        }
        result
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
                        "blog.translation_progress_invalid",
                        error.to_string(),
                    )
                })?;
                return Ok(facts);
            }
        }

        Err(PortError::unavailable(
            "blog.translation_progress_unstable",
            "Blog category translation progress changed while it was being aggregated",
        ))
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let after = request
            .after
            .as_ref()
            .map(|cursor| {
                Uuid::parse_str(cursor.as_str()).map_err(|_| {
                    PortError::validation(
                        "blog.translation_change_cursor_invalid",
                        "Blog category translation change cursor must be a change UUID",
                    )
                })
            })
            .transpose()?;
        let mut query = TranslationChangeEntity::find()
            .filter(TranslationChangeColumn::TenantId.eq(tenant_id))
            .filter(TranslationChangeColumn::ResourceKind.eq(TRANSLATION_RESOURCE_KIND))
            .order_by_asc(TranslationChangeColumn::Id);
        if let Some(after) = after {
            query = query.filter(TranslationChangeColumn::Id.gt(after));
        }
        let rows = query
            .limit(u64::from(request.limit))
            .all(self.service.database())
            .await
            .map_err(blog_database_error_to_port_error)?;
        let next_cursor = rows.last().map(|change| {
            OpaqueCursor::new(change.id.to_string())
                .expect("Blog change UUID must satisfy the opaque cursor contract")
        });
        let changes = rows
            .into_iter()
            .map(change_from_model)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TranslationTargetChangePage {
            changes,
            next_cursor,
        })
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "blog.invalid_tenant_id",
            "Blog translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::BlogCategories, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "blog.translation_permission_denied",
            format!("blog_categories:{action} permission is required"),
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
            "blog.translation_identity_invalid",
            "Blog translation identity must address blog/category without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "blog.translation_resource_id_invalid",
            "Blog category translation resource id must be a UUID",
        )
    })
}

fn summary_from_models(
    category: &CategoryModel,
    translations: &[CategoryTranslationModel],
    source_locale: &TenantLocale,
) -> Result<TranslationResourceSummary, PortError> {
    let source = translations
        .iter()
        .find(|translation| translation.locale == source_locale.as_str())
        .ok_or_else(|| {
            PortError::invariant_violation(
                "blog.translation_source_missing",
                "Blog category was listed without its exact source locale",
            )
        })?;
    let exact_locales = translations
        .iter()
        .filter_map(|translation| TenantLocale::new(&translation.locale).ok())
        .collect::<Vec<_>>();
    Ok(TranslationResourceSummary {
        identity: blog_category_identity(category.id),
        display_label: source.name.clone(),
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: opaque_positive_revision(category.revision, "resource_revision")?,
        exact_locales,
    })
}

fn snapshot_from_models(
    category: CategoryModel,
    translations: Vec<CategoryTranslationModel>,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let source = translations
        .iter()
        .find(|translation| translation.locale == request.source_locale.as_str())
        .ok_or_else(|| {
            PortError::not_found(
                "blog.translation_source_not_found",
                "Exact source Blog category translation was not found",
            )
        })?;
    let target = translations
        .iter()
        .find(|translation| translation.locale == request.target_locale.as_str());
    let summary = summary_from_models(&category, &translations, &request.source_locale)?;
    let snapshot = TranslationResourceSnapshot {
        summary,
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
        rendered_fallback_locale: None,
        source_revision: opaque_positive_revision(source.revision, "source_revision")?,
        target_revision: target
            .map(|translation| opaque_positive_revision(translation.revision, "target_revision"))
            .transpose()?,
        fields: translation_fields(source, target),
    };
    snapshot.validate().map_err(|error| {
        PortError::invariant_violation("blog.translation_snapshot_invalid", error.to_string())
    })?;
    Ok(snapshot)
}

fn blog_category_identity(category_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Blog owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Blog resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(category_id.to_string())
            .expect("Blog category UUID must satisfy the resource id contract"),
        subresource_id: None,
    }
}

fn translation_fields(
    source: &CategoryTranslationModel,
    target: Option<&CategoryTranslationModel>,
) -> Vec<TranslationFieldSnapshot> {
    [
        (
            "name",
            source.name.as_str(),
            target.map(|translation| translation.name.as_str()),
            TranslationValueProfile::PlainText,
            TranslationStrategy::Translate,
            true,
            true,
            Some(255),
        ),
        (
            "slug",
            source.slug.as_str(),
            target.map(|translation| translation.slug.as_str()),
            TranslationValueProfile::Slug,
            TranslationStrategy::TransliterateWithReview,
            true,
            false,
            Some(255),
        ),
        (
            "description",
            source.description.as_deref().unwrap_or_default(),
            target.and_then(|translation| translation.description.as_deref()),
            TranslationValueProfile::PlainText,
            TranslationStrategy::Translate,
            false,
            true,
            Some(1_000),
        ),
    ]
    .into_iter()
    .map(
        |(key, source_value, target_value, profile, strategy, required, ai_export_allowed, max)| {
            TranslationFieldSnapshot {
                descriptor: TranslationFieldDescriptor {
                    key: FieldKey::new(key)
                        .expect("static Blog category field key must satisfy the contract"),
                    profile,
                    strategy,
                    classification: TranslationDataClassification::Public,
                    required,
                    ai_export_allowed,
                    max_characters: max,
                    preserves_whitespace: false,
                },
                source_value: source_value.to_string(),
                exact_target_value: target_value.map(str::to_string),
                source_hash: field_hash(source_value),
                protected_tokens: Vec::new(),
            }
        },
    )
    .collect()
}

struct MergedTarget {
    name: String,
    slug: String,
    description: Option<String>,
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<MergedTarget, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    let name = required_target_value(values.remove("name").flatten(), "name")?;
    let slug = required_target_value(values.remove("slug").flatten(), "slug")?;
    let description = values
        .remove("description")
        .flatten()
        .and_then(normalize_optional_target_value);
    Ok(MergedTarget {
        name,
        slug,
        description,
    })
}

fn change_from_model(change: TranslationChangeModel) -> Result<TranslationTargetChange, PortError> {
    Ok(TranslationTargetChange {
        identity: blog_category_identity(change.resource_id),
        resource_revision: opaque_positive_revision(change.resource_revision, "resource_revision")?,
        lifecycle: parse_resource_lifecycle(&change.lifecycle)?,
    })
}

fn blog_error_to_port_error(error: BlogError) -> PortError {
    match error {
        BlogError::CategoryNotFound(_)
        | BlogError::PostNotFound(_)
        | BlogError::CommentNotFound(_)
        | BlogError::TagNotFound(_) => PortError::not_found(
            "blog.translation_resource_not_found",
            "Blog translation resource was not found",
        ),
        BlogError::DuplicateSlug { .. } | BlogError::Conflict(_) => PortError::conflict(
            "blog.translation_owner_conflict",
            "Blog state conflicts with the requested translation mutation",
        ),
        BlogError::Forbidden(_) => PortError::forbidden(
            "blog.translation_permission_denied",
            "Blog category permission is required",
        ),
        BlogError::Validation(_)
        | BlogError::CannotDeletePublished
        | BlogError::CannotPublishArchived
        | BlogError::AuthorRequired => PortError::validation(
            "blog.translation_owner_validation",
            "Blog rejected the translation mutation",
        ),
        BlogError::CategoryTranslationRevisionExhausted { .. } => PortError::invariant_violation(
            "blog.translation_revision_exhausted",
            "Blog category translation revision is exhausted",
        ),
        BlogError::Database(_)
        | BlogError::Content(_)
        | BlogError::Comments(_)
        | BlogError::Rich(_)
        | BlogError::Core(_) => PortError::unavailable(
            "blog.translation_owner_unavailable",
            "Blog translation storage is unavailable",
        ),
    }
}

fn blog_database_error_to_port_error(error: sea_orm::DbErr) -> PortError {
    blog_error_to_port_error(BlogError::Database(error))
}
