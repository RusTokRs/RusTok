use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::ports::{
    CartPromotionKindRequest, CartPromotionPort, CartPromotionRequest, CartPromotionScopeRequest,
};
use crate::{CartError, CartPromotionPreview, CartResponse, CartService};

const CART_PROMOTION_OWNER: &str = "rustok_cart.promotion";
const CART_PROMOTION_CONTEXT_BOUNDARY: &str = "cart_promotion_context";
const CART_PROMOTION_OWNER_BOUNDARY: &str = "cart_promotion_owner_service";
const READ_CART_PROMOTION_PREVIEW_OPERATION: &str = "read_cart_promotion_preview";
const APPLY_CART_PROMOTION_OPERATION: &str = "apply_cart_promotion";

struct CartPromotionContextFacts {
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
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

struct CartPromotionRequestFacts {
    cart_id_non_nil: bool,
    line_item_id_present: bool,
    line_item_id_non_nil: Option<bool>,
    scope_kind: &'static str,
    promotion_kind: &'static str,
    source_id_present: bool,
    source_id_length: usize,
    amount_text_length: usize,
    metadata_kind: &'static str,
    metadata_size: Option<usize>,
}

struct CartPromotionOwnerErrorFacts {
    error_variant: &'static str,
    validation_detail_present: bool,
    validation_detail_length: Option<usize>,
    resource_id_non_nil: Option<bool>,
    transition_from_length: Option<usize>,
    transition_to_length: Option<usize>,
    database_error_present: bool,
    tax_code_present: bool,
    tax_code_length: Option<usize>,
    tax_message_present: bool,
    tax_message_length: Option<usize>,
}

pub fn guarded_cart_promotion_port(db: DatabaseConnection) -> Arc<dyn CartPromotionPort> {
    Arc::new(GuardedCartPromotionPort {
        service: CartService::new(db),
    })
}

struct GuardedCartPromotionPort {
    service: CartService,
}

#[async_trait]
impl CartPromotionPort for GuardedCartPromotionPort {
    async fn read_cart_promotion_preview(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<CartPromotionPreview, PortError> {
        let owner_operation = READ_CART_PROMOTION_PREVIEW_OPERATION;
        let request_facts = cart_promotion_request_facts(&request);
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| cart_promotion_context_error(&context, owner_operation, error))?;
        validate_cart_promotion_request(&context, owner_operation, &request, &request_facts)?;
        let tenant_id = parse_cart_promotion_tenant_id(&context, owner_operation)?;

        match (request.scope, request.kind) {
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::PercentageDiscount) => {
                self.service
                    .preview_percentage_shipping_promotion(
                        tenant_id,
                        request.cart_id,
                        &request.source_id,
                        request.amount,
                    )
                    .await
            }
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::FixedDiscount) => {
                self.service
                    .preview_fixed_shipping_promotion(
                        tenant_id,
                        request.cart_id,
                        &request.source_id,
                        request.amount,
                    )
                    .await
            }
            (_, CartPromotionKindRequest::PercentageDiscount) => {
                self.service
                    .preview_percentage_promotion(
                        tenant_id,
                        request.cart_id,
                        request.line_item_id,
                        &request.source_id,
                        request.amount,
                    )
                    .await
            }
            (_, CartPromotionKindRequest::FixedDiscount) => {
                self.service
                    .preview_fixed_promotion(
                        tenant_id,
                        request.cart_id,
                        request.line_item_id,
                        &request.source_id,
                        request.amount,
                    )
                    .await
            }
        }
        .map_err(|error| cart_promotion_error(&context, owner_operation, &request_facts, error))
    }

    async fn apply_cart_promotion(
        &self,
        context: PortContext,
        request: CartPromotionRequest,
    ) -> Result<CartResponse, PortError> {
        let owner_operation = APPLY_CART_PROMOTION_OPERATION;
        let request_facts = cart_promotion_request_facts(&request);
        context
            .require_write_semantics()
            .map_err(|error| cart_promotion_context_error(&context, owner_operation, error))?;
        validate_cart_promotion_request(&context, owner_operation, &request, &request_facts)?;
        let tenant_id = parse_cart_promotion_tenant_id(&context, owner_operation)?;

        match (request.scope, request.kind) {
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::PercentageDiscount) => {
                self.service
                    .apply_percentage_shipping_promotion(
                        tenant_id,
                        request.cart_id,
                        &request.source_id,
                        request.amount,
                        request.metadata,
                    )
                    .await
            }
            (CartPromotionScopeRequest::Shipping, CartPromotionKindRequest::FixedDiscount) => {
                self.service
                    .apply_fixed_shipping_promotion(
                        tenant_id,
                        request.cart_id,
                        &request.source_id,
                        request.amount,
                        request.metadata,
                    )
                    .await
            }
            (_, CartPromotionKindRequest::PercentageDiscount) => {
                self.service
                    .apply_percentage_promotion(
                        tenant_id,
                        request.cart_id,
                        request.line_item_id,
                        &request.source_id,
                        request.amount,
                        request.metadata,
                    )
                    .await
            }
            (_, CartPromotionKindRequest::FixedDiscount) => {
                self.service
                    .apply_fixed_promotion(
                        tenant_id,
                        request.cart_id,
                        request.line_item_id,
                        &request.source_id,
                        request.amount,
                        request.metadata,
                    )
                    .await
            }
        }
        .map_err(|error| cart_promotion_error(&context, owner_operation, &request_facts, error))
    }
}

