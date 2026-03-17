/// Built-in workflow marketplace templates.
/// Each template provides a ready-made workflow configuration that users
/// can import and customise for their tenant.
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::entities::{OnError, StepType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateStep {
    pub step_type: StepType,
    pub config: serde_json::Value,
    pub on_error: OnError,
    pub timeout_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub trigger_config: serde_json::Value,
    pub steps: Vec<TemplateStep>,
}

fn blog_published_notify() -> WorkflowTemplate {
    WorkflowTemplate {
        id: "blog-published-notify",
        name: "Blog Post Published → Notify Subscribers",
        description: "Sends an email notification to subscribers whenever a blog post is published.",
        category: "content",
        trigger_config: json!({ "type": "event", "event_type": "blog.post.published" }),
        steps: vec![
            TemplateStep {
                step_type: StepType::Notify,
                config: json!({
                    "channel": "email",
                    "template": "blog_post_published",
                    "recipient": "{{context.author_email}}",
                    "subject": "Your post \"{{context.title}}\" is live!",
                }),
                on_error: OnError::Skip,
                timeout_ms: Some(10_000),
            },
            TemplateStep {
                step_type: StepType::EmitEvent,
                config: json!({
                    "event_type": "workflow.blog_notify.completed",
                    "payload": { "post_id": "{{context.post_id}}" }
                }),
                on_error: OnError::Skip,
                timeout_ms: None,
            },
        ],
    }
}

fn order_paid_fulfillment() -> WorkflowTemplate {
    WorkflowTemplate {
        id: "order-paid-fulfillment",
        name: "Order Paid → Fulfillment Pipeline",
        description: "Triggers fulfillment and sends order confirmation email when payment is received.",
        category: "commerce",
        trigger_config: json!({ "type": "event", "event_type": "commerce.order.paid" }),
        steps: vec![
            TemplateStep {
                step_type: StepType::Action,
                config: json!({
                    "service": "commerce",
                    "action": "fulfill_order",
                    "params": { "order_id": "{{context.order_id}}" }
                }),
                on_error: OnError::Stop,
                timeout_ms: Some(30_000),
            },
            TemplateStep {
                step_type: StepType::Notify,
                config: json!({
                    "channel": "email",
                    "template": "order_confirmation",
                    "recipient": "{{context.customer_email}}",
                    "subject": "Order #{{context.order_number}} confirmed",
                }),
                on_error: OnError::Skip,
                timeout_ms: Some(10_000),
            },
        ],
    }
}

fn new_user_onboarding() -> WorkflowTemplate {
    WorkflowTemplate {
        id: "new-user-onboarding",
        name: "New User → Onboarding Sequence",
        description: "Sends a welcome email and queues a follow-up notification after user registration.",
        category: "auth",
        trigger_config: json!({ "type": "event", "event_type": "auth.user.registered" }),
        steps: vec![
            TemplateStep {
                step_type: StepType::Notify,
                config: json!({
                    "channel": "email",
                    "template": "welcome",
                    "recipient": "{{context.email}}",
                    "subject": "Welcome to the platform!",
                }),
                on_error: OnError::Skip,
                timeout_ms: Some(10_000),
            },
            TemplateStep {
                step_type: StepType::Delay,
                config: json!({ "duration_seconds": 86400 }),
                on_error: OnError::Skip,
                timeout_ms: None,
            },
            TemplateStep {
                step_type: StepType::Notify,
                config: json!({
                    "channel": "email",
                    "template": "onboarding_day2",
                    "recipient": "{{context.email}}",
                    "subject": "Getting started tips",
                }),
                on_error: OnError::Skip,
                timeout_ms: Some(10_000),
            },
        ],
    }
}

fn daily_report() -> WorkflowTemplate {
    WorkflowTemplate {
        id: "daily-report",
        name: "Daily Summary Report (Cron)",
        description: "Generates and emails a daily activity summary every morning at 08:00.",
        category: "reporting",
        trigger_config: json!({ "type": "cron", "expression": "0 0 8 * * *" }),
        steps: vec![
            TemplateStep {
                step_type: StepType::AlloyScript,
                config: json!({
                    "script_name": "generate_daily_report",
                    "params": {}
                }),
                on_error: OnError::Stop,
                timeout_ms: Some(60_000),
            },
            TemplateStep {
                step_type: StepType::Notify,
                config: json!({
                    "channel": "email",
                    "template": "daily_report",
                    "recipient": "{{context.admin_email}}",
                    "subject": "Daily Report — {{context.date}}",
                }),
                on_error: OnError::Skip,
                timeout_ms: Some(10_000),
            },
        ],
    }
}

fn webhook_crm_sync() -> WorkflowTemplate {
    WorkflowTemplate {
        id: "webhook-crm-sync",
        name: "Incoming Webhook → CRM Sync",
        description: "Receives data from an external CRM webhook and syncs it to the platform via an HTTP call.",
        category: "integrations",
        trigger_config: json!({ "type": "webhook", "path": "crm-sync" }),
        steps: vec![
            TemplateStep {
                step_type: StepType::Condition,
                config: json!({
                    "field": "webhook.payload.event",
                    "operator": "equals",
                    "value": "contact.updated",
                    "on_false": "stop"
                }),
                on_error: OnError::Stop,
                timeout_ms: None,
            },
            TemplateStep {
                step_type: StepType::Http,
                config: json!({
                    "method": "POST",
                    "url": "https://api.example.com/contacts/sync",
                    "headers": { "Content-Type": "application/json" },
                    "body": "{{webhook.payload.contact}}"
                }),
                on_error: OnError::Retry,
                timeout_ms: Some(15_000),
            },
            TemplateStep {
                step_type: StepType::EmitEvent,
                config: json!({
                    "event_type": "integration.crm.synced",
                    "payload": { "contact_id": "{{context.contact_id}}" }
                }),
                on_error: OnError::Skip,
                timeout_ms: None,
            },
        ],
    }
}

/// All built-in workflow templates available in the marketplace.
pub static BUILTIN_TEMPLATES: std::sync::LazyLock<Vec<WorkflowTemplate>> =
    std::sync::LazyLock::new(|| {
        vec![
            blog_published_notify(),
            order_paid_fulfillment(),
            new_user_onboarding(),
            daily_report(),
            webhook_crm_sync(),
        ]
    });
