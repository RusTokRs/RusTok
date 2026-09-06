use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use async_trait::async_trait;
use rustok_api::{Action, PortActorKind, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::idempotency::{self, Admission};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldPatch,
    TranslationFieldSnapshot, TranslationPatchIssue, TranslationPatchIssueSeverity,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetChange, TranslationTargetChangePage, TranslationTargetChangesRequest,
    TranslationTargetProgressFacts, TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationValueProfile,
    validate_translation_apply_context, validate_translation_read_context,
};
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
    Select, TransactionTrait, sea_query::SelectStatement,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ApplyExactMediaTranslationInput, MediaError, MediaService, NormalizedTranslationInput,
    entities::{
        asset::{Column as AssetColumn, Entity as AssetEntity, Model as AssetModel},
        media_translation::{
            Column as TranslationColumn, Entity as TranslationEntity, Model as TranslationModel,
        },
        translation_change::{
            Column as TranslationChangeColumn, Entity as TranslationChangeEntity,
            Model as TranslationChangeModel,
        },
    },
    lifecycle::AssetState,
    ports::media_error_to_port_error,
    service::media_resource_revision,
    translation_evidence::{
        TRANSLATION_OWNER_SLUG, TRANSLATION_RESOURCE_KIND, TranslationChangeEvidence,
        record_translation_change_in_transaction,
    },
};

const OPERATION_APPLY_PATCH: &str = "translation_target_apply_patch";
const TRANSLATABLE_FIELD_COUNT: u64 = 3;
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
/// Owner adapter that exposes Media localized metadata through the neutral
/// translation-target SPI without granting direct access to Media tables.
pub struct MediaTranslationTargetProvider {
    service: Arc<MediaService>,
}

