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
        contract_validation_error, field_hash, merged_patch_values, read_request_from_patch,
        required_target_value, validate_patch_against_snapshot, validation_to_port_error,
    },
    register_translation_target_provider, validate_translation_apply_context,
    validate_translation_read_context,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    MarketplaceSellerError, MarketplaceSellerTranslationChangeLifecycle,
    MarketplaceSellerTranslationExactLocaleApply,
    MarketplaceSellerTranslationExactLocaleApplyReceipt,
    MarketplaceSellerTranslationExactLocaleError,
    MarketplaceSellerTranslationExactLocaleRecord,
    MarketplaceSellerTranslationExactLocaleSnapshot, MarketplaceSellerTranslationService,
};

const TRANSLATION_OWNER_SLUG: &str = "marketplace_seller";
const TRANSLATION_RESOURCE_KIND: &str = "seller_presentation";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_seller_presentation_patch";
const CHANGE_CURSOR_VERSION: &str = "v1";
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct MarketplaceSellerTranslationTargetProvider {
    service: Arc<MarketplaceSellerTranslationService>,
}

impl MarketplaceSellerTranslationTargetProvider {
    pub fn new(service: Arc<MarketplaceSellerTranslationService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Marketplace Seller owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND).expect(
                "static Marketplace Seller presentation resource kind must satisfy the target contract",
            ),
            display_name: "Marketplace Seller presentation".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["marketplace_sellers:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["marketplace_sellers:update".to_string()]),
        }
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let seller_id = parse_identity(&request.identity)?;
        let snapshot = self
            .service
            .read_exact_locale(
                tenant_id,
                seller_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await
            .map_err(seller_translation_error_to_port_error)?;
        snapshot_from_owner(snapshot, request)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Marketplace Seller translation-target failure receipt"
            );
        }
    }
}