fn cart_promotion_context_facts(context: &PortContext) -> CartPromotionContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    CartPromotionContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
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

fn cart_promotion_port_error_kind(kind: &PortErrorKind) -> &'static str {
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

fn cart_promotion_request_facts(request: &CartPromotionRequest) -> CartPromotionRequestFacts {
    let scope_kind = match &request.scope {
        CartPromotionScopeRequest::Cart => "cart",
        CartPromotionScopeRequest::LineItem => "line_item",
        CartPromotionScopeRequest::Shipping => "shipping",
    };
    let promotion_kind = match &request.kind {
        CartPromotionKindRequest::PercentageDiscount => "percentage_discount",
        CartPromotionKindRequest::FixedDiscount => "fixed_discount",
    };
    let (metadata_kind, metadata_size) = match &request.metadata {
        serde_json::Value::Null => ("null", None),
        serde_json::Value::Bool(_) => ("bool", None),
        serde_json::Value::Number(_) => ("number", None),
        serde_json::Value::String(value) => ("string", Some(value.chars().count())),
        serde_json::Value::Array(values) => ("array", Some(values.len())),
        serde_json::Value::Object(values) => ("object", Some(values.len())),
    };

    CartPromotionRequestFacts {
        cart_id_non_nil: !request.cart_id.is_nil(),
        line_item_id_present: request.line_item_id.is_some(),
        line_item_id_non_nil: request.line_item_id.map(|value| !value.is_nil()),
        scope_kind,
        promotion_kind,
        source_id_present: !request.source_id.trim().is_empty(),
        source_id_length: request.source_id.chars().count(),
        amount_text_length: request.amount.to_string().chars().count(),
        metadata_kind,
        metadata_size,
    }
}

fn cart_promotion_owner_error_facts(error: &CartError) -> CartPromotionOwnerErrorFacts {
    match error {
        CartError::Validation(detail) => CartPromotionOwnerErrorFacts {
            error_variant: "validation",
            validation_detail_present: !detail.trim().is_empty(),
            validation_detail_length: Some(detail.chars().count()),
            resource_id_non_nil: None,
            transition_from_length: None,
            transition_to_length: None,
            database_error_present: false,
            tax_code_present: false,
            tax_code_length: None,
            tax_message_present: false,
            tax_message_length: None,
        },
        CartError::CartNotFound(id) => CartPromotionOwnerErrorFacts {
            error_variant: "cart_not_found",
            validation_detail_present: false,
            validation_detail_length: None,
            resource_id_non_nil: Some(!id.is_nil()),
            transition_from_length: None,
            transition_to_length: None,
            database_error_present: false,
            tax_code_present: false,
            tax_code_length: None,
            tax_message_present: false,
            tax_message_length: None,
        },
        CartError::CartLineItemNotFound(id) => CartPromotionOwnerErrorFacts {
            error_variant: "cart_line_item_not_found",
            validation_detail_present: false,
            validation_detail_length: None,
            resource_id_non_nil: Some(!id.is_nil()),
            transition_from_length: None,
            transition_to_length: None,
            database_error_present: false,
            tax_code_present: false,
            tax_code_length: None,
            tax_message_present: false,
            tax_message_length: None,
        },
        CartError::InvalidTransition { from, to } => CartPromotionOwnerErrorFacts {
            error_variant: "invalid_transition",
            validation_detail_present: false,
            validation_detail_length: None,
            resource_id_non_nil: None,
            transition_from_length: Some(from.chars().count()),
            transition_to_length: Some(to.chars().count()),
            database_error_present: false,
            tax_code_present: false,
            tax_code_length: None,
            tax_message_present: false,
            tax_message_length: None,
        },
        CartError::Database(_) => CartPromotionOwnerErrorFacts {
            error_variant: "database",
            validation_detail_present: false,
            validation_detail_length: None,
            resource_id_non_nil: None,
            transition_from_length: None,
            transition_to_length: None,
            database_error_present: true,
            tax_code_present: false,
            tax_code_length: None,
            tax_message_present: false,
            tax_message_length: None,
        },
        CartError::TaxBoundary { code, message, .. } => CartPromotionOwnerErrorFacts {
            error_variant: "tax_boundary",
            validation_detail_present: false,
            validation_detail_length: None,
            resource_id_non_nil: None,
            transition_from_length: None,
            transition_to_length: None,
            database_error_present: false,
            tax_code_present: !code.trim().is_empty(),
            tax_code_length: Some(code.chars().count()),
            tax_message_present: !message.trim().is_empty(),
            tax_message_length: Some(message.chars().count()),
        },
    }
}