impl MediaTranslationTargetProvider {
    /// Builds the provider from the canonical Media service.
    pub fn new(service: Arc<MediaService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static owner slug must be valid"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static resource kind must be valid"),
            display_name: "Media asset metadata".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["media:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["media:update".to_string()]),
        }
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let media_id = parse_identity(&request.identity)?;
        let asset = AssetEntity::find_by_id(media_id)
            .filter(AssetColumn::TenantId.eq(tenant_id))
            .filter(AssetColumn::LifecycleState.eq(AssetState::Active.as_str()))
            .one(self.service.database())
            .await
            .map_err(|error| media_error_to_port_error(MediaError::Db(error)))?
            .ok_or_else(|| {
                PortError::not_found(
                    "media.translation_resource_not_found",
                    format!("media translation resource not found: {media_id}"),
                )
            })?;
        let translations = TranslationEntity::find()
            .filter(TranslationColumn::TenantId.eq(tenant_id))
            .filter(TranslationColumn::AssetId.eq(media_id))
            .order_by_asc(TranslationColumn::Locale)
            .all(self.service.database())
            .await
            .map_err(|error| media_error_to_port_error(MediaError::Db(error)))?;
        snapshot_from_models(asset, translations, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Media translation-target failure receipt"
            );
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for MediaTranslationTargetProvider {
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
                        "media.translation_cursor_invalid",
                        "media translation cursor must be a resource UUID",
                    )
                })
            })
            .transpose()?;
        let mut query = AssetEntity::find()
            .filter(AssetColumn::TenantId.eq(tenant_id))
            .filter(AssetColumn::LifecycleState.eq(AssetState::Active.as_str()))
            .order_by_asc(AssetColumn::Id);
        if let Some(after) = after {
            query = query.filter(AssetColumn::Id.gt(after));
        }
        let mut assets = query
            .limit(u64::from(request.limit) + 1)
            .all(self.service.database())
            .await
            .map_err(|error| media_error_to_port_error(MediaError::Db(error)))?;
        let has_more = assets.len() > usize::from(request.limit);
        if has_more {
            assets.truncate(usize::from(request.limit));
        }

        let asset_ids = assets.iter().map(|asset| asset.id).collect::<Vec<_>>();
        let translations = if asset_ids.is_empty() {
            Vec::new()
        } else {
            TranslationEntity::find()
                .filter(TranslationColumn::TenantId.eq(tenant_id))
                .filter(TranslationColumn::AssetId.is_in(asset_ids))
                .order_by_asc(TranslationColumn::AssetId)
                .order_by_asc(TranslationColumn::Locale)
                .all(self.service.database())
                .await
                .map_err(|error| media_error_to_port_error(MediaError::Db(error)))?
        };
        let mut translations_by_asset = BTreeMap::<Uuid, Vec<TranslationModel>>::new();
        for translation in translations {
            translations_by_asset
                .entry(translation.asset_id)
                .or_default()
                .push(translation);
        }

        let resources = assets
            .iter()
            .map(|asset| {
                summary_from_models(
                    asset,
                    translations_by_asset
                        .get(&asset.id)
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                    &request.source_locale,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = has_more.then(|| assets.last()).flatten().map(|asset| {
            OpaqueCursor::new(asset.id.to_string())
                .expect("media UUID cursor must satisfy the opaque cursor contract")
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
        let media_id = parse_identity(&request.identity)?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        // The patch, tenant, operation kind, and idempotency key define the
        // durable owner mutation. Authorization is re-evaluated above for
        // every caller, while actor-neutral request hashing lets an explicitly
        // authorized Translation recovery operator reconcile an unknown
        // outcome without issuing a second mutation identity.
        let admission_request = &request;
        let lease = match idempotency::admit(
            self.service.database(),
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            TRANSLATION_OWNER_SLUG,
            idempotency_key,
            OPERATION_APPLY_PATCH,
            admission_request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => return decode_receipt(value),
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
            let expected_source_revision = parse_numeric_revision(
                &request.expected_source_revision,
                "expected_source_revision",
            )?;
            let expected_target_revision = request
                .expected_target_revision
                .as_ref()
                .map(|revision| parse_numeric_revision(revision, "expected_target_revision"))
                .transpose()?;
            let transaction = self
                .service
                .database()
                .begin()
                .await
                .map_err(|error| media_error_to_port_error(MediaError::Db(error)))?;
            let applied = self
                .service
                .apply_exact_translation_in_transaction(
                    &transaction,
                    tenant_id,
                    media_id,
                    ApplyExactMediaTranslationInput {
                        source_locale: request.source_locale.clone(),
                        target,
                        expected_resource_revision: request
                            .expected_resource_revision
                            .as_str()
                            .to_string(),
                        expected_source_revision,
                        expected_target_revision,
                    },
                )
                .await
                .map_err(media_error_to_port_error)?;
            let receipt = TranslationApplicationReceipt {
                provider_receipt_id: format!("media:{}", lease.operation_id),
                resource_revision: request.expected_resource_revision.clone(),
                target_revision: OpaqueRevision::new(applied.revision.to_string())
                    .expect("positive media revision must be a valid opaque revision"),
                applied_field_keys: request
                    .fields
                    .iter()
                    .map(|field| field.key.clone())
                    .collect(),
            };
            record_translation_change_in_transaction(
                &transaction,
                self.service.translation_event_bus(),
                TranslationChangeEvidence {
                    tenant_id,
                    media_id,
                    locale: request.target_locale.as_str(),
                    resource_revision: receipt.resource_revision.as_str(),
                    target_revision: applied.revision,
                    operation: "upsert",
                    lifecycle: "active",
                    actor_id: event_actor_id(&context)?,
                    correlation_id: context.correlation_id.clone(),
                },
            )
            .await
            .map_err(media_error_to_port_error)?;
            idempotency::complete(&transaction, lease, &receipt).await?;
            transaction
                .commit()
                .await
                .map_err(|error| media_error_to_port_error(MediaError::Db(error)))?;
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
                        "media.translation_progress_invalid",
                        error.to_string(),
                    )
                })?;
                return Ok(facts);
            }
        }

        Err(PortError::unavailable(
            "media.translation_progress_unstable",
            "media translation progress changed while it was being aggregated",
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
                        "media.translation_change_cursor_invalid",
                        "media translation change cursor must be a change UUID",
                    )
                })
            })
            .transpose()?;
        let mut query = TranslationChangeEntity::find()
            .filter(TranslationChangeColumn::TenantId.eq(tenant_id))
            .order_by_asc(TranslationChangeColumn::Id);
        if let Some(after) = after {
            query = query.filter(TranslationChangeColumn::Id.gt(after));
        }
        let rows = query
            .limit(u64::from(request.limit))
            .all(self.service.database())
            .await
            .map_err(|error| media_error_to_port_error(MediaError::Db(error)))?;
        let next_cursor = rows.last().map(|change| {
            OpaqueCursor::new(change.id.to_string())
                .expect("media change UUID must satisfy the opaque cursor contract")
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

impl MediaTranslationTargetProvider {
    async fn latest_change_cursor(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<OpaqueCursor>, PortError> {
        TranslationChangeEntity::find()
            .filter(TranslationChangeColumn::TenantId.eq(tenant_id))
            .order_by_desc(TranslationChangeColumn::Id)
            .one(self.service.database())
            .await
            .map_err(|error| media_error_to_port_error(MediaError::Db(error)))?
            .map(|change| {
                OpaqueCursor::new(change.id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "media.translation_change_cursor_invalid",
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
        let resources = eligible_source_translations(tenant_id, request.source_locale.as_str())
            .count(self.service.database())
            .await
            .map_err(|error| media_error_to_port_error(MediaError::Db(error)))?;
        let optional_units = resources
            .checked_mul(TRANSLATABLE_FIELD_COUNT)
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "media.translation_progress_overflow",
                    "media translation progress count overflow",
                )
            })?;

        let mut exact_optional_units = 0_u64;
        for column in [
            TranslationColumn::Title,
            TranslationColumn::AltText,
            TranslationColumn::Caption,
        ] {
            exact_optional_units = exact_optional_units
                .checked_add(
                    exact_target_field_count(
                        self.service.database(),
                        tenant_id,
                        request.source_locale.as_str(),
                        request.target_locale.as_str(),
                        column,
                    )
                    .await?,
                )
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "media.translation_progress_overflow",
                        "media translation progress count overflow",
                    )
                })?;
        }

        Ok(TranslationTargetProgressFacts {
            required_units: 0,
            exact_required_units: 0,
            optional_units,
            exact_optional_units,
            resources,
            // Media metadata fields are optional by owner contract, so no
            // optional absence can make a source-eligible resource incomplete.
            complete_resources: resources,
            owner_change_cursor: None,
        })
    }
}