pub fn register_marketplace_seller_translation_target_provider(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<(), TranslationTargetRegistryError> {
    register_translation_target_provider(
        extensions,
        MarketplaceSellerTranslationTargetProvider::new(Arc::new(
            MarketplaceSellerTranslationService::new(db),
        )),
    )
}

#[async_trait]
impl TranslationTargetProvider for MarketplaceSellerTranslationTargetProvider {
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
                        "marketplace_seller.translation_cursor_invalid",
                        "Marketplace Seller translation cursor must be a Seller UUID",
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
            .map_err(seller_translation_error_to_port_error)?;
        let resources = snapshots
            .into_iter()
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = next_after
            .map(|seller_id| {
                OpaqueCursor::new(seller_id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "marketplace_seller.translation_cursor_invalid",
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
        let actor_user_id = authorize(&context, Action::Update)?.user_id;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let seller_id = parse_identity(&request.identity)?;
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
            let target_display_name = merged_target_display_name(&request, &snapshot)?;
            let applied = self
                .service
                .apply_exact_locale_with_operation(
                    tenant_id,
                    actor_user_id,
                    seller_id,
                    MarketplaceSellerTranslationExactLocaleApply {
                        source_locale: request.source_locale.as_str().to_string(),
                        target_locale: request.target_locale.as_str().to_string(),
                        display_name: target_display_name,
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
                .map_err(seller_translation_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "marketplace_seller.translation_receipt_identity_invalid",
                    "Marketplace Seller owner receipt is not bound to the active translation operation",
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
                .seller_translation_change_highwater(tenant_id)
                .await
                .map_err(seller_error_to_port_error)?;
            let owner = self
                .service
                .read_exact_progress(
                    tenant_id,
                    request.source_locale.as_str(),
                    request.target_locale.as_str(),
                )
                .await
                .map_err(seller_translation_error_to_port_error)?;
            let after = self
                .service
                .seller_translation_change_highwater(tenant_id)
                .await
                .map_err(seller_error_to_port_error)?;
            if before != after {
                continue;
            }

            let facts = TranslationTargetProgressFacts {
                required_units: owner.resources,
                exact_required_units: owner.exact_required_units,
                optional_units: 0,
                exact_optional_units: 0,
                resources: owner.resources,
                complete_resources: owner.complete_resources,
                owner_change_cursor: after
                    .map(|change_seq| change_cursor(change_seq, change_seq))
                    .transpose()?,
            };
            facts.validate().map_err(|error| {
                PortError::invariant_violation(
                    "marketplace_seller.translation_progress_invalid",
                    error.to_string(),
                )
            })?;
            return Ok(facts);
        }

        Err(PortError::unavailable(
            "marketplace_seller.translation_progress_unstable",
            "Marketplace Seller translation progress changed while it was being aggregated",
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
                    .seller_translation_change_highwater(tenant_id)
                    .await
                    .map_err(seller_error_to_port_error)?
                    .unwrap_or(after)
                    .max(after);
                (current, after)
            }
            Some(cursor) => cursor,
            None => (
                self.service
                    .seller_translation_change_highwater(tenant_id)
                    .await
                    .map_err(seller_error_to_port_error)?
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
            .read_seller_translation_changes(tenant_id, after, through, request.limit)
            .await
            .map_err(seller_error_to_port_error)?;
        let last_seq = owner_changes.last().map(|change| change.change_seq);
        let next_cursor = Some(match last_seq {
            Some(last_seq) if last_seq < through => change_cursor(through, last_seq)?,
            _ => change_cursor(through, through)?,
        });
        let changes = owner_changes
            .into_iter()
            .map(|change| {
                Ok(TranslationTargetChange {
                    identity: seller_identity(change.seller_id),
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
            "marketplace_seller.invalid_tenant_id",
            "Marketplace Seller translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::MarketplaceSellers, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "marketplace_seller.translation_permission_denied",
            format!("marketplace_sellers:{action} permission is required"),
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
            "marketplace_seller.translation_identity_invalid",
            "Marketplace Seller translation identity must address marketplace_seller/seller_presentation without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_seller.translation_resource_id_invalid",
            "Marketplace Seller translation resource id must be a UUID",
        )
    })
}

fn seller_identity(seller_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Marketplace Seller owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND).expect(
            "static Marketplace Seller presentation resource kind must satisfy the target contract",
        ),
        resource_id: ResourceId::new(seller_id.to_string())
            .expect("Seller UUID must satisfy the resource id contract"),
        subresource_id: None,
    }
}

fn summary_from_owner(
    snapshot: MarketplaceSellerTranslationExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: seller_identity(snapshot.seller_id),
        display_label: snapshot.source.display_name,
        lifecycle: lifecycle_from_status(&snapshot.status)?,
        resource_revision: opaque_revision(snapshot.resource_revision, "resource_revision")?,
        exact_locales: exact_locales(snapshot.exact_locales)?,
    })
}

fn snapshot_from_owner(
    snapshot: MarketplaceSellerTranslationExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let summary = TranslationResourceSummary {
        identity: seller_identity(snapshot.seller_id),
        display_label: snapshot.source.display_name.clone(),
        lifecycle: lifecycle_from_status(&snapshot.status)?,
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
            "marketplace_seller.translation_snapshot_invalid",
            error.to_string(),
        )
    })?;
    Ok(resource)
}

fn exact_locales(locales: Vec<String>) -> Result<Vec<TenantLocale>, PortError> {
    locales
        .into_iter()
        .map(|locale| {
            TenantLocale::new(locale).map_err(|error| {
                PortError::invariant_violation(
                    "marketplace_seller.translation_locale_invalid",
                    error.to_string(),
                )
            })
        })
        .collect()
}

fn translation_fields(
    source: &MarketplaceSellerTranslationExactLocaleRecord,
    target: Option<&MarketplaceSellerTranslationExactLocaleRecord>,
) -> Vec<TranslationFieldSnapshot> {
    vec![TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key: FieldKey::new("display_name")
                .expect("static Marketplace Seller field key must satisfy the target contract"),
            profile: TranslationValueProfile::PlainText,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::Public,
            required: true,
            ai_export_allowed: true,
            max_characters: Some(160),
            preserves_whitespace: false,
        },
        source_value: source.display_name.clone(),
        exact_target_value: target.map(|value| value.display_name.clone()),
        source_hash: field_hash(&source.display_name),
        protected_tokens: Vec::new(),
    }]
}

fn merged_target_display_name(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<String, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    required_target_value(values.remove("display_name").flatten(), "display_name")
}

fn decode_owner_receipt(
    value: serde_json::Value,
) -> Result<MarketplaceSellerTranslationExactLocaleApplyReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

fn application_receipt(
    owner_receipt: &MarketplaceSellerTranslationExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
) -> Result<TranslationApplicationReceipt, PortError> {
    let operation_id = owner_receipt.operation_id.ok_or_else(|| {
        PortError::invariant_violation(
            "marketplace_seller.translation_receipt_identity_invalid",
            "Marketplace Seller owner receipt does not carry a translation operation id",
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

fn lifecycle_from_status(status: &str) -> Result<TranslationResourceLifecycle, PortError> {
    match status {
        "draft" | "active" | "suspended" => Ok(TranslationResourceLifecycle::Active),
        "closed" => Ok(TranslationResourceLifecycle::Archived),
        _ => Err(PortError::invariant_violation(
            "marketplace_seller.translation_lifecycle_invalid",
            format!("Marketplace Seller has unknown lifecycle status `{status}`"),
        )),
    }
}

fn translation_change_lifecycle(
    lifecycle: MarketplaceSellerTranslationChangeLifecycle,
) -> TranslationResourceLifecycle {
    match lifecycle {
        MarketplaceSellerTranslationChangeLifecycle::Active => TranslationResourceLifecycle::Active,
        MarketplaceSellerTranslationChangeLifecycle::Archived => {
            TranslationResourceLifecycle::Archived
        }
        MarketplaceSellerTranslationChangeLifecycle::Deleted => {
            TranslationResourceLifecycle::Deleted
        }
    }
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "marketplace_seller.translation_change_cursor_invalid",
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
            "marketplace_seller.translation_change_cursor_invalid",
            "Marketplace Seller translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "marketplace_seller.translation_change_cursor_invalid",
            "Marketplace Seller translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "marketplace_seller.translation_revision_invalid",
            format!("Marketplace Seller {field} is invalid: {error}"),
        )
    })
}

fn seller_translation_error_to_port_error(
    error: MarketplaceSellerTranslationExactLocaleError,
) -> PortError {
    match error {
        MarketplaceSellerTranslationExactLocaleError::SellerNotFound(_) => PortError::not_found(
            "marketplace_seller.translation_resource_not_found",
            "Marketplace Seller translation resource was not found",
        ),
        MarketplaceSellerTranslationExactLocaleError::SourceLocaleNotFound { .. } => {
            PortError::not_found(
                "marketplace_seller.translation_source_not_found",
                "Exact source Marketplace Seller locale was not found",
            )
        }
        MarketplaceSellerTranslationExactLocaleError::TargetLocaleMissingAfterApply { .. } => {
            PortError::invariant_violation(
                "marketplace_seller.translation_owner_invariant",
                "Marketplace Seller exact target locale is missing after owner apply",
            )
        }
        MarketplaceSellerTranslationExactLocaleError::RevisionConflict { .. } => {
            PortError::conflict(
                "marketplace_seller.translation_revision_conflict",
                "Marketplace Seller translation state conflicts with the request",
            )
        }
        MarketplaceSellerTranslationExactLocaleError::Validation(_) => PortError::validation(
            "marketplace_seller.translation_owner_validation",
            "Marketplace Seller rejected the translation request",
        ),
        MarketplaceSellerTranslationExactLocaleError::OperationReceipt(error) => error,
        MarketplaceSellerTranslationExactLocaleError::Database(_) => PortError::unavailable(
            "marketplace_seller.translation_owner_unavailable",
            "Marketplace Seller translation storage is temporarily unavailable",
        ),
    }
}

fn seller_error_to_port_error(error: MarketplaceSellerError) -> PortError {
    match error {
        MarketplaceSellerError::Database(_) => PortError::unavailable(
            "marketplace_seller.translation_owner_unavailable",
            "Marketplace Seller translation storage is temporarily unavailable",
        ),
        MarketplaceSellerError::SellerNotFound(_)
        | MarketplaceSellerError::MemberNotFound(_)
        | MarketplaceSellerError::MembershipNotFound { .. } => PortError::not_found(
            "marketplace_seller.translation_resource_not_found",
            "Marketplace Seller resource was not found",
        ),
        MarketplaceSellerError::IdempotencyConflict(_)
        | MarketplaceSellerError::DuplicateHandle(_)
        | MarketplaceSellerError::DuplicateMembership { .. }
        | MarketplaceSellerError::InvalidTransition { .. } => PortError::conflict(
            "marketplace_seller.translation_owner_conflict",
            "Marketplace Seller state conflicts with the request",
        ),
        MarketplaceSellerError::CommandReceiptCorrupt(_) => PortError::invariant_violation(
            "marketplace_seller.translation_owner_invariant",
            "Marketplace Seller owner receipt is invalid",
        ),
        MarketplaceSellerError::Validation(_) => PortError::validation(
            "marketplace_seller.translation_owner_validation",
            "Marketplace Seller rejected the translation request",
        ),
    }
}
