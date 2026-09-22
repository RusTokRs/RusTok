use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};

use crate::{EmailError, TransactionalEmailSender};

/// Require shared write semantics for transactional email delivery calls.
pub fn require_email_delivery_policy(context: &PortContext) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .map_err(|error| match error.kind {
            PortErrorKind::Timeout => PortError::timeout(
                "email.deadline_required",
                "email delivery port calls require deadline semantics",
            ),
            PortErrorKind::Validation => PortError::validation(
                "email.idempotency_required",
                "email delivery port calls require an idempotency key",
            ),
            _ => error,
        })
}

/// Transport-neutral transactional delivery request owned by the email module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmailDeliveryRequest {
    pub template_id: String,
    pub locale: String,
    pub to: String,
    pub vars: serde_json::Value,
}

/// Transport-neutral delivery result exposed to workflow/auth/commerce consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailDeliveryReceipt {
    pub accepted: bool,
    pub provider_mode: EmailProviderMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmailProviderMode {
    DisabledNoop,
    Smtp,
}

/// Transport-neutral owner boundary for transactional email delivery.
#[async_trait]
pub trait EmailDeliveryPort: Send + Sync {
    async fn send_transactional_email(
        &self,
        context: PortContext,
        request: EmailDeliveryRequest,
    ) -> Result<EmailDeliveryReceipt, PortError>;
}

#[async_trait]
impl EmailDeliveryPort for crate::EmailService {
    async fn send_transactional_email(
        &self,
        context: PortContext,
        request: EmailDeliveryRequest,
    ) -> Result<EmailDeliveryReceipt, PortError> {
        require_email_delivery_policy(&context)?;
        validate_delivery_request(&request)?;
        self.send_transactional(
            &request.template_id,
            &request.locale,
            &request.to,
            &request.vars,
        )
        .await
        .map_err(map_email_error)?;

        Ok(EmailDeliveryReceipt {
            accepted: true,
            provider_mode: match self {
                crate::EmailService::Disabled => EmailProviderMode::DisabledNoop,
                crate::EmailService::Smtp(_) => EmailProviderMode::Smtp,
            },
        })
    }
}

fn validate_delivery_request(request: &EmailDeliveryRequest) -> Result<(), PortError> {
    if request.template_id.trim().is_empty() {
        return Err(PortError::validation(
            "email.template_id_empty",
            "email delivery requires a non-empty template id",
        ));
    }
    if request.locale.trim().is_empty() {
        return Err(PortError::validation(
            "email.locale_empty",
            "email delivery requires a non-empty locale",
        ));
    }
    if request.to.trim().is_empty() {
        return Err(PortError::validation(
            "email.recipient_empty",
            "email delivery requires a non-empty recipient",
        ));
    }
    Ok(())
}

fn map_email_error(error: EmailError) -> PortError {
    match error {
        EmailError::Disabled => {
            PortError::unavailable("email.disabled", "email sending is disabled".to_string())
        }
        EmailError::Template(message) => {
            PortError::invariant_violation("email.template_failed", message)
        }
        EmailError::InvalidAddress(message) | EmailError::Build(message) => {
            PortError::validation("email.delivery_invalid", message)
        }
        EmailError::SmtpConfig(message) | EmailError::Send(message) => {
            PortError::unavailable("email.delivery_failed", message)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortContext, PortErrorKind};

    use super::*;

    fn base_context() -> PortContext {
        PortContext::new(
            "tenant-a",
            PortActor::service("email-contract-test"),
            "ru",
            "corr-email-a",
        )
    }

    fn write_context() -> PortContext {
        base_context()
            .with_idempotency_key("email-send-a")
            .with_deadline(Duration::from_secs(3))
    }

    fn delivery_request() -> EmailDeliveryRequest {
        EmailDeliveryRequest {
            template_id: "auth/password_reset".to_string(),
            locale: "ru".to_string(),
            to: "user@example.test".to_string(),
            vars: serde_json::json!({ "reset_url": "https://admin.example.test/reset?token=t" }),
        }
    }

    #[test]
    fn delivery_policy_maps_missing_deadline_to_email_specific_timeout() {
        let error = require_email_delivery_policy(&base_context())
            .expect_err("write policy without deadline/idempotency must fail");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "email.idempotency_required");
        assert!(!error.retryable);

        let error =
            require_email_delivery_policy(&base_context().with_idempotency_key("email-send-a"))
                .expect_err("write policy with idempotency but without deadline must fail");

        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert_eq!(error.code, "email.deadline_required");
        assert!(error.retryable);
    }

    #[test]
    fn delivery_policy_accepts_shared_write_context() {
        assert!(require_email_delivery_policy(&write_context()).is_ok());
    }

    #[tokio::test]
    async fn disabled_provider_preserves_noop_receipt_after_policy_and_validation() {
        let receipt = EmailDeliveryPort::send_transactional_email(
            &crate::EmailService::Disabled,
            write_context(),
            delivery_request(),
        )
        .await
        .expect("disabled provider is an accepted noop fallback");

        assert!(receipt.accepted);
        assert_eq!(receipt.provider_mode, EmailProviderMode::DisabledNoop);
    }

    #[tokio::test]
    async fn delivery_request_validation_uses_typed_port_errors() {
        let mut request = delivery_request();
        request.template_id = " ".to_string();

        let error = EmailDeliveryPort::send_transactional_email(
            &crate::EmailService::Disabled,
            write_context(),
            request,
        )
        .await
        .expect_err("empty template id must be rejected before provider delivery");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "email.template_id_empty");
        assert!(!error.retryable);
    }
}