fn eligible_source_translations(tenant_id: Uuid, source_locale: &str) -> Select<TranslationEntity> {
    TranslationEntity::find()
        .inner_join(AssetEntity)
        .filter(TranslationColumn::TenantId.eq(tenant_id))
        .filter(TranslationColumn::Locale.eq(source_locale))
        .filter(AssetColumn::TenantId.eq(tenant_id))
        .filter(AssetColumn::LifecycleState.eq(AssetState::Active.as_str()))
}

fn eligible_source_asset_ids(tenant_id: Uuid, source_locale: &str) -> SelectStatement {
    eligible_source_translations(tenant_id, source_locale)
        .select_only()
        .column(TranslationColumn::AssetId)
        .into_query()
}

async fn exact_target_field_count(
    database: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    source_locale: &str,
    target_locale: &str,
    column: TranslationColumn,
) -> Result<u64, PortError> {
    TranslationEntity::find()
        .filter(TranslationColumn::TenantId.eq(tenant_id))
        .filter(TranslationColumn::Locale.eq(target_locale))
        .filter(
            TranslationColumn::AssetId
                .in_subquery(eligible_source_asset_ids(tenant_id, source_locale)),
        )
        .filter(column.is_not_null())
        .count(database)
        .await
        .map_err(|error| media_error_to_port_error(MediaError::Db(error)))
}

fn change_from_model(change: TranslationChangeModel) -> Result<TranslationTargetChange, PortError> {
    let lifecycle = match change.lifecycle.as_str() {
        "active" => TranslationResourceLifecycle::Active,
        "archived" => TranslationResourceLifecycle::Archived,
        "deleted" => TranslationResourceLifecycle::Deleted,
        "unavailable" => TranslationResourceLifecycle::Unavailable,
        value => {
            return Err(PortError::invariant_violation(
                "media.translation_change_lifecycle_invalid",
                format!("invalid persisted Media translation lifecycle: {value}"),
            ));
        }
    };
    Ok(TranslationTargetChange {
        identity: media_identity(change.asset_id),
        resource_revision: OpaqueRevision::new(change.resource_revision).map_err(|error| {
            PortError::invariant_violation(
                "media.translation_change_revision_invalid",
                error.to_string(),
            )
        })?,
        lifecycle,
    })
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "media.invalid_tenant_id",
            "media translation target context must carry a UUID tenant_id",
        )
    })
}