fn validate_cart_promotion_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &CartPromotionRequest,
    request_facts: &CartPromotionRequestFacts,
) -> Result<(), PortError> {
    let code = match &request.scope {
        CartPromotionScopeRequest::LineItem if request.line_item_id.is_none() => {
            Some("cart.promotion_line_item_required")
        }
        CartPromotionScopeRequest::Shipping if request.line_item_id.is_some() => {
            Some("cart.promotion_shipping_line_item_forbidden")
        }
        _ => None,
    };

    if let Some(code) = code {
        let facts = cart_promotion_context_facts(context);
        tracing::warn!(
            owner = CART_PROMOTION_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            locale_length = facts.locale_length,
            causation_id_present = facts.causation_id_present,
            causation_id_length = ?facts.causation_id_length,
            traceparent_present = facts.traceparent_present,
            traceparent_length = ?facts.traceparent_length,
            idempotency_key_present = facts.idempotency_key_present,
            idempotency_key_length = ?facts.idempotency_key_length,
            deadline_ms = ?facts.deadline_ms,
            operation = owner_operation,
            scope_kind = request_facts.scope_kind,
            promotion_kind = request_facts.promotion_kind,
            cart_id_non_nil = request_facts.cart_id_non_nil,
            line_item_id_present = request_facts.line_item_id_present,
            line_item_id_non_nil = ?request_facts.line_item_id_non_nil,
            source_id_present = request_facts.source_id_present,
            source_id_length = request_facts.source_id_length,
            amount_text_length = request_facts.amount_text_length,
            metadata_kind = request_facts.metadata_kind,
            metadata_size = ?request_facts.metadata_size,
            code,
            boundary = CART_PROMOTION_CONTEXT_BOUNDARY,
            "cart promotion target validation failed"
        );
        return Err(PortError::validation(
            code,
            "cart promotion request is invalid",
        ));
    }

    Ok(())
}

fn parse_cart_promotion_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let facts = cart_promotion_context_facts(context);
        tracing::warn!(
            owner = CART_PROMOTION_OWNER,
            correlation_id = %context.correlation_id,
            tenant_id_length = facts.tenant_id_length,
            actor_kind = facts.actor_kind,
            actor_id_length = facts.actor_id_length,
            claim_count = facts.claim_count,
            role_count = facts.role_count,
            channel_present = facts.channel_present,
            channel_length = ?facts.channel_length,
            locale_length = facts.locale_length,
            causation_id_present = facts.causation_id_present,
            causation_id_length = ?facts.causation_id_length,
            traceparent_present = facts.traceparent_present,
            traceparent_length = ?facts.traceparent_length,
            idempotency_key_present = facts.idempotency_key_present,
            idempotency_key_length = ?facts.idempotency_key_length,
            deadline_ms = ?facts.deadline_ms,
            operation = owner_operation,
            code = "cart.tenant_id_invalid",
            tenant_id_parse_failed = true,
            boundary = CART_PROMOTION_CONTEXT_BOUNDARY,
            "cart promotion tenant context is invalid"
        );
        PortError::validation(
            "cart.tenant_id_invalid",
            "cart promotion request context is invalid",
        )
    })
}

fn cart_promotion_context_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> PortError {
    log_cart_promotion_context_rejection(context, owner_operation, &error);

    match error.kind {
        PortErrorKind::Timeout => {
            PortError::timeout(error.code, "cart promotion request context is invalid")
        }
        PortErrorKind::Validation => {
            PortError::validation(error.code, "cart promotion request context is invalid")
        }
        kind => PortError::new(
            kind,
            "cart.promotion_context_invalid",
            "cart promotion request context is invalid",
            error.retryable,
        ),
    }
}

