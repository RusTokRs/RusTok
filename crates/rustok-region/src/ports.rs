use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::RegionResponse;

const REGION_OWNER: &str = "rustok_region";
const REGION_READ_PORT_BOUNDARY: &str = "region_read_owner_port";

/// Transport-neutral selector for region read-projection consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionReadSelector {
    Id(Uuid),
    CountryCode(String),
}

/// Transport-neutral request for region read-projection consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionReadRequest {
    pub selector: RegionReadSelector,
    pub requested_locale: Option<String>,
    pub tenant_default_locale: Option<String>,
}

/// Transport-neutral request for region list consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionListRequest {
    pub requested_locale: Option<String>,
    pub tenant_default_locale: Option<String>,
}

/// Transport-neutral region projection exposed by the region owner module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionReadProjection {
    pub region: RegionResponse,
}

/// Transport-neutral owner boundary for region read projections.
#[async_trait]
pub trait RegionReadPort: Send + Sync {
    async fn read_region(
        &self,
        context: PortContext,
        request: RegionReadRequest,
    ) -> Result<Option<RegionReadProjection>, PortError>;

    async fn list_regions_for_tenant(
        &self,
        context: PortContext,
        request: RegionListRequest,
    ) -> Result<Vec<RegionReadProjection>, PortError>;
}

#[async_trait]
impl RegionReadPort for crate::RegionService {
    async fn read_region(
        &self,
        context: PortContext,
        request: RegionReadRequest,
    ) -> Result<Option<RegionReadProjection>, PortError> {
        let owner_operation = "read_region";
        require_region_read_policy(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        validate_region_read_request(&context, owner_operation, &request)?;

        let result = match request.selector {
            RegionReadSelector::Id(region_id) => self
                .get_region(
                    tenant_id,
                    region_id,
                    request.requested_locale.as_deref(),
                    request.tenant_default_locale.as_deref(),
                )
                .await
                .map(Some),
            RegionReadSelector::CountryCode(country_code) => {
                self.resolve_region_for_country(
                    tenant_id,
                    &country_code,
                    request.requested_locale.as_deref(),
                    request.tenant_default_locale.as_deref(),
                )
                .await
            }
        }
        .map_err(|error| map_region_error(&context, owner_operation, error))?;

        Ok(result.map(|region| RegionReadProjection { region }))
    }

    async fn list_regions_for_tenant(
        &self,
        context: PortContext,
        request: RegionListRequest,
    ) -> Result<Vec<RegionReadProjection>, PortError> {
        let owner_operation = "list_regions_for_tenant";
        require_region_read_policy(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        self.list_regions(
            tenant_id,
            request.requested_locale.as_deref(),
            request.tenant_default_locale.as_deref(),
        )
        .await
        .map_err(|error| map_region_error(&context, owner_operation, error))
        .map(|regions| {
            regions
                .into_iter()
                .map(|region| RegionReadProjection { region })
                .collect()
        })
    }
}

struct RegionReadContextFacts {
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    correlation_id_length: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    locale_length: usize,
    causation_id_present: bool,
    causation_id_length: Option<usize>,
    traceparent_present: bool,
    traceparent_length: Option<usize>,
    idempotency_key_present: bool,
    idempotency_key_length: Option<usize>,
    deadline_ms: Option<u64>,
}

struct RegionReadRequestFacts {
    selector_kind: &'static str,
    selector_uuid_non_nil: Option<bool>,
    country_code_present: bool,
    country_code_length: Option<usize>,
    requested_locale_present: bool,
    requested_locale_length: Option<usize>,
    tenant_default_locale_present: bool,
    tenant_default_locale_length: Option<usize>,
}

struct RegionOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

fn region_read_context_facts(context: &PortContext) -> RegionReadContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    RegionReadContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        correlation_id_length: context.correlation_id.chars().count(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        locale_length: context.locale.chars().count(),
        causation_id_present: context.causation_id.is_some(),
        causation_id_length: context
            .causation_id
            .as_ref()
            .map(|value| value.chars().count()),
        traceparent_present: context.traceparent.is_some(),
        traceparent_length: context
            .traceparent
            .as_ref()
            .map(|value| value.chars().count()),
        idempotency_key_present: context.idempotency_key.is_some(),
        idempotency_key_length: context
            .idempotency_key
            .as_ref()
            .map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

fn region_read_request_facts(request: &RegionReadRequest) -> RegionReadRequestFacts {
    let (selector_kind, selector_uuid_non_nil, country_code_present, country_code_length) =
        match &request.selector {
            RegionReadSelector::Id(region_id) => ("id", Some(!region_id.is_nil()), false, None),
            RegionReadSelector::CountryCode(country_code) => (
                "country_code",
                None,
                true,
                Some(country_code.chars().count()),
            ),
        };
    RegionReadRequestFacts {
        selector_kind,
        selector_uuid_non_nil,
        country_code_present,
        country_code_length,
        requested_locale_present: request.requested_locale.is_some(),
        requested_locale_length: request
            .requested_locale
            .as_ref()
            .map(|value| value.chars().count()),
        tenant_default_locale_present: request.tenant_default_locale.is_some(),
        tenant_default_locale_length: request
            .tenant_default_locale
            .as_ref()
            .map(|value| value.chars().count()),
    }
}

fn region_port_error_kind(kind: &PortErrorKind) -> &'static str {
    match kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    }
}

fn require_region_read_policy(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .map_err(|error| {
            log_region_read_admission_rejection(context, owner_operation, &error);
            error
        })
}

fn log_region_read_admission_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    error: &PortError,
) {
    let context_facts = region_read_context_facts(context);
    tracing::warn!(
        owner = REGION_OWNER,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        operation = owner_operation,
        code = %error.code,
        error_kind = region_port_error_kind(&error.kind),
        error_message_present = !error.message.is_empty(),
        error_message_length = error.message.chars().count(),
        retryable = error.retryable,
        boundary = REGION_READ_PORT_BOUNDARY,
        "region read admission was rejected with bounded diagnostics"
    );
}

