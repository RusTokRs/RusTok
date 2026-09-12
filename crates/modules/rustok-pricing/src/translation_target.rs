use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{ModuleRuntimeExtensions, PermissionScope, SecurityContext};
use rustok_outbox::idempotency::{self, Admission};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetChange, TranslationTargetChangePage, TranslationTargetChangesRequest,
    TranslationTargetProgressFacts, TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationTargetRegistryError, TranslationValueProfile,
    provider_support::{
        contract_validation_error, field_hash, merged_patch_values, normalize_optional_target_value,
        read_request_from_patch, required_target_value, validate_patch_against_snapshot,
        validation_to_port_error,
    },
    register_translation_target_provider, validate_translation_apply_context,
    validate_translation_read_context,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    PriceListTranslationChangeLifecycle, PriceListTranslationExactLocaleApply,
    PriceListTranslationExactLocaleApplyReceipt, PriceListTranslationExactLocaleError,
    PriceListTranslationExactLocaleRecord, PriceListTranslationExactLocaleSnapshot,
    PriceListTranslationService,
};

const TRANSLATION_OWNER_SLUG: &str = "pricing";
const TRANSLATION_RESOURCE_KIND: &str = "price_list_copy";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_price_list_copy_patch";
const CHANGE_CURSOR_VERSION: &str = "v1";
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct PriceListTranslationTargetProvider {
    service: Arc<PriceListTranslationService>,
}

impl PriceListTranslationTargetProvider {
    pub fn new(service: Arc<PriceListTranslationService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Pricing owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Price List Copy resource kind must satisfy the target contract"),
            display_name: "Price Lists".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["discounts:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["discounts:update".to_string()]),
        }
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let price_list_id = parse_identity(&request.identity)?;
        let snapshot = self
            .service
            .read_exact_locale(
                tenant_id,
                price_list_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(pricing_translation_error_to_port_error)?;
        snapshot_from_owner(snapshot, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Pricing translation-target failure receipt"
            );
        }
    }
}