fn log_cart_promotion_context_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    error: &PortError,
) {
    let facts = cart_promotion_context_facts(context);
    match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                owner = CART_PROMOTION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id_length = facts.tenant_id_length,
                actor_kind = facts.actor_kind,
                actor_id_length = facts.actor_id_length,
                claim_count = facts.claim_count,
                role_count = facts.role_count,
                channel_present = facts.channel_present,
                channel_length = ?facts.channel_length,
                locale_length = facts.locale_length,
                causation_id_present = facts.causation_id_present,
                causation_id_length = ?facts.causation_id_length,
                traceparent_present = facts.traceparent_present,
                traceparent_length = ?facts.traceparent_length,
                idempotency_key_present = facts.idempotency_key_present,
                idempotency_key_length = ?facts.idempotency_key_length,
                deadline_ms = ?facts.deadline_ms,
                operation = owner_operation,
                internal_code = %error.code,
                internal_message_present = !error.message.trim().is_empty(),
                internal_message_length = error.message.chars().count(),
                error_kind = cart_promotion_port_error_kind(&error.kind),
                retryable = error.retryable,
                boundary = CART_PROMOTION_CONTEXT_BOUNDARY,
                code = "cart.promotion_context_invalid",
                "cart promotion call context failed"
            );
        }
        _ => {
            tracing::warn!(
                owner = CART_PROMOTION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id_length = facts.tenant_id_length,
                actor_kind = facts.actor_kind,
                actor_id_length = facts.actor_id_length,
                claim_count = facts.claim_count,
                role_count = facts.role_count,
                channel_present = facts.channel_present,
                channel_length = ?facts.channel_length,
                locale_length = facts.locale_length,
                causation_id_present = facts.causation_id_present,
                causation_id_length = ?facts.causation_id_length,
                traceparent_present = facts.traceparent_present,
                traceparent_length = ?facts.traceparent_length,
                idempotency_key_present = facts.idempotency_key_present,
                idempotency_key_length = ?facts.idempotency_key_length,
                deadline_ms = ?facts.deadline_ms,
                operation = owner_operation,
                internal_code = %error.code,
                internal_message_present = !error.message.trim().is_empty(),
                internal_message_length = error.message.chars().count(),
                error_kind = cart_promotion_port_error_kind(&error.kind),
                retryable = error.retryable,
                boundary = CART_PROMOTION_CONTEXT_BOUNDARY,
                code = "cart.promotion_context_invalid",
                "cart promotion call context was rejected"
            );
        }
    }
}

