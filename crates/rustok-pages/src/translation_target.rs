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
    PageService, PagesError,
    entities::{
        page::{Column as PageColumn, Entity as PageEntity, Model as PageModel},
        page_translation::{
            Column as PageTranslationColumn, Entity as PageTranslationEntity,
            Model as PageTranslationModel,
        },
        translation_change::{
            Column as TranslationChangeColumn, Entity as TranslationChangeEntity,
            Model as TranslationChangeModel,
        },
    },
    services::page::ApplyExactPageMetadataTranslationInput,
    translation_evidence::{TRANSLATION_OWNER_SLUG, TRANSLATION_RESOURCE_KIND},
};

const OPERATION_APPLY_PATCH: &str = "translation_target_apply_patch";
const REQUIRED_FIELD_COUNT: u64 = 2;
const OPTIONAL_FIELD_COUNT: u64 = 2;
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
/// Owner adapter for exact Page metadata localization. It delegates writes to
/// `PageService`, so Translation never obtains direct write access to Pages rows.
pub struct PagesMetadataTranslationTargetProvider {
    service: Arc<PageService>,
}

impl PagesMetadataTranslationTargetProvider {
    pub fn new(service: Arc<PageService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Pages owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Pages resource kind must satisfy the target contract"),
            display_name: "Page metadata".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["pages:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["pages:update".to_string()]),
        }
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let page_id = parse_identity(&request.identity)?;
        let page = PageEntity::find_by_id(page_id)
            .filter(PageColumn::TenantId.eq(tenant_id))
            .one(self.service.database())
            .await
            .map_err(pages_database_error_to_port_error)?
            .ok_or_else(|| {
                PortError::not_found(
                    "pages.translation_resource_not_found",
                    "Page metadata translation resource was not found",
                )
            })?;
        let translations = self.load_translations(tenant_id, vec![page_id]).await?;
        snapshot_from_models(
            page,
            translations
                .get(&page_id)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            request,
        )
    }