fn event_actor_id(context: &PortContext) -> Result<Option<Uuid>, PortError> {
    if context.actor.kind == PortActorKind::System {
        return Ok(None);
    }
    Uuid::parse_str(&context.actor.id).map(Some).map_err(|_| {
        PortError::validation(
            "media.translation_actor_id_invalid",
            "Media translation event actor id must be a UUID",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Media, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "media.translation_permission_denied",
            format!("media:{action} permission is required"),
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
            "media.translation_identity_invalid",
            "media translation identity must address media/asset without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "media.translation_resource_id_invalid",
            "media translation resource id must be a UUID",
        )
    })
}

fn summary_from_models(
    asset: &AssetModel,
    translations: &[TranslationModel],
    source_locale: &TenantLocale,
) -> Result<TranslationResourceSummary, PortError> {
    let exact_locales = translations
        .iter()
        .filter_map(|translation| TenantLocale::new(&translation.locale).ok())
        .collect::<Vec<_>>();
    let display_label = translations
        .iter()
        .find(|translation| translation.locale == source_locale.as_str())
        .and_then(|translation| translation.title.as_deref())
        .filter(|title| !title.is_empty())
        .unwrap_or(asset.original_name.as_str())
        .to_string();
    Ok(TranslationResourceSummary {
        identity: media_identity(asset.id),
        display_label,
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: opaque_resource_revision(asset)?,
        exact_locales,
    })
}

fn snapshot_from_models(
    asset: AssetModel,
    translations: Vec<TranslationModel>,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let source = translations
        .iter()
        .find(|translation| translation.locale == request.source_locale.as_str())
        .ok_or_else(|| {
            PortError::not_found(
                "media.translation_source_not_found",
                format!(
                    "exact source translation {} was not found for media {}",
                    request.source_locale, asset.id
                ),
            )
        })?;
    let target = translations
        .iter()
        .find(|translation| translation.locale == request.target_locale.as_str());
    let summary = summary_from_models(&asset, &translations, &request.source_locale)?;
    let snapshot = TranslationResourceSnapshot {
        summary,
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
        rendered_fallback_locale: None,
        source_revision: opaque_numeric_revision(source.revision)?,
        target_revision: target
            .map(|translation| opaque_numeric_revision(translation.revision))
            .transpose()?,
        fields: translation_fields(source, target),
    };
    snapshot.validate().map_err(|error| {
        PortError::invariant_violation("media.translation_snapshot_invalid", error.to_string())
    })?;
    Ok(snapshot)
}

fn media_identity(media_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static owner slug must be valid"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static resource kind must be valid"),
        resource_id: ResourceId::new(media_id.to_string())
            .expect("media UUID must be a valid resource id"),
        subresource_id: None,
    }
}

fn opaque_resource_revision(asset: &AssetModel) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(media_resource_revision(asset)).map_err(|error| {
        PortError::invariant_violation("media.resource_revision_invalid", error.to_string())
    })
}

fn opaque_numeric_revision(revision: i64) -> Result<OpaqueRevision, PortError> {
    if revision <= 0 {
        return Err(PortError::invariant_violation(
            "media.translation_revision_invalid",
            "persisted media translation revision must be positive",
        ));
    }
    OpaqueRevision::new(revision.to_string()).map_err(|error| {
        PortError::invariant_violation("media.translation_revision_invalid", error.to_string())
    })
}