fn cart_promotion_error(
    context: &PortContext,
    owner_operation: &'static str,
    request_facts: &CartPromotionRequestFacts,
    error: CartError,
) -> PortError {
    let owner_code = cart_promotion_error_code(&error);
    let owner_error_facts = cart_promotion_owner_error_facts(&error);
    let public_error = match &error {
        CartError::Validation(_) => PortError::validation(
            "cart.promotion_validation",
            "cart promotion request is invalid",
        ),
        CartError::CartNotFound(_) => {
            PortError::not_found("cart.cart_not_found", "cart was not found")
        }
        CartError::CartLineItemNotFound(_) => {
            PortError::not_found("cart.line_item_not_found", "cart line item was not found")
        }
        CartError::InvalidTransition { .. } => PortError::conflict(
            "cart.promotion_state_conflict",
            "cart promotion conflicts with the current cart state",
        ),
        CartError::Database(_) => PortError::unavailable(
            "cart.database_unavailable",
            "cart storage is temporarily unavailable",
        ),
        CartError::TaxBoundary {
            kind,
            code,
            retryable,
            ..
        } => PortError::new(
            kind.clone(),
            code.clone(),
            "cart promotion tax recalculation failed",
            *retryable,
        ),
    };
    let facts = cart_promotion_context_facts(context);

    match &public_error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                owner = CART_PROMOTION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id_length = facts.tenant_id_length,
                actor_kind = facts.actor_kind,
                actor_id_length = facts.actor_id_length,
                claim_count = facts.claim_count,
                role_count = facts.role_count,
                channel_present = facts.channel_present,
                channel_length = ?facts.channel_length,
                locale_length = facts.locale_length,
                causation_id_present = facts.causation_id_present,
                causation_id_length = ?facts.causation_id_length,
                traceparent_present = facts.traceparent_present,
                traceparent_length = ?facts.traceparent_length,
                idempotency_key_present = facts.idempotency_key_present,
                idempotency_key_length = ?facts.idempotency_key_length,
                deadline_ms = ?facts.deadline_ms,
                operation = owner_operation,
                scope_kind = request_facts.scope_kind,
                promotion_kind = request_facts.promotion_kind,
                cart_id_non_nil = request_facts.cart_id_non_nil,
                line_item_id_present = request_facts.line_item_id_present,
                line_item_id_non_nil = ?request_facts.line_item_id_non_nil,
                source_id_present = request_facts.source_id_present,
                source_id_length = request_facts.source_id_length,
                amount_text_length = request_facts.amount_text_length,
                metadata_kind = request_facts.metadata_kind,
                metadata_size = ?request_facts.metadata_size,
                owner_error_variant = owner_error_facts.error_variant,
                validation_detail_present = owner_error_facts.validation_detail_present,
                validation_detail_length = ?owner_error_facts.validation_detail_length,
                resource_id_non_nil = ?owner_error_facts.resource_id_non_nil,
                transition_from_length = ?owner_error_facts.transition_from_length,
                transition_to_length = ?owner_error_facts.transition_to_length,
                database_error_present = owner_error_facts.database_error_present,
                tax_code_present = owner_error_facts.tax_code_present,
                tax_code_length = ?owner_error_facts.tax_code_length,
                tax_message_present = owner_error_facts.tax_message_present,
                tax_message_length = ?owner_error_facts.tax_message_length,
                owner_code,
                public_code = %public_error.code,
                error_kind = cart_promotion_port_error_kind(&public_error.kind),
                retryable = public_error.retryable,
                boundary = CART_PROMOTION_OWNER_BOUNDARY,
                "cart promotion owner operation failed"
            );
        }
        _ => {
            tracing::warn!(
                owner = CART_PROMOTION_OWNER,
                correlation_id = %context.correlation_id,
                tenant_id_length = facts.tenant_id_length,
                actor_kind = facts.actor_kind,
                actor_id_length = facts.actor_id_length,
                claim_count = facts.claim_count,
                role_count = facts.role_count,
                channel_present = facts.channel_present,
                channel_length = ?facts.channel_length,
                locale_length = facts.locale_length,
                causation_id_present = facts.causation_id_present,
                causation_id_length = ?facts.causation_id_length,
                traceparent_present = facts.traceparent_present,
                traceparent_length = ?facts.traceparent_length,
                idempotency_key_present = facts.idempotency_key_present,
                idempotency_key_length = ?facts.idempotency_key_length,
                deadline_ms = ?facts.deadline_ms,
                operation = owner_operation,
                scope_kind = request_facts.scope_kind,
                promotion_kind = request_facts.promotion_kind,
                cart_id_non_nil = request_facts.cart_id_non_nil,
                line_item_id_present = request_facts.line_item_id_present,
                line_item_id_non_nil = ?request_facts.line_item_id_non_nil,
                source_id_present = request_facts.source_id_present,
                source_id_length = request_facts.source_id_length,
                amount_text_length = request_facts.amount_text_length,
                metadata_kind = request_facts.metadata_kind,
                metadata_size = ?request_facts.metadata_size,
                owner_error_variant = owner_error_facts.error_variant,
                validation_detail_present = owner_error_facts.validation_detail_present,
                validation_detail_length = ?owner_error_facts.validation_detail_length,
                resource_id_non_nil = ?owner_error_facts.resource_id_non_nil,
                transition_from_length = ?owner_error_facts.transition_from_length,
                transition_to_length = ?owner_error_facts.transition_to_length,
                database_error_present = owner_error_facts.database_error_present,
                tax_code_present = owner_error_facts.tax_code_present,
                tax_code_length = ?owner_error_facts.tax_code_length,
                tax_message_present = owner_error_facts.tax_message_present,
                tax_message_length = ?owner_error_facts.tax_message_length,
                owner_code,
                public_code = %public_error.code,
                error_kind = cart_promotion_port_error_kind(&public_error.kind),
                retryable = public_error.retryable,
                boundary = CART_PROMOTION_OWNER_BOUNDARY,
                "cart promotion owner operation was rejected"
            );
        }
    }

    public_error
}

fn cart_promotion_error_code(error: &CartError) -> &str {
    match error {
        CartError::Validation(_) => "cart.promotion_validation",
        CartError::CartNotFound(_) => "cart.cart_not_found",
        CartError::CartLineItemNotFound(_) => "cart.line_item_not_found",
        CartError::InvalidTransition { .. } => "cart.promotion_state_conflict",
        CartError::Database(_) => "cart.database_unavailable",
        CartError::TaxBoundary { code, .. } => code.as_str(),
    }
}
