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
    ApplyExactTaxonomyTranslationInput, TaxonomyError, TaxonomyService,
    entities::{
        taxonomy_term::{Column as TermColumn, Entity as TermEntity, Model as TermModel},
        taxonomy_term_translation::{
            Column as TranslationColumn, Entity as TranslationEntity, Model as TranslationModel,
        },
        translation_change::{
            Column as ChangeColumn, Entity as ChangeEntity, Model as ChangeModel,
        },
    },
    translation_evidence::{
        TRANSLATION_OWNER_SLUG, TRANSLATION_RESOURCE_KIND, TranslationChangeEvidence,
        record_translation_change_in_tx,
    },
};

const OPERATION_APPLY_PATCH: &str = "translation_target_apply_patch";
const REQUIRED_FIELD_COUNT: u64 = 2;
const OPTIONAL_FIELD_COUNT: u64 = 1;
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
/// Owner adapter for exact taxonomy term localization. It calls the canonical
/// Taxonomy service and never exposes owner tables to Translation directly.
pub struct TaxonomyTranslationTargetProvider {
    service: Arc<TaxonomyService>,
}

impl TaxonomyTranslationTargetProvider {
    pub fn new(service: Arc<TaxonomyService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static resource kind must satisfy the target contract"),
            display_name: "Taxonomy term".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["taxonomy:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["taxonomy:update".to_string()]),
        }
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let term_id = parse_identity(&request.identity)?;
        let term = TermEntity::find_by_id(term_id)
            .filter(TermColumn::TenantId.eq(tenant_id))
            .one(self.service.database())
            .await
            .map_err(taxonomy_database_error_to_port_error)?
            .ok_or_else(|| {
                PortError::not_found(
                    "taxonomy.translation_resource_not_found",
                    format!("taxonomy translation resource not found: {term_id}"),
                )
            })?;
        let translations = TranslationEntity::find()
            .filter(TranslationColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::TermId.eq(term_id))
            .order_by_asc(TranslationColumn::Locale)
            .all(self.service.database())
            .await
            .map_err(taxonomy_database_error_to_port_error)?;
        snapshot_from_models(term, translations, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Taxonomy translation-target failure receipt"
            );
        }
    }

    async fn latest_change_cursor(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<OpaqueCursor>, PortError> {
        ChangeEntity::find()
            .filter(ChangeColumn::TenantId.eq(tenant_id))
            .order_by_desc(ChangeColumn::Id)
            .one(self.service.database())
            .await
            .map_err(taxonomy_database_error_to_port_error)?
            .map(|change| {
                OpaqueCursor::new(change.id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "taxonomy.translation_change_cursor_invalid",
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
        let terms = TermEntity::find()
            .inner_join(TranslationEntity)
            .filter(TermColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::Locale.eq(request.source_locale.as_str()))
            .order_by_asc(TermColumn::Id)
            .all(self.service.database())
            .await
            .map_err(taxonomy_database_error_to_port_error)?;
        let term_ids = terms.iter().map(|term| term.id).collect::<Vec<_>>();
        let targets = if term_ids.is_empty() {
            Vec::new()
        } else {
            TranslationEntity::find()
                .filter(TranslationColumn::TenantId.eq(tenant_id))
                .filter(TranslationColumn::TermId.is_in(term_ids))
                .filter(TranslationColumn::Locale.eq(request.target_locale.as_str()))
                .all(self.service.database())
                .await
                .map_err(taxonomy_database_error_to_port_error)?
        };
        let targets = targets
            .into_iter()
            .map(|translation| (translation.term_id, translation))
            .collect::<BTreeMap<_, _>>();
        let resources = u64::try_from(terms.len()).map_err(|_| {
            PortError::invariant_violation(
                "taxonomy.translation_progress_overflow",
                "taxonomy resource count exceeds the progress contract",
            )
        })?;
        let required_units = resources.checked_mul(REQUIRED_FIELD_COUNT).ok_or_else(|| {
            PortError::invariant_violation(
                "taxonomy.translation_progress_overflow",
                "taxonomy required progress count overflow",
            )
        })?;
        let optional_units = resources.checked_mul(OPTIONAL_FIELD_COUNT).ok_or_else(|| {
            PortError::invariant_violation(
                "taxonomy.translation_progress_overflow",
                "taxonomy optional progress count overflow",
            )
        })?;

        let mut exact_required_units = 0_u64;
        let mut exact_optional_units = 0_u64;
        let mut complete_resources = 0_u64;
        for term in terms {
            let Some(target) = targets.get(&term.id) else {
                continue;
            };
            let has_name = !target.name.trim().is_empty();
            let has_slug = !target.slug.trim().is_empty();
            exact_required_units = exact_required_units
                .checked_add(u64::from(has_name) + u64::from(has_slug))
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "taxonomy.translation_progress_overflow",
                        "taxonomy exact required progress count overflow",
                    )
                })?;
            if target
                .description
                .as_deref()
                .is_some_and(|description| !description.trim().is_empty())
            {
                exact_optional_units = exact_optional_units.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "taxonomy.translation_progress_overflow",
                        "taxonomy exact optional progress count overflow",
                    )
                })?;
            }
            if has_name && has_slug {
                complete_resources = complete_resources.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "taxonomy.translation_progress_overflow",
                        "taxonomy complete resource count overflow",
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
impl TranslationTargetProvider for TaxonomyTranslationTargetProvider {
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
                        "taxonomy.translation_cursor_invalid",
                        "taxonomy translation cursor must be a term UUID",
                    )
                })
            })
            .transpose()?;
        let mut query = TermEntity::find()
            .inner_join(TranslationEntity)
            .filter(TermColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::Locale.eq(request.source_locale.as_str()))
            .order_by_asc(TermColumn::Id);
        if let Some(after) = after {
            query = query.filter(TermColumn::Id.gt(after));
        }
        let mut terms = query
            .limit(u64::from(request.limit) + 1)
            .all(self.service.database())
            .await
            .map_err(taxonomy_database_error_to_port_error)?;
        let has_more = terms.len() > usize::from(request.limit);
        if has_more {
            terms.truncate(usize::from(request.limit));
        }
        let term_ids = terms.iter().map(|term| term.id).collect::<Vec<_>>();
        let translations = if term_ids.is_empty() {
            Vec::new()
        } else {
            TranslationEntity::find()
                .filter(TranslationColumn::TenantId.eq(tenant_id))
                .filter(TranslationColumn::TermId.is_in(term_ids))
                .order_by_asc(TranslationColumn::TermId)
                .order_by_asc(TranslationColumn::Locale)
                .all(self.service.database())
                .await
                .map_err(taxonomy_database_error_to_port_error)?
        };
        let mut translations_by_term = BTreeMap::<Uuid, Vec<TranslationModel>>::new();
        for translation in translations {
            translations_by_term
                .entry(translation.term_id)
                .or_default()
                .push(translation);
        }
        let resources = terms
            .iter()
            .map(|term| {
                summary_from_models(
                    term,
                    translations_by_term
                        .get(&term.id)
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                    &request.source_locale,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = has_more.then(|| terms.last()).flatten().map(|term| {
            OpaqueCursor::new(term.id.to_string())
                .expect("taxonomy UUID cursor must satisfy the opaque cursor contract")
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
        authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let term_id = parse_identity(&request.identity)?;
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
                .map_err(taxonomy_database_error_to_port_error)?;
            let applied = self
                .service
                .apply_exact_translation_in_tx(
                    &transaction,
                    tenant_id,
                    term_id,
                    ApplyExactTaxonomyTranslationInput {
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
                    },
                )
                .await
                .map_err(taxonomy_error_to_port_error)?;
            let receipt = TranslationApplicationReceipt {
                provider_receipt_id: format!("taxonomy:{}", lease.operation_id),
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
            record_translation_change_in_tx(
                &transaction,
                TranslationChangeEvidence {
                    tenant_id,
                    term_id,
                    locale: request.target_locale.as_str(),
                    resource_revision: applied.resource_revision,
                    target_revision: applied.target_revision,
                    operation: "upsert",
                },
            )
            .await
            .map_err(taxonomy_error_to_port_error)?;
            idempotency::complete(&transaction, lease, &receipt).await?;
            transaction
                .commit()
                .await
                .map_err(taxonomy_database_error_to_port_error)?;
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
                        "taxonomy.translation_progress_invalid",
                        error.to_string(),
                    )
                })?;
                return Ok(facts);
            }
        }

        Err(PortError::unavailable(
            "taxonomy.translation_progress_unstable",
            "taxonomy translation progress changed while it was being aggregated",
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
                        "taxonomy.translation_change_cursor_invalid",
                        "taxonomy translation change cursor must be a change UUID",
                    )
                })
            })
            .transpose()?;
        let mut query = ChangeEntity::find()
            .filter(ChangeColumn::TenantId.eq(tenant_id))
            .order_by_asc(ChangeColumn::Id);
        if let Some(after) = after {
            query = query.filter(ChangeColumn::Id.gt(after));
        }
        let rows = query
            .limit(u64::from(request.limit))
            .all(self.service.database())
            .await
            .map_err(taxonomy_database_error_to_port_error)?;
        let next_cursor = rows.last().map(|change| {
            OpaqueCursor::new(change.id.to_string())
                .expect("taxonomy change UUID must satisfy the opaque cursor contract")
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
            "taxonomy.invalid_tenant_id",
            "taxonomy translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Taxonomy, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "taxonomy.translation_permission_denied",
            format!("taxonomy:{action} permission is required"),
        ));
    }
    Ok(())
}