    async fn load_translations(
        &self,
        tenant_id: Uuid,
        page_ids: Vec<Uuid>,
    ) -> Result<BTreeMap<Uuid, Vec<PageTranslationModel>>, PortError> {
        if page_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let translations = PageTranslationEntity::find()
            .filter(PageTranslationColumn::TenantId.eq(tenant_id))
            .filter(PageTranslationColumn::PageId.is_in(page_ids))
            .order_by_asc(PageTranslationColumn::PageId)
            .order_by_asc(PageTranslationColumn::Locale)
            .all(self.service.database())
            .await
            .map_err(pages_database_error_to_port_error)?;
        let mut by_page = BTreeMap::<Uuid, Vec<PageTranslationModel>>::new();
        for translation in translations {
            by_page
                .entry(translation.page_id)
                .or_default()
                .push(translation);
        }
        Ok(by_page)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Pages translation-target failure receipt"
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
            .map_err(pages_database_error_to_port_error)?
            .map(|change| {
                OpaqueCursor::new(change.id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "pages.translation_change_cursor_invalid",
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
        let pages = PageEntity::find()
            .inner_join(PageTranslationEntity)
            .filter(PageColumn::TenantId.eq(tenant_id))
            .filter(PageColumn::Status.ne("archived"))
            .filter(PageTranslationColumn::TenantId.eq(tenant_id))
            .filter(PageTranslationColumn::Locale.eq(request.source_locale.as_str()))
            .order_by_asc(PageColumn::Id)
            .all(self.service.database())
            .await
            .map_err(pages_database_error_to_port_error)?;
        let page_ids = pages.iter().map(|page| page.id).collect::<Vec<_>>();
        let target_rows = if page_ids.is_empty() {
            Vec::new()
        } else {
            PageTranslationEntity::find()
                .filter(PageTranslationColumn::TenantId.eq(tenant_id))
                .filter(PageTranslationColumn::PageId.is_in(page_ids))
                .filter(PageTranslationColumn::Locale.eq(request.target_locale.as_str()))
                .all(self.service.database())
                .await
                .map_err(pages_database_error_to_port_error)?
        };
        let targets = target_rows
            .into_iter()
            .map(|translation| (translation.page_id, translation))
            .collect::<BTreeMap<_, _>>();
        let resources = u64::try_from(pages.len()).map_err(|_| {
            PortError::invariant_violation(
                "pages.translation_progress_overflow",
                "Page metadata resource count exceeds the progress contract",
            )
        })?;
        let required_units = resources.checked_mul(REQUIRED_FIELD_COUNT).ok_or_else(|| {
            PortError::invariant_violation(
                "pages.translation_progress_overflow",
                "Page metadata required progress count overflow",
            )
        })?;
        let optional_units = resources.checked_mul(OPTIONAL_FIELD_COUNT).ok_or_else(|| {
            PortError::invariant_violation(
                "pages.translation_progress_overflow",
                "Page metadata optional progress count overflow",
            )
        })?;

        let mut exact_required_units = 0_u64;
        let mut exact_optional_units = 0_u64;
        let mut complete_resources = 0_u64;
        for page in pages {
            let Some(target) = targets.get(&page.id) else {
                continue;
            };
            let has_title = !target.title.trim().is_empty();
            let has_slug = !target.slug.trim().is_empty();
            exact_required_units = exact_required_units
                .checked_add(u64::from(has_title) + u64::from(has_slug))
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "pages.translation_progress_overflow",
                        "Page metadata exact required progress count overflow",
                    )
                })?;
            for optional in [&target.meta_title, &target.meta_description] {
                if optional
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
                {
                    exact_optional_units =
                        exact_optional_units.checked_add(1).ok_or_else(|| {
                            PortError::invariant_violation(
                                "pages.translation_progress_overflow",
                                "Page metadata exact optional progress count overflow",
                            )
                        })?;
                }
            }
            if has_title && has_slug {
                complete_resources = complete_resources.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "pages.translation_progress_overflow",
                        "Page metadata complete resource count overflow",
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
impl TranslationTargetProvider for PagesMetadataTranslationTargetProvider {
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
                        "pages.translation_cursor_invalid",
                        "Page metadata translation cursor must be a Page UUID",
                    )
                })
            })
            .transpose()?;
        let mut query = PageEntity::find()
            .inner_join(PageTranslationEntity)
            .filter(PageColumn::TenantId.eq(tenant_id))
            .filter(PageColumn::Status.ne("archived"))
            .filter(PageTranslationColumn::TenantId.eq(tenant_id))
            .filter(PageTranslationColumn::Locale.eq(request.source_locale.as_str()))
            .order_by_asc(PageColumn::Id);
        if let Some(after) = after {
            query = query.filter(PageColumn::Id.gt(after));
        }
        let mut pages = query
            .limit(u64::from(request.limit) + 1)
            .all(self.service.database())
            .await
            .map_err(pages_database_error_to_port_error)?;
        let has_more = pages.len() > usize::from(request.limit);
        if has_more {
            pages.truncate(usize::from(request.limit));
        }
        let next_cursor = has_more.then(|| pages.last()).flatten().map(|page| {
            OpaqueCursor::new(page.id.to_string())
                .expect("Pages UUID cursor must satisfy the opaque cursor contract")
        });
        let page_ids = pages.iter().map(|page| page.id).collect::<Vec<_>>();
        let translations = self.load_translations(tenant_id, page_ids).await?;
        let resources = pages
            .iter()
            .map(|page| {
                summary_from_models(
                    page,
                    translations
                        .get(&page.id)
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                    &request.source_locale,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

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
        let page_id = parse_identity(&request.identity)?;
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
                .map_err(pages_database_error_to_port_error)?;
            let applied = self
                .service
                .apply_exact_metadata_translation_in_tx(
                    &transaction,
                    tenant_id,
                    page_id,
                    ApplyExactPageMetadataTranslationInput {
                        source_locale: request.source_locale.clone(),
                        target_locale: request.target_locale.clone(),
                        title: target.title,
                        slug: target.slug,
                        meta_title: target.meta_title,
                        meta_description: target.meta_description,
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
                        actor_id: security.user_id,
                    },
                )
                .await
                .map_err(pages_error_to_port_error)?;
            let receipt = TranslationApplicationReceipt {
                provider_receipt_id: format!("pages:{}", lease.operation_id),
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
                .map_err(pages_database_error_to_port_error)?;
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
                        "pages.translation_progress_invalid",
                        error.to_string(),
                    )
                })?;
                return Ok(facts);
            }
        }

        Err(PortError::unavailable(
            "pages.translation_progress_unstable",
            "Page metadata translation progress changed while it was being aggregated",
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
                        "pages.translation_change_cursor_invalid",
                        "Page metadata translation cursor must be a change UUID",
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
            .map_err(pages_database_error_to_port_error)?;
        let next_cursor = rows.last().map(|change| {
            OpaqueCursor::new(change.id.to_string())
                .expect("Pages change UUID must satisfy the opaque cursor contract")
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
            "pages.invalid_tenant_id",
            "Pages translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Pages, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "pages.translation_permission_denied",
            format!("pages:{action} permission is required"),
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
            "pages.translation_identity_invalid",
            "Pages translation identity must address pages/page_metadata without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "pages.translation_resource_id_invalid",
            "Page metadata translation resource id must be a UUID",
        )
    })
}