pub fn register_price_list_translation_target_provider(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<(), TranslationTargetRegistryError> {
    register_translation_target_provider(
        extensions,
        PriceListTranslationTargetProvider::new(Arc::new(PriceListTranslationService::new(db))),
    )
}

#[async_trait]
impl TranslationTargetProvider for PriceListTranslationTargetProvider {
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
                        "pricing.translation_cursor_invalid",
                        "Pricing translation cursor must be a Price List UUID",
                    )
                })
            })
            .transpose()?;
        let (snapshots, next_after) = self
            .service
            .list_exact_resources(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
                after,
                request.limit,
            )
            .await
            .map_err(pricing_translation_error_to_port_error)?;
        let resources = snapshots
            .into_iter()
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = next_after
            .map(|price_list_id| {
                OpaqueCursor::new(price_list_id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "pricing.translation_cursor_invalid",
                        error.to_string(),
                    )
                })
            })
            .transpose()?;
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
        let price_list_id = parse_identity(&request.identity)?;
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
            Admission::Replay(value) => {
                let owner_receipt = decode_owner_receipt(value)?;
                return application_receipt(&owner_receipt, &request);
            }
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
            let (target_name, target_description) = merged_target(&request, &snapshot)?;
            let applied = self
                .service
                .apply_exact_locale_with_operation(
                    tenant_id,
                    price_list_id,
                    PriceListTranslationExactLocaleApply {
                        source_locale: request.source_locale.as_str().to_string(),
                        target_locale: request.target_locale.as_str().to_string(),
                        name: target_name,
                        description: target_description,
                        expected_resource_revision: request
                            .expected_resource_revision
                            .as_str()
                            .to_string(),
                        expected_source_revision: request
                            .expected_source_revision
                            .as_str()
                            .to_string(),
                        expected_target_revision: request
                            .expected_target_revision
                            .as_ref()
                            .map(|revision| revision.as_str().to_string()),
                    },
                    lease,
                )
                .await
                .map_err(pricing_translation_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "pricing.translation_receipt_identity_invalid",
                    "Pricing owner receipt is not bound to the active translation operation",
                ));
            }
            application_receipt(&applied, &request)
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
            let before = self
                .service
                .price_list_translation_change_highwater(tenant_id)
                .await
                .map_err(pricing_translation_error_to_port_error)?;
            let owner = self
                .service
                .read_exact_progress(
                    tenant_id,
                    request.source_locale.as_str(),
                    request.target_locale.as_str(),
                )
                .await
                .map_err(pricing_translation_error_to_port_error)?;
            let after = self
                .service
                .price_list_translation_change_highwater(tenant_id)
                .await
                .map_err(pricing_translation_error_to_port_error)?;
            if before != after {
                continue;
            }

            let facts = TranslationTargetProgressFacts {
                required_units: owner.resources,
                exact_required_units: owner.exact_required_units,
                optional_units: owner.resources,
                exact_optional_units: owner.exact_optional_units,
                resources: owner.resources,
                complete_resources: owner.complete_resources,
                owner_change_cursor: after
                    .map(|change_seq| change_cursor(change_seq, change_seq))
                    .transpose()?,
            };
            facts.validate().map_err(|error| {
                PortError::invariant_violation(
                    "pricing.translation_progress_invalid",
                    error.to_string(),
                )
            })?;
            return Ok(facts);
        }

        Err(PortError::unavailable(
            "pricing.translation_progress_unstable",
            "Pricing translation progress changed while it was being aggregated",
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
        let parsed = request
            .after
            .as_ref()
            .map(parse_change_cursor)
            .transpose()?;
        let (through, after) = match parsed {
            Some((through, after)) if through == after => {
                let current = self
                    .service
                    .price_list_translation_change_highwater(tenant_id)
                    .await
                    .map_err(pricing_translation_error_to_port_error)?
                    .unwrap_or(after)
                    .max(after);
                (current, after)
            }
            Some(cursor) => cursor,
            None => (
                self.service
                    .price_list_translation_change_highwater(tenant_id)
                    .await
                    .map_err(pricing_translation_error_to_port_error)?
                    .unwrap_or(0),
                0,
            ),
        };
        if through == 0 {
            return Ok(TranslationTargetChangePage {
                changes: Vec::new(),
                next_cursor: None,
            });
        }

        let owner_changes = self
            .service
            .read_price_list_translation_changes(tenant_id, after, through, request.limit)
            .await
            .map_err(pricing_translation_error_to_port_error)?;
        let last_seq = owner_changes.last().map(|change| change.change_seq);
        let next_cursor = Some(match last_seq {
            Some(last_seq) if last_seq < through => change_cursor(through, last_seq)?,
            _ => change_cursor(through, through)?,
        });
        let changes = owner_changes
            .into_iter()
            .map(|change| {
                Ok(TranslationTargetChange {
                    identity: price_list_identity(change.price_list_id),
                    resource_revision: opaque_revision(
                        change.resource_revision,
                        "resource_revision",
                    )?,
                    lifecycle: translation_change_lifecycle(change.lifecycle),
                })
            })
            .collect::<Result<Vec<_>, PortError>>()?;

        Ok(TranslationTargetChangePage {
            changes,
            next_cursor,
        })
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "pricing.invalid_tenant_id",
            "Pricing translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Discounts, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "pricing.translation_permission_denied",
            format!("discounts:{action} permission is required"),
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
            "pricing.translation_identity_invalid",
            "Pricing translation identity must address pricing/price_list_copy without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "pricing.translation_resource_id_invalid",
            "Pricing translation resource id must be a UUID",
        )
    })
}

fn price_list_identity(price_list_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Pricing owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Price List Copy resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(price_list_id.to_string())
            .expect("Price List UUID must satisfy the resource id contract"),
        subresource_id: None,
    }
}

fn summary_from_owner(
    snapshot: PriceListTranslationExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: price_list_identity(snapshot.price_list_id),
        display_label: snapshot.source.name,
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: opaque_revision(snapshot.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(snapshot.exact_locales)?,
    })
}