fn translation_fields(
    source: &TranslationModel,
    target: Option<&TranslationModel>,
) -> Vec<TranslationFieldSnapshot> {
    [
        (
            "title",
            source.title.as_deref(),
            target.and_then(|row| row.title.as_deref()),
        ),
        (
            "alt_text",
            source.alt_text.as_deref(),
            target.and_then(|row| row.alt_text.as_deref()),
        ),
        (
            "caption",
            source.caption.as_deref(),
            target.and_then(|row| row.caption.as_deref()),
        ),
    ]
    .into_iter()
    .map(|(key, source_value, target_value)| {
        let source_value = source_value.unwrap_or_default().to_string();
        TranslationFieldSnapshot {
            descriptor: TranslationFieldDescriptor {
                key: FieldKey::new(key).expect("static field key must be valid"),
                profile: TranslationValueProfile::PlainText,
                strategy: TranslationStrategy::Translate,
                classification: TranslationDataClassification::Public,
                required: false,
                ai_export_allowed: true,
                max_characters: None,
                preserves_whitespace: false,
            },
            source_hash: field_hash(&source_value),
            source_value,
            exact_target_value: target_value.map(str::to_string),
            protected_tokens: Vec::new(),
        }
    })
    .collect()
}

fn field_hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn read_request_from_patch(request: &TranslationPatchRequest) -> ReadTranslationResourceRequest {
    ReadTranslationResourceRequest {
        identity: request.identity.clone(),
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
    }
}

fn validate_patch_against_snapshot(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> TranslationPatchValidation {
    let mut issues = Vec::new();
    if request.expected_resource_revision != snapshot.summary.resource_revision {
        issues.push(conflict_issue(None, "resource_revision_conflict"));
    }
    if request.expected_source_revision != snapshot.source_revision {
        issues.push(conflict_issue(None, "source_revision_conflict"));
    }
    if request.expected_target_revision != snapshot.target_revision {
        issues.push(conflict_issue(None, "target_revision_conflict"));
    }
    let fields = snapshot
        .fields
        .iter()
        .map(|field| (field.descriptor.key.as_str(), field))
        .collect::<BTreeMap<_, _>>();
    for patch in &request.fields {
        match fields.get(patch.key.as_str()) {
            Some(field) if field.source_hash != patch.expected_source_hash => {
                issues.push(conflict_issue(
                    Some(patch.key.clone()),
                    "source_hash_conflict",
                ));
            }
            Some(_) => {}
            None => issues.push(TranslationPatchIssue {
                field: Some(patch.key.clone()),
                severity: TranslationPatchIssueSeverity::Error,
                code: "field_not_supported".to_string(),
                message: "field is not exposed by the Media translation target".to_string(),
            }),
        }
    }
    TranslationPatchValidation {
        accepted: issues.is_empty(),
        issues,
    }
}

fn conflict_issue(field: Option<FieldKey>, code: &str) -> TranslationPatchIssue {
    TranslationPatchIssue {
        field,
        severity: TranslationPatchIssueSeverity::Error,
        code: code.to_string(),
        message: "live Media translation state no longer matches the proposal".to_string(),
    }
}

fn validation_to_port_error(validation: &TranslationPatchValidation) -> PortError {
    let conflict = validation
        .issues
        .iter()
        .any(|issue| issue.code.ends_with("_conflict"));
    let codes = validation
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>()
        .join(",");
    if conflict {
        PortError::conflict(
            "media.translation_patch_conflict",
            format!("media translation patch conflicts with live state: {codes}"),
        )
    } else {
        PortError::validation(
            "media.translation_patch_invalid",
            format!("media translation patch is invalid: {codes}"),
        )
    }
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<NormalizedTranslationInput, PortError> {
    let mut values = snapshot
        .fields
        .iter()
        .map(|field| {
            (
                field.descriptor.key.as_str(),
                field.exact_target_value.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for TranslationFieldPatch { key, value, .. } in &request.fields {
        if let Some(existing) = values.get_mut(key.as_str()) {
            *existing = Some(value.clone());
        }
    }
    Ok(NormalizedTranslationInput {
        locale: request.target_locale.clone(),
        title: normalize_optional(values.remove("title").flatten()),
        alt_text: normalize_optional(values.remove("alt_text").flatten()),
        caption: normalize_optional(values.remove("caption").flatten()),
    })
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_numeric_revision(
    revision: &OpaqueRevision,
    field: &'static str,
) -> Result<i64, PortError> {
    revision
        .as_str()
        .parse::<i64>()
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| {
            PortError::validation(
                "media.translation_revision_invalid",
                format!("{field} must be a positive Media translation revision"),
            )
        })
}

fn contract_validation_error(message: String) -> PortError {
    PortError::validation("media.translation_contract_invalid", message)
}

fn decode_receipt(value: serde_json::Value) -> Result<TranslationApplicationReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}
