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
    TranslationTargetProgressFacts, TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationTargetRegistryError, TranslationValueProfile,
    provider_support::{
        contract_validation_error, field_hash, merged_patch_values, read_request_from_patch,
        required_target_value, validate_patch_against_snapshot, validation_to_port_error,
    },
    register_translation_target_provider, validate_translation_apply_context,
    validate_translation_read_context,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ShippingOptionTranslationExactLocaleApply, ShippingOptionTranslationExactLocaleApplyReceipt,
    ShippingOptionTranslationExactLocaleError, ShippingOptionTranslationExactLocaleRecord,
    ShippingOptionTranslationExactLocaleSnapshot, ShippingOptionTranslationService,
};

const TRANSLATION_OWNER_SLUG: &str = "fulfillment";
const TRANSLATION_RESOURCE_KIND: &str = "shipping_option_copy";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_shipping_option_copy_patch";

#[derive(Clone)]
pub struct ShippingOptionTranslationTargetProvider {
    service: Arc<ShippingOptionTranslationService>,
}

impl ShippingOptionTranslationTargetProvider {
    pub fn new(service: Arc<ShippingOptionTranslationService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Fulfillment owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Shipping Option Copy resource kind must satisfy the target contract"),
            display_name: "Shipping Options".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
            ]),
            read_permission_floor: BTreeSet::from(["fulfillments:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["fulfillments:update".to_string()]),
        }
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let shipping_option_id = parse_identity(&request.identity)?;
        let snapshot = self
            .service
            .read_exact_locale(
                tenant_id,
                shipping_option_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(fulfillment_translation_error_to_port_error)?;
        snapshot_from_owner(snapshot, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Fulfillment translation-target failure receipt"
            );
        }
    }
}

pub fn register_shipping_option_translation_target_provider(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<(), TranslationTargetRegistryError> {
    register_translation_target_provider(
        extensions,
        ShippingOptionTranslationTargetProvider::new(Arc::new(
            ShippingOptionTranslationService::new(db),
        )),
    )
}

#[async_trait]
impl TranslationTargetProvider for ShippingOptionTranslationTargetProvider {
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
                        "fulfillment.translation_cursor_invalid",
                        "Fulfillment translation cursor must be a Shipping Option UUID",
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
            .map_err(fulfillment_translation_error_to_port_error)?;
        let resources = snapshots
            .into_iter()
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = next_after
            .map(|shipping_option_id| {
                OpaqueCursor::new(shipping_option_id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "fulfillment.translation_cursor_invalid",
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
        let shipping_option_id = parse_identity(&request.identity)?;
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
            let target_name = merged_target(&request, &snapshot)?;
            let applied = self
                .service
                .apply_exact_locale_with_operation(
                    tenant_id,
                    shipping_option_id,
                    ShippingOptionTranslationExactLocaleApply {
                        source_locale: request.source_locale.as_str().to_string(),
                        target_locale: request.target_locale.as_str().to_string(),
                        name: target_name,
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
                .map_err(fulfillment_translation_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "fulfillment.translation_receipt_identity_invalid",
                    "Fulfillment owner receipt is not bound to the active translation operation",
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
        let owner = self
            .service
            .read_exact_progress(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(fulfillment_translation_error_to_port_error)?;
        let facts = TranslationTargetProgressFacts {
            required_units: owner.resources,
            exact_required_units: owner.exact_required_units,
            optional_units: 0,
            exact_optional_units: 0,
            resources: owner.resources,
            complete_resources: owner.complete_resources,
            owner_change_cursor: None,
        };
        facts.validate().map_err(|error| {
            PortError::invariant_violation(
                "fulfillment.translation_progress_invalid",
                error.to_string(),
            )
        })?;
        Ok(facts)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "fulfillment.invalid_tenant_id",
            "Fulfillment translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Fulfillments, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "fulfillment.translation_permission_denied",
            format!("fulfillments:{action} permission is required"),
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
            "fulfillment.translation_identity_invalid",
            "Fulfillment translation identity must address fulfillment/shipping_option_copy without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "fulfillment.translation_resource_id_invalid",
            "Fulfillment translation resource id must be a UUID",
        )
    })
}

fn shipping_option_identity(shipping_option_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Fulfillment owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Shipping Option Copy resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(shipping_option_id.to_string())
            .expect("Shipping Option UUID must satisfy the resource id contract"),
        subresource_id: None,
    }
}

fn summary_from_owner(
    snapshot: ShippingOptionTranslationExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: shipping_option_identity(snapshot.shipping_option_id),
        display_label: snapshot.source.name,
        lifecycle: translation_lifecycle(snapshot.active),
        resource_revision: opaque_revision(snapshot.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(snapshot.exact_locales)?,
    })
}

fn snapshot_from_owner(
    snapshot: ShippingOptionTranslationExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let summary = TranslationResourceSummary {
        identity: shipping_option_identity(snapshot.shipping_option_id),
        display_label: snapshot.source.name.clone(),
        lifecycle: translation_lifecycle(snapshot.active),
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
        PortError::invariant_violation(
            "fulfillment.translation_snapshot_invalid",
            error.to_string(),
        )
    })?;
    Ok(resource)
}

fn translation_lifecycle(active: bool) -> TranslationResourceLifecycle {
    if active {
        TranslationResourceLifecycle::Active
    } else {
        TranslationResourceLifecycle::Archived
    }
}

fn exact_locales(locales: Vec<String>) -> Result<Vec<TenantLocale>, PortError> {
    locales
        .into_iter()
        .map(|locale| {
            TenantLocale::new(locale).map_err(|error| {
                PortError::invariant_violation(
                    "fulfillment.translation_locale_invalid",
                    error.to_string(),
                )
            })
        })
        .collect()
}

fn translation_fields(
    source: &ShippingOptionTranslationExactLocaleRecord,
    target: Option<&ShippingOptionTranslationExactLocaleRecord>,
) -> Vec<TranslationFieldSnapshot> {
    vec![TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key: FieldKey::new("name")
                .expect("static Fulfillment field key must satisfy the target contract"),
            profile: TranslationValueProfile::PlainText,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::Public,
            required: true,
            ai_export_allowed: true,
            max_characters: Some(120),
            preserves_whitespace: false,
        },
        source_value: source.name.clone(),
        exact_target_value: target.map(|value| value.name.clone()),
        source_hash: field_hash(&source.name),
        protected_tokens: Vec::new(),
    }]
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<String, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    required_target_value(values.remove("name").flatten(), "name")
}