fn parse_identity(identity: &TranslationResourceIdentity) -> Result<Uuid, PortError> {
    if identity.owner_slug.as_str() != TRANSLATION_OWNER_SLUG
        || identity.resource_kind.as_str() != TRANSLATION_RESOURCE_KIND
        || identity.subresource_id.is_some()
    {
        return Err(PortError::validation(
            "taxonomy.translation_identity_invalid",
            "taxonomy translation identity must address taxonomy/term without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "taxonomy.translation_resource_id_invalid",
            "taxonomy translation resource id must be a UUID",
        )
    })
}

fn summary_from_models(
    term: &TermModel,
    translations: &[TranslationModel],
    source_locale: &TenantLocale,
) -> Result<TranslationResourceSummary, PortError> {
    let source = translations
        .iter()
        .find(|translation| translation.locale == source_locale.as_str())
        .ok_or_else(|| {
            PortError::invariant_violation(
                "taxonomy.translation_source_missing",
                format!(
                    "taxonomy term {} was listed without its exact source locale",
                    term.id
                ),
            )
        })?;
    let exact_locales = translations
        .iter()
        .filter_map(|translation| TenantLocale::new(&translation.locale).ok())
        .collect::<Vec<_>>();
    Ok(TranslationResourceSummary {
        identity: taxonomy_identity(term.id),
        display_label: source.name.clone(),
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: opaque_positive_revision(term.revision, "resource_revision")?,
        exact_locales,
    })
}

