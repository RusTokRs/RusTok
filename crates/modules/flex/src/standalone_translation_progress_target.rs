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
    FlexStandaloneTranslationChangeLifecycle, FlexStandaloneTranslationChangeOwnerPort,
    FlexStandaloneTranslationError, FlexStandaloneTranslationOwnerPort,
    FlexStandaloneTranslationProgressOwnerPort, FlexStandaloneTranslationResult,
    standalone_translation_target,
};

const CHANGE_CURSOR_VERSION: &str = "v1";
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct FlexStandaloneTranslationProgressTargetProvider {
    inner: standalone_translation_target::FlexStandaloneTranslationTargetProvider,
    progress: Arc<dyn FlexStandaloneTranslationProgressOwnerPort>,
    changes: Arc<dyn FlexStandaloneTranslationChangeOwnerPort>,
}

impl FlexStandaloneTranslationProgressTargetProvider {
    pub fn new(
        owner: Arc<dyn FlexStandaloneTranslationOwnerPort>,
        progress: Arc<dyn FlexStandaloneTranslationProgressOwnerPort>,
        changes: Arc<dyn FlexStandaloneTranslationChangeOwnerPort>,
    ) -> FlexStandaloneTranslationResult<Self> {
        Ok(Self {
            inner: standalone_translation_target::FlexStandaloneTranslationTargetProvider::new(owner),
            progress,
            changes,
        })
    }
}

#[async_trait]
impl TranslationTargetProvider for FlexStandaloneTranslationProgressTargetProvider {
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
            owner.validate().map_err(progress_owner_error_to_port_error)?;
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
                    "flex.standalone_translation_progress_invalid",
                    error.to_string(),
                )
            })?;
            return Ok(facts);
        }

        Err(PortError::unavailable(
            "flex.standalone_translation_progress_unstable",
            "Flex standalone translation progress changed while it was being aggregated",
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
                Ok(TranslationTargetChange {
                    identity: standalone_translation_target::standalone_identity(
                        change.schema_id,
                        change.entry_id,
                    )?,
                    resource_revision: opaque_revision(change.resource_revision)?,
                    lifecycle: match change.lifecycle {
                        FlexStandaloneTranslationChangeLifecycle::Active => {
                            TranslationResourceLifecycle::Active
                        }
                        FlexStandaloneTranslationChangeLifecycle::Archived => {
                            TranslationResourceLifecycle::Archived
                        }
                        FlexStandaloneTranslationChangeLifecycle::Deleted => {
                            TranslationResourceLifecycle::Deleted
                        }
                    },
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
                "Flex standalone translation progress context must carry a non-nil UUID tenant_id",
            )
        })
}

fn authorize_read(context: &PortContext) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::FlexEntries, Action::Read) == PermissionScope::None {
        return Err(PortError::forbidden(
            "flex.standalone_translation_permission_denied",
            "flex_entries:read permission is required",
        ));
    }
    Ok(())
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "flex.standalone_translation_change_cursor_invalid",
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
            "flex.standalone_translation_change_cursor_invalid",
            "Flex standalone translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "flex.standalone_translation_change_cursor_invalid",
            "Flex standalone translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn opaque_revision(value: String) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "flex.standalone_translation_change_revision_invalid",
            format!("Flex standalone change resource_revision is invalid: {error}"),
        )
    })
}

fn progress_owner_error_to_port_error(error: FlexStandaloneTranslationError) -> PortError {
    match error {
        FlexStandaloneTranslationError::Invalid(_) => PortError::validation(
            "flex.standalone_translation_progress_validation",
            "Flex rejected the standalone translation progress request",
        ),
        FlexStandaloneTranslationError::Storage(_) => PortError::unavailable(
            "flex.standalone_translation_progress_unavailable",
            "Flex standalone translation progress storage is temporarily unavailable",
        ),
        FlexStandaloneTranslationError::Operation(error) => error,
        _ => PortError::invariant_violation(
            "flex.standalone_translation_progress_owner_invariant",
            "Flex standalone translation progress owner state is invalid",
        ),
    }
}

fn change_owner_error_to_port_error(error: FlexStandaloneTranslationError) -> PortError {
    match error {
        FlexStandaloneTranslationError::Invalid(_) => PortError::validation(
            "flex.standalone_translation_change_validation",
            "Flex rejected the standalone translation change request",
        ),
        FlexStandaloneTranslationError::Storage(_) => PortError::unavailable(
            "flex.standalone_translation_change_unavailable",
            "Flex standalone translation change storage is temporarily unavailable",
        ),
        FlexStandaloneTranslationError::Operation(error) => error,
        _ => PortError::invariant_violation(
            "flex.standalone_translation_change_owner_invariant",
            "Flex standalone translation change owner state is invalid",
        ),
    }
}
