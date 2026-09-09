use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, ReadTranslationResourceRequest,
    TranslationApplicationReceipt, TranslationPatchRequest, TranslationPatchValidation,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationTargetCapability, TranslationTargetChange, TranslationTargetChangePage,
    TranslationTargetChangesRequest, TranslationTargetProgressFacts, TranslationTargetProgressRequest,
    TranslationTargetProvider, TranslationTargetProviderDescriptor,
    provider_support::contract_validation_error, validate_translation_read_context,
};
use uuid::Uuid;

use crate::{
    FlexAttachedTranslationChangeLifecycle, FlexAttachedTranslationChangeOwnerPort,
    FlexAttachedTranslationError, FlexAttachedTranslationOwnerPort,
    FlexAttachedTranslationProgressOwnerPort, FlexAttachedTranslationResult,
    attached_translation_target,
};

const CHANGE_CURSOR_VERSION: &str = "v1";
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

/// Adds aggregate progress and the durable Flex ChangeCursor to one attached donor adapter.
#[derive(Clone)]
pub struct FlexAttachedTranslationProgressTargetProvider {
    inner: attached_translation_target::FlexAttachedTranslationTargetProvider,
    progress: Arc<dyn FlexAttachedTranslationProgressOwnerPort>,
    changes: Arc<dyn FlexAttachedTranslationChangeOwnerPort>,
    entity_type: String,
}

impl FlexAttachedTranslationProgressTargetProvider {
    pub fn new(
        owner: Arc<dyn FlexAttachedTranslationOwnerPort>,
        progress: Arc<dyn FlexAttachedTranslationProgressOwnerPort>,
        changes: Arc<dyn FlexAttachedTranslationChangeOwnerPort>,
    ) -> FlexAttachedTranslationResult<Self> {
        let inner = attached_translation_target::FlexAttachedTranslationTargetProvider::new(owner)?;
        let entity_type = inner.entity_type().to_string();
        if progress.entity_type() != entity_type || changes.entity_type() != entity_type {
            return Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                "attached Translation owner/progress/change adapters disagree on entity type `{entity_type}`"
            )));
        }
        Ok(Self {
            inner,
            progress,
            changes,
            entity_type,
        })
    }
}

