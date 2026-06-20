use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{EmailError, TransactionalEmailSender};

/// Transport-agnostic context for email delivery boundary calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortContext {
    pub tenant_id: String,
    pub correlation_id: String,
    pub deadline_ms: Option<u64>,
    pub idempotency_key: Option<String>,
}

impl PortContext {
    pub fn require_deadline_semantics(&self) -> Result<(), PortError> {
        if self.deadline_ms.unwrap_or_default() == 0 {
            return Err(PortError::new(
                PortErrorKind::Timeout,
                "email.deadline_required",
                "email delivery port calls require deadline semantics",
                true,
            ));
        }
        Ok(())
    }

    pub fn require_write_semantics(&self) -> Result<(), PortError> {
        self.require_deadline_semantics()?;
        if self
            .idempotency_key
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(PortError::new(
                PortErrorKind::Validation,
                "email.idempotency_required",
                "email delivery port calls require an idempotency key",
                false,
            ));
        }
        Ok(())
    }
}

/// Transport-neutral error returned by email owner ports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortError {
    pub kind: PortErrorKind,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl PortError {
    pub fn new(
        kind: PortErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortErrorKind {
    Validation,
    Template,
    Unavailable,
    Timeout,
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
        context.require_write_semantics()?;
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
        return Err(PortError::new(
            PortErrorKind::Validation,
            "email.template_id_empty",
            "email delivery requires a non-empty template id",
            false,
        ));
    }
    if request.locale.trim().is_empty() {
        return Err(PortError::new(
            PortErrorKind::Validation,
            "email.locale_empty",
            "email delivery requires a non-empty locale",
            false,
        ));
    }
    if request.to.trim().is_empty() {
        return Err(PortError::new(
            PortErrorKind::Validation,
            "email.recipient_empty",
            "email delivery requires a non-empty recipient",
            false,
        ));
    }
    Ok(())
}

fn map_email_error(error: EmailError) -> PortError {
    match error {
        EmailError::Template(message) => PortError::new(
            PortErrorKind::Template,
            "email.template_failed",
            message,
            false,
        ),
        EmailError::InvalidAddress(message) | EmailError::Build(message) => PortError::new(
            PortErrorKind::Validation,
            "email.delivery_invalid",
            message,
            false,
        ),
        EmailError::SmtpConfig(message) | EmailError::Send(message) => PortError::new(
            PortErrorKind::Unavailable,
            "email.delivery_failed",
            message,
            true,
        ),
    }
}