fn parse_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    context.tenant_id.parse::<Uuid>().map_err(|_| {
        log_region_tenant_parse_rejection(context, owner_operation);
        PortError::validation(
            "region.tenant_id_invalid",
            "region request context is invalid",
        )
    })
}

fn log_region_tenant_parse_rejection(context: &PortContext, owner_operation: &'static str) {
    let context_facts = region_read_context_facts(context);
    tracing::warn!(
        owner = REGION_OWNER,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        operation = owner_operation,
        code = "region.tenant_id_invalid",
        tenant_id_parse_failed = true,
        boundary = REGION_READ_PORT_BOUNDARY,
        "region read tenant context was rejected with bounded diagnostics"
    );
}

fn validate_region_read_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &RegionReadRequest,
) -> Result<(), PortError> {
    if let RegionReadSelector::CountryCode(country_code) = &request.selector
        && country_code.trim().is_empty()
    {
        log_region_request_validation_rejection(context, owner_operation, request);
        return Err(PortError::validation(
            "region.country_code_empty",
            "region read port requires a non-empty country code selector",
        ));
    }
    Ok(())
}

fn log_region_request_validation_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    request: &RegionReadRequest,
) {
    let context_facts = region_read_context_facts(context);
    let request_facts = region_read_request_facts(request);
    tracing::warn!(
        owner = REGION_OWNER,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        operation = owner_operation,
        code = "region.country_code_empty",
        selector_kind = request_facts.selector_kind,
        selector_uuid_non_nil = ?request_facts.selector_uuid_non_nil,
        country_code_present = request_facts.country_code_present,
        country_code_length = ?request_facts.country_code_length,
        requested_locale_present = request_facts.requested_locale_present,
        requested_locale_length = ?request_facts.requested_locale_length,
        tenant_default_locale_present = request_facts.tenant_default_locale_present,
        tenant_default_locale_length = ?request_facts.tenant_default_locale_length,
        boundary = REGION_READ_PORT_BOUNDARY,
        "region read request was rejected with bounded diagnostics"
    );
}

fn region_owner_error_facts(error: &crate::RegionError) -> RegionOwnerErrorFacts {
    match error {
        crate::RegionError::Validation(message) => RegionOwnerErrorFacts {
            error_variant: "validation",
            text_field_count: 1,
            text_total_length: message.chars().count(),
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        crate::RegionError::RegionNotFound(region_id) => RegionOwnerErrorFacts {
            error_variant: "region_not_found",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 1,
            uuid_non_nil_count: if region_id.is_nil() { 0 } else { 1 },
            opaque_payload_present: false,
        },
        crate::RegionError::InvalidCountryCode(country_code) => RegionOwnerErrorFacts {
            error_variant: "invalid_country_code",
            text_field_count: 1,
            text_total_length: country_code.chars().count(),
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        crate::RegionError::Database(_) => RegionOwnerErrorFacts {
            error_variant: "database",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
    }
}

fn log_region_owner_failure(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    error_facts: &RegionOwnerErrorFacts,
    technical_failure: bool,
) {
    let context_facts = region_read_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = REGION_OWNER,
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = REGION_READ_PORT_BOUNDARY,
            "region owner read failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = REGION_OWNER,
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = REGION_READ_PORT_BOUNDARY,
            "region owner read was rejected with bounded diagnostics"
        );
    }
}

fn map_region_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: crate::RegionError,
) -> PortError {
    let error_facts = region_owner_error_facts(&error);
    match error {
        crate::RegionError::RegionNotFound(_) => {
            log_region_owner_failure(
                context,
                owner_operation,
                "region.not_found",
                &error_facts,
                false,
            );
            PortError::not_found("region.not_found", "region read projection was not found")
        }
        crate::RegionError::Validation(_) | crate::RegionError::InvalidCountryCode(_) => {
            log_region_owner_failure(
                context,
                owner_operation,
                "region.validation",
                &error_facts,
                false,
            );
            PortError::validation("region.validation", "region request is invalid")
        }
        crate::RegionError::Database(_) => {
            log_region_owner_failure(
                context,
                owner_operation,
                "region.read_failed",
                &error_facts,
                true,
            );
            PortError::unavailable(
                "region.read_failed",
                "region storage is temporarily unavailable",
            )
        }
    }
}