#[async_trait]
impl TranslationTargetProvider for FlexAttachedTranslationProgressTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        let mut descriptor = self.inner.descriptor();
        descriptor
            .capabilities
            .insert(TranslationTargetCapability::AggregateProgress);
        descriptor
            .capabilities
            .insert(TranslationTargetCapability::ChangeCursor);
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
        authorize_read(&context)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;

        for _ in 0..PROGRESS_STABILITY_ATTEMPTS {
            let before = self
                .changes
                .read_change_highwater(tenant_id)
                .await
                .map_err(change_owner_error_to_port_error)?;
            let owner = self
                .progress
                .read_exact_progress(
                    tenant_id,
                    request.source_locale.as_str(),
                    request.target_locale.as_str(),
                )
                .await
                .map_err(progress_owner_error_to_port_error)?;
            let after = self
                .changes
                .read_change_highwater(tenant_id)
                .await
                .map_err(change_owner_error_to_port_error)?;
            if before != after {
                continue;
            }

            owner
                .validate()
                .map_err(progress_owner_error_to_port_error)?;
            let facts = TranslationTargetProgressFacts {
                required_units: owner.required_units,
                exact_required_units: owner.exact_required_units,
                optional_units: owner.optional_units,
                exact_optional_units: owner.exact_optional_units,
                resources: owner.resources,
                complete_resources: owner.complete_resources,
                owner_change_cursor: after
                    .map(|change_seq| change_cursor(change_seq, change_seq))
                    .transpose()?,
            };
            facts.validate().map_err(|error| {
                PortError::invariant_violation(
                    "flex.attached_translation_progress_invalid",
                    error.to_string(),
                )
            })?;
            return Ok(facts);
        }

        Err(PortError::unavailable(
            "flex.attached_translation_progress_unstable",
            "Flex attached translation progress changed while it was being aggregated",
        ))
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize_read(&context)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let parsed = request.after.as_ref().map(parse_change_cursor).transpose()?;
        let (through, after) = match parsed {
            Some((through, after)) if through == after => {
                let current = self
                    .changes
                    .read_change_highwater(tenant_id)
                    .await
                    .map_err(change_owner_error_to_port_error)?
                    .unwrap_or(after)
                    .max(after);
                (current, after)
            }
            Some(cursor) => cursor,
            None => (
                self.changes
                    .read_change_highwater(tenant_id)
                    .await
                    .map_err(change_owner_error_to_port_error)?
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
            .changes
            .read_changes(tenant_id, after, through, request.limit)
            .await
            .map_err(change_owner_error_to_port_error)?;
        let last_seq = owner_changes.last().map(|change| change.change_seq);
        let next_cursor = Some(match last_seq {
            Some(last_seq) if last_seq < through => change_cursor(through, last_seq)?,
            _ => change_cursor(through, through)?,
        });
        let changes = owner_changes
            .into_iter()
            .map(|change| {
                if change.entity_type != self.entity_type {
                    return Err(PortError::invariant_violation(
                        "flex.attached_translation_change_entity_type_invalid",
                        "Flex attached change reader returned another donor entity type",
                    ));
                }
                Ok(TranslationTargetChange {
                    identity: attached_translation_target::attached_identity(
                        &change.entity_type,
                        change.entity_id,
                    )?,
                    resource_revision: opaque_revision(change.resource_revision)?,
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
    Uuid::parse_str(&context.tenant_id)
        .ok()
        .filter(|tenant_id| !tenant_id.is_nil())
        .ok_or_else(|| {
            PortError::validation(
                "flex.invalid_tenant_id",
                "Flex attached translation progress context must carry a non-nil UUID tenant_id",
            )
        })
}

fn authorize_read(context: &PortContext) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::FlexEntries, Action::Read) == PermissionScope::None {
        return Err(PortError::forbidden(
            "flex.attached_translation_permission_denied",
            "flex_entries:read permission is required",
        ));
    }
    Ok(())
}

fn translation_change_lifecycle(
    lifecycle: FlexAttachedTranslationChangeLifecycle,
) -> TranslationResourceLifecycle {
    match lifecycle {
        FlexAttachedTranslationChangeLifecycle::Active => TranslationResourceLifecycle::Active,
        FlexAttachedTranslationChangeLifecycle::Deleted => TranslationResourceLifecycle::Deleted,
    }
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "flex.attached_translation_change_cursor_invalid",
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
            "flex.attached_translation_change_cursor_invalid",
            "Flex attached translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "flex.attached_translation_change_cursor_invalid",
            "Flex attached translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn opaque_revision(value: String) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "flex.attached_translation_change_revision_invalid",
            format!("Flex attached change resource_revision is invalid: {error}"),
        )
    })
}

fn progress_owner_error_to_port_error(error: FlexAttachedTranslationError) -> PortError {
    match error {
        FlexAttachedTranslationError::Invalid(_) => PortError::validation(
            "flex.attached_translation_progress_validation",
            "Flex rejected the attached translation progress request",
        ),
        FlexAttachedTranslationError::EntityNotFound { .. }
        | FlexAttachedTranslationError::SourceLocaleNotFound { .. }
        | FlexAttachedTranslationError::RevisionConflict { .. }
        | FlexAttachedTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "flex.attached_translation_progress_owner_invariant",
            "Flex attached translation progress owner state is invalid",
        ),
        FlexAttachedTranslationError::Operation(error) => error,
        FlexAttachedTranslationError::Storage(_) => PortError::unavailable(
            "flex.attached_translation_progress_unavailable",
            "Flex attached translation progress storage is temporarily unavailable",
        ),
    }
}

fn change_owner_error_to_port_error(error: FlexAttachedTranslationError) -> PortError {
    match error {
        FlexAttachedTranslationError::Invalid(_) => PortError::validation(
            "flex.attached_translation_change_validation",
            "Flex rejected the attached translation change request",
        ),
        FlexAttachedTranslationError::Storage(_) => PortError::unavailable(
            "flex.attached_translation_change_unavailable",
            "Flex attached translation change storage is temporarily unavailable",
        ),
        FlexAttachedTranslationError::Operation(error) => error,
        FlexAttachedTranslationError::EntityNotFound { .. }
        | FlexAttachedTranslationError::SourceLocaleNotFound { .. }
        | FlexAttachedTranslationError::RevisionConflict { .. }
        | FlexAttachedTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "flex.attached_translation_change_owner_invariant",
            "Flex attached translation change owner state is invalid",
        ),
    }
}