fn snapshot_from_models(
    term: TermModel,
    translations: Vec<TranslationModel>,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let source = translations
        .iter()
        .find(|translation| translation.locale == request.source_locale.as_str())
        .ok_or_else(|| {
            PortError::not_found(
                "taxonomy.translation_source_not_found",
                format!(
                    "exact source translation {} was not found for taxonomy term {}",
                    request.source_locale, term.id
                ),
            )
        })?;
    let target = translations
        .iter()
        .find(|translation| translation.locale == request.target_locale.as_str());
    let summary = summary_from_models(&term, &translations, &request.source_locale)?;
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
        PortError::invariant_violation("taxonomy.translation_snapshot_invalid", error.to_string())
    })?;
    Ok(snapshot)
}

fn taxonomy_identity(term_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(term_id.to_string())
            .expect("taxonomy UUID must satisfy the resource id contract"),
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
            TranslationValueProfile::PlainText,
            TranslationStrategy::Translate,
            true,
            true,
            Some(120),
        ),
        (
            "slug",
            source.slug.as_str(),
            target.map(|translation| translation.slug.as_str()),
            TranslationValueProfile::Slug,
            TranslationStrategy::TransliterateWithReview,
            true,
            false,
            Some(120),
        ),
        (
            "description",
            source.description.as_deref().unwrap_or_default(),
            target.and_then(|translation| translation.description.as_deref()),
            TranslationValueProfile::PlainText,
            TranslationStrategy::Translate,
            false,
            true,
            Some(2_000),
        ),
    ]
    .into_iter()
    .map(
        |(key, source_value, target_value, profile, strategy, required, ai_export_allowed, max)| {
            TranslationFieldSnapshot {
                descriptor: TranslationFieldDescriptor {
                    key: FieldKey::new(key).expect("static field key must satisfy the contract"),
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

fn change_from_model(change: ChangeModel) -> Result<TranslationTargetChange, PortError> {
    let lifecycle = parse_resource_lifecycle(&change.lifecycle)?;
    Ok(TranslationTargetChange {
        identity: taxonomy_identity(change.term_id),
        resource_revision: opaque_positive_revision(change.resource_revision, "resource_revision")?,
        lifecycle,
    })
}

fn taxonomy_error_to_port_error(error: TaxonomyError) -> PortError {
    match error {
        TaxonomyError::TermNotFound(term_id) => PortError::not_found(
            "taxonomy.translation_resource_not_found",
            format!("taxonomy translation resource not found: {term_id}"),
        ),
        TaxonomyError::DuplicateCanonicalKey(_)
        | TaxonomyError::DuplicateSlug(_)
        | TaxonomyError::DuplicateAlias(_)
        | TaxonomyError::Conflict(_) => PortError::conflict(
            "taxonomy.translation_owner_conflict",
            "Taxonomy state conflicts with the requested translation mutation",
        ),
        TaxonomyError::Forbidden(_) => PortError::forbidden(
            "taxonomy.translation_permission_denied",
            "taxonomy permission is required",
        ),
        TaxonomyError::Validation(_) => PortError::validation(
            "taxonomy.translation_owner_validation",
            "Taxonomy rejected the translation mutation",
        ),
        TaxonomyError::TranslationRevisionExhausted { .. } => PortError::invariant_violation(
            "taxonomy.translation_revision_exhausted",
            "Taxonomy translation revision is exhausted",
        ),
        TaxonomyError::Database(_) => PortError::unavailable(
            "taxonomy.translation_database",
            "Taxonomy storage is unavailable",
        ),
    }
}

fn taxonomy_database_error_to_port_error(error: sea_orm::DbErr) -> PortError {
    taxonomy_error_to_port_error(TaxonomyError::Database(error))
}