fn snapshot_from_owner(
    snapshot: PriceListTranslationExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let summary = TranslationResourceSummary {
        identity: price_list_identity(snapshot.price_list_id),
        display_label: snapshot.source.name.clone(),
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: opaque_revision(snapshot.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(snapshot.exact_locales)?,
    };
    let resource = TranslationResourceSnapshot {
        summary,
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
        rendered_fallback_locale: None,
        source_revision: opaque_revision(snapshot.source_revision, "source_revision")?,
        target_revision: snapshot
            .target_revision
            .map(|revision| opaque_revision(revision, "target_revision"))
            .transpose()?,
        fields: translation_fields(&snapshot.source, snapshot.target.as_ref()),
    };
    resource.validate().map_err(|error| {
        PortError::invariant_violation("pricing.translation_snapshot_invalid", error.to_string())
    })?;
    Ok(resource)
}

fn exact_locales(locales: Vec<String>) -> Result<Vec<TenantLocale>, PortError> {
    locales
        .into_iter()
        .map(|locale| {
            TenantLocale::new(locale).map_err(|error| {
                PortError::invariant_violation(
                    "pricing.translation_locale_invalid",
                    error.to_string(),
                )
            })
        })
        .collect()
}

fn translation_fields(
    source: &PriceListTranslationExactLocaleRecord,
    target: Option<&PriceListTranslationExactLocaleRecord>,
) -> Vec<TranslationFieldSnapshot> {
    [
        (
            "name",
            source.name.as_str(),
            target.map(|value| value.name.as_str()),
            true,
            Some(100),
        ),
        (
            "description",
            source.description.as_deref().unwrap_or_default(),
            target.and_then(|value| value.description.as_deref()),
            false,
            None,
        ),
    ]
    .into_iter()
    .map(|(key, source_value, target_value, required, max_characters)| {
        TranslationFieldSnapshot {
            descriptor: TranslationFieldDescriptor {
                key: FieldKey::new(key)
                    .expect("static Pricing field key must satisfy the target contract"),
                profile: TranslationValueProfile::PlainText,
                strategy: TranslationStrategy::Translate,
                classification: TranslationDataClassification::Public,
                required,
                ai_export_allowed: true,
                max_characters,
                preserves_whitespace: false,
            },
            source_value: source_value.to_string(),
            exact_target_value: target_value.map(str::to_string),
            source_hash: field_hash(source_value),
            protected_tokens: Vec::new(),
        }
    })
    .collect()
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<(String, Option<String>), PortError> {
    let mut values = merged_patch_values(request, snapshot);
    let name = required_target_value(values.remove("name").flatten(), "name")?;
    let description = values
        .remove("description")
        .flatten()
        .and_then(normalize_optional_target_value);
    Ok((name, description))
}

fn decode_owner_receipt(
    value: serde_json::Value,
) -> Result<PriceListTranslationExactLocaleApplyReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

fn application_receipt(
    owner_receipt: &PriceListTranslationExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
) -> Result<TranslationApplicationReceipt, PortError> {
    let operation_id = owner_receipt.operation_id.ok_or_else(|| {
        PortError::invariant_violation(
            "pricing.translation_receipt_identity_invalid",
            "Pricing owner receipt does not carry a translation operation id",
        )
    })?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: operation_id.to_string(),
        resource_revision: opaque_revision(
            owner_receipt.resource_revision.clone(),
            "resource_revision",
        )?,
        target_revision: opaque_revision(owner_receipt.target_revision.clone(), "target_revision")?,
        applied_field_keys: request
            .fields
            .iter()
            .map(|field| field.key.clone())
            .collect(),
    })
}

fn translation_change_lifecycle(
    lifecycle: PriceListTranslationChangeLifecycle,
) -> TranslationResourceLifecycle {
    match lifecycle {
        PriceListTranslationChangeLifecycle::Active => TranslationResourceLifecycle::Active,
        PriceListTranslationChangeLifecycle::Deleted => TranslationResourceLifecycle::Deleted,
    }
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "pricing.translation_change_cursor_invalid",
            error.to_string(),
        )
    })
}

fn parse_change_cursor(cursor: &OpaqueCursor) -> Result<(u64, u64), PortError> {
    let mut parts = cursor.as_str().split(':');
    let version = parts.next();
    let through = parts.next().and_then(|value| value.parse::<u64>().ok());
    let after = parts.next().and_then(|value| value.parse::<u64>().ok());
    if version != Some(CHANGE_CURSOR_VERSION)
        || parts.next().is_some()
        || through.is_none()
        || after.is_none()
    {
        return Err(PortError::validation(
            "pricing.translation_change_cursor_invalid",
            "Pricing translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "pricing.translation_change_cursor_invalid",
            "Pricing translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "pricing.translation_revision_invalid",
            format!("Pricing {field} is invalid: {error}"),
        )
    })
}

fn pricing_translation_error_to_port_error(error: PriceListTranslationExactLocaleError) -> PortError {
    match error {
        PriceListTranslationExactLocaleError::SourceLocaleNotFound { .. } => PortError::not_found(
            "pricing.translation_source_not_found",
            "Exact source Price List locale was not found",
        ),
        PriceListTranslationExactLocaleError::TargetLocaleMissingAfterApply { .. } => {
            PortError::invariant_violation(
                "pricing.translation_owner_invariant",
                "Price List exact target locale is missing after owner apply",
            )
        }
        PriceListTranslationExactLocaleError::RevisionConflict { .. } => PortError::conflict(
            "pricing.translation_revision_conflict",
            "Pricing translation state conflicts with the request",
        ),
        PriceListTranslationExactLocaleError::PriceListNotFound(_) => PortError::not_found(
            "pricing.translation_resource_not_found",
            "Price List translation resource was not found",
        ),
        PriceListTranslationExactLocaleError::Validation(_) => PortError::validation(
            "pricing.translation_owner_validation",
            "Pricing rejected the translation request",
        ),
        PriceListTranslationExactLocaleError::OperationReceipt(error) => error,
        PriceListTranslationExactLocaleError::Database(_) => PortError::unavailable(
            "pricing.translation_owner_unavailable",
            "Pricing translation storage is temporarily unavailable",
        ),
    }
}