fn decode_owner_receipt(
    value: serde_json::Value,
) -> Result<ShippingOptionTranslationExactLocaleApplyReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

fn application_receipt(
    owner_receipt: &ShippingOptionTranslationExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
) -> Result<TranslationApplicationReceipt, PortError> {
    let operation_id = owner_receipt.operation_id.ok_or_else(|| {
        PortError::invariant_violation(
            "fulfillment.translation_receipt_identity_invalid",
            "Fulfillment owner receipt does not carry a translation operation id",
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

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "fulfillment.translation_revision_invalid",
            format!("Fulfillment {field} is invalid: {error}"),
        )
    })
}

fn fulfillment_translation_error_to_port_error(
    error: ShippingOptionTranslationExactLocaleError,
) -> PortError {
    match error {
        ShippingOptionTranslationExactLocaleError::SourceLocaleNotFound { .. } => {
            PortError::not_found(
                "fulfillment.translation_source_not_found",
                "Exact source Shipping Option locale was not found",
            )
        }
        ShippingOptionTranslationExactLocaleError::TargetLocaleMissingAfterApply { .. } => {
            PortError::invariant_violation(
                "fulfillment.translation_owner_invariant",
                "Shipping Option exact target locale is missing after owner apply",
            )
        }
        ShippingOptionTranslationExactLocaleError::RevisionConflict { .. } => {
            PortError::conflict(
                "fulfillment.translation_revision_conflict",
                "Fulfillment translation state conflicts with the request",
            )
        }
        ShippingOptionTranslationExactLocaleError::ShippingOptionNotFound(_) => {
            PortError::not_found(
                "fulfillment.translation_resource_not_found",
                "Shipping Option translation resource was not found",
            )
        }
        ShippingOptionTranslationExactLocaleError::Validation(_) => PortError::validation(
            "fulfillment.translation_owner_validation",
            "Fulfillment rejected the translation request",
        ),
        ShippingOptionTranslationExactLocaleError::OperationReceipt(error) => error,
        ShippingOptionTranslationExactLocaleError::Database(_) => PortError::unavailable(
            "fulfillment.translation_owner_unavailable",
            "Fulfillment translation storage is temporarily unavailable",
        ),
    }
}
