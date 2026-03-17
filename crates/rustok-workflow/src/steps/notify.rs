use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

use super::{StepContext, StepOutput, WorkflowStep};
use crate::error::{WorkflowError, WorkflowResult};

/// Abstract interface for sending notifications.
/// Implement and register to support email, Telegram, Slack, etc.
#[async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send(
        &self,
        channel: &str,
        recipient: &str,
        subject: &str,
        body: &str,
    ) -> WorkflowResult<()>;
}

/// Notify step — sends a notification via a registered `NotificationSender`.
///
/// Step config format:
/// ```json
/// {
///   "channel": "email",
///   "recipient": "user@example.com",
///   "subject": "Your order is ready",
///   "body": "Hello! Your order {{context.order_id}} has been processed."
/// }
/// ```
pub struct NotifyStep {
    sender: Option<std::sync::Arc<dyn NotificationSender>>,
}

impl NotifyStep {
    pub fn new(sender: std::sync::Arc<dyn NotificationSender>) -> Self {
        Self { sender: Some(sender) }
    }

    pub fn stub() -> Self {
        Self { sender: None }
    }
}

#[async_trait]
impl WorkflowStep for NotifyStep {
    fn step_type(&self) -> &'static str {
        "notify"
    }

    async fn execute(&self, config: &Value, context: StepContext) -> WorkflowResult<StepOutput> {
        let channel = config
            .get("channel")
            .and_then(Value::as_str)
            .unwrap_or("email");

        let recipient = config
            .get("recipient")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                WorkflowError::InvalidStepConfig("notify: missing 'recipient'".into())
            })?;

        let subject = config
            .get("subject")
            .and_then(Value::as_str)
            .unwrap_or("Workflow notification");

        let body = config
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("");

        info!(channel = channel, recipient = recipient, "Executing notify step");

        let sender = self.sender.as_ref().ok_or_else(|| {
            WorkflowError::StepFailed(
                "notify: no NotificationSender registered".into(),
            )
        })?;

        sender.send(channel, recipient, subject, body).await?;

        Ok(StepOutput::continue_with(
            context,
            serde_json::json!({
                "channel": channel,
                "recipient": recipient,
                "status": "sent"
            }),
        ))
    }
}