fn summary_from_models(
    page: &PageModel,
    translations: &[PageTranslationModel],
    source_locale: &TenantLocale,
) -> Result<TranslationResourceSummary, PortError> {
    let source = translations
        .iter()
        .find(|translation| translation.locale == source_locale.as_str())
        .ok_or_else(|| {
            PortError::invariant_violation(
                "pages.translation_source_missing",
                "Page was listed without its exact source locale",
            )
        })?;
    let exact_locales = translations
        .iter()
        .map(|translation| {
            TenantLocale::new(&translation.locale).map_err(|error| {
                PortError::invariant_violation(
                    "pages.translation_locale_invalid",
                    error.to_string(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TranslationResourceSummary {
        identity: page_metadata_identity(page.id),
        display_label: source.title.clone(),
        lifecycle: page_lifecycle(page)?,
        resource_revision: opaque_positive_revision(i64::from(page.version), "resource_revision")?,
        exact_locales,
    })
}

fn snapshot_from_models(
    page: PageModel,
    translations: &[PageTranslationModel],
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let source = translations
        .iter()
        .find(|translation| translation.locale == request.source_locale.as_str())
        .ok_or_else(|| {
            PortError::not_found(
                "pages.translation_source_not_found",
                "Exact source Page metadata locale was not found",
            )
        })?;
    let target = translations
        .iter()
        .find(|translation| translation.locale == request.target_locale.as_str());
    let summary = summary_from_models(&page, translations, &request.source_locale)?;
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
        PortError::invariant_violation("pages.translation_snapshot_invalid", error.to_string())
    })?;
    Ok(snapshot)
}

fn page_lifecycle(page: &PageModel) -> Result<TranslationResourceLifecycle, PortError> {
    match page.status.as_str() {
        "draft" | "published" => Ok(TranslationResourceLifecycle::Active),
        "archived" => Ok(TranslationResourceLifecycle::Archived),
        _ => Err(PortError::invariant_violation(
            "pages.translation_page_status_invalid",
            "Page translation resource has an unsupported lifecycle status",
        )),
    }
}

fn page_metadata_identity(page_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Pages owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Pages resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(page_id.to_string())
            .expect("Pages UUID must satisfy the resource id contract"),
        subresource_id: None,
    }
}

fn translation_fields(
    source: &PageTranslationModel,
    target: Option<&PageTranslationModel>,
) -> Vec<TranslationFieldSnapshot> {
    [
        (
            "title",
            source.title.as_str(),
            target.map(|translation| translation.title.as_str()),
            TranslationValueProfile::PlainText,
            TranslationStrategy::Translate,
            true,
            true,
            None,
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
            "meta_title",
            source.meta_title.as_deref().unwrap_or_default(),
            target.and_then(|translation| translation.meta_title.as_deref()),
            TranslationValueProfile::SeoText,
            TranslationStrategy::Translate,
            false,
            true,
            None,
        ),
        (
            "meta_description",
            source.meta_description.as_deref().unwrap_or_default(),
            target.and_then(|translation| translation.meta_description.as_deref()),
            TranslationValueProfile::SeoText,
            TranslationStrategy::Translate,
            false,
            true,
            None,
        ),
    ]
    .into_iter()
    .map(
        |(key, source_value, target_value, profile, strategy, required, ai_export_allowed, max)| {
            TranslationFieldSnapshot {
                descriptor: TranslationFieldDescriptor {
                    key: FieldKey::new(key)
                        .expect("static Pages field key must satisfy the target contract"),
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
    title: String,
    slug: String,
    meta_title: Option<String>,
    meta_description: Option<String>,
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<MergedTarget, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    let title = required_target_value(values.remove("title").flatten(), "title")?;
    let slug = required_target_value(values.remove("slug").flatten(), "slug")?;
    let meta_title = values
        .remove("meta_title")
        .flatten()
        .and_then(normalize_optional_target_value);
    let meta_description = values
        .remove("meta_description")
        .flatten()
        .and_then(normalize_optional_target_value);
    Ok(MergedTarget {
        title,
        slug,
        meta_title,
        meta_description,
    })
}

fn change_from_model(change: TranslationChangeModel) -> Result<TranslationTargetChange, PortError> {
    Ok(TranslationTargetChange {
        identity: page_metadata_identity(change.resource_id),
        resource_revision: opaque_positive_revision(change.resource_revision, "resource_revision")?,
        lifecycle: parse_resource_lifecycle(&change.lifecycle)?,
    })
}

fn pages_error_to_port_error(error: PagesError) -> PortError {
    match error {
        PagesError::PageNotFound(_) => PortError::not_found(
            "pages.translation_resource_not_found",
            "Page metadata translation resource was not found",
        ),
        PagesError::DuplicateSlug { .. }
        | PagesError::VersionConflict { .. }
        | PagesError::TranslationConflict(_) => PortError::conflict(
            "pages.translation_owner_conflict",
            "Pages state conflicts with the requested translation mutation",
        ),
        PagesError::Forbidden(_) => PortError::forbidden(
            "pages.translation_permission_denied",
            "Pages permission is required",
        ),
        PagesError::Validation(_) | PagesError::CannotDeletePublished => PortError::validation(
            "pages.translation_owner_validation",
            "Pages rejected the translation mutation",
        ),
        PagesError::VersionExhausted { .. }
        | PagesError::TranslationRevisionExhausted { .. }
        | PagesError::ArtifactIntegrity(_)
        | PagesError::PublishOperationIntegrity(_)
        | PagesError::RollbackOperationIntegrity(_) => PortError::invariant_violation(
            "pages.translation_owner_invariant",
            "Pages translation state is invalid",
        ),
        PagesError::Database(_)
        | PagesError::Core(_)
        | PagesError::Content(_)
        | PagesError::Tenant(_)
        | PagesError::Rich(_)
        | PagesError::PublishRuntimeReviewInvalid(_)
        | PagesError::PublishSanitize(_)
        | PagesError::PublishRuntimeMaterializationMismatch(_)
        | PagesError::PublishIdempotencyConflict(_)
        | PagesError::RollbackIdempotencyConflict(_)
        | PagesError::RollbackTargetUnavailable(_)
        | PagesError::RollbackRequiresPublished
        | PagesError::FeatureDisabled { .. } => PortError::unavailable(
            "pages.translation_owner_unavailable",
            "Pages translation storage is unavailable",
        ),
    }
}

fn pages_database_error_to_port_error(error: sea_orm::DbErr) -> PortError {
    pages_error_to_port_error(PagesError::Database(error))
}
