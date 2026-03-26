use async_trait::async_trait;
use lettre::{
    message::{header::ContentType, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use crate::config::EmailConfig;
use crate::error::{EmailError, Result};
use crate::template::{EmailTemplateProvider, RenderedEmail};

/// Email to send for password reset.
#[derive(Debug, Clone)]
pub struct PasswordResetEmail {
    pub to: String,
    pub reset_url: String,
}

/// Trait for sending password reset emails. Allows test doubles.
#[async_trait]
pub trait PasswordResetEmailSender: Send + Sync {
    async fn send_password_reset(&self, email: PasswordResetEmail) -> Result<()>;
}

/// General-purpose transactional email sender.
///
/// Modules register their templates via [`EmailTemplateProvider`] and call
/// `send_transactional` with a `template_id` and JSON vars. The sender looks up
/// the provider that handles the template, renders it, and delivers via SMTP.
///
/// Template ID convention: `{module_slug}/{action}`
/// e.g. `"auth/password_reset"`, `"commerce/order_confirmed"`, `"forum/new_reply"`.
#[async_trait]
pub trait TransactionalEmailSender: Send + Sync {
    async fn send_transactional(
        &self,
        template_id: &str,
        locale: &str,
        to: &str,
        vars: &serde_json::Value,
    ) -> Result<()>;
}

/// Top-level email service — disabled or SMTP-backed.
#[derive(Clone)]
pub enum EmailService {
    Disabled,
    Smtp(Box<SmtpEmailSender>),
}

impl EmailService {
    /// Build from config. Returns `Disabled` if `enabled == false`.
    pub fn from_config(config: &EmailConfig) -> Result<Self> {
        if !config.enabled {
            return Ok(Self::Disabled);
        }
        Ok(Self::Smtp(Box::new(SmtpEmailSender::try_new(config)?)))
    }

    /// Build the password-reset URL from config + token.
    pub fn password_reset_url(config: &EmailConfig, token: &str) -> String {
        format!("{}?token={}", config.reset_base_url, token)
    }
}

#[async_trait]
impl PasswordResetEmailSender for EmailService {
    async fn send_password_reset(&self, email: PasswordResetEmail) -> Result<()> {
        match self {
            Self::Disabled => {
                tracing::info!(
                    recipient = %email.to,
                    "Password reset email provider disabled; skipping outbound send"
                );
                Ok(())
            }
            Self::Smtp(sender) => sender.send_password_reset(email).await,
        }
    }
}

#[async_trait]
impl TransactionalEmailSender for EmailService {
    async fn send_transactional(
        &self,
        template_id: &str,
        locale: &str,
        to: &str,
        vars: &serde_json::Value,
    ) -> Result<()> {
        match self {
            Self::Disabled => {
                tracing::info!(
                    recipient = %to,
                    template_id,
                    "Transactional email provider disabled; skipping outbound send"
                );
                Ok(())
            }
            Self::Smtp(sender) => sender.send_transactional(template_id, locale, to, vars).await,
        }
    }
}

/// SMTP-backed email sender.
#[derive(Clone)]
pub struct SmtpEmailSender {
    from: Mailbox,
    transport: AsyncSmtpTransport<Tokio1Executor>,
    providers: Vec<std::sync::Arc<dyn EmailTemplateProvider>>,
}

impl SmtpEmailSender {
    fn try_new(config: &EmailConfig) -> Result<Self> {
        let from = config
            .from
            .parse::<Mailbox>()
            .map_err(|e| EmailError::InvalidAddress(format!("Invalid from address: {e}")))?;

        let mut transport_builder = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp.host)
            .map_err(|e| EmailError::SmtpConfig(format!("Invalid SMTP relay: {e}")))?
            .port(config.smtp.port);

        if !config.smtp.username.trim().is_empty() {
            let creds =
                Credentials::new(config.smtp.username.clone(), config.smtp.password.clone());
            transport_builder = transport_builder.credentials(creds);
        }

        Ok(Self {
            from,
            transport: transport_builder.build(),
            providers: Vec::new(),
        })
    }

    /// Register an additional template provider for transactional email.
    pub fn with_provider(mut self, provider: std::sync::Arc<dyn EmailTemplateProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Send a pre-rendered email to the given recipient.
    pub async fn send_rendered(&self, to: &str, rendered: &RenderedEmail) -> Result<()> {
        let recipient = to
            .parse::<Mailbox>()
            .map_err(|e| EmailError::InvalidAddress(format!("Invalid recipient: {e}")))?;

        let body = MultiPart::alternative()
            .singlepart(
                SinglePart::builder()
                    .header(lettre::message::header::ContentType::TEXT_PLAIN)
                    .body(rendered.text.clone()),
            )
            .singlepart(
                SinglePart::builder()
                    .header(lettre::message::header::ContentType::TEXT_HTML)
                    .body(rendered.html.clone()),
            );

        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject(rendered.subject.clone())
            .multipart(body)
            .map_err(|e| EmailError::Build(e.to_string()))?;

        self.transport
            .send(message)
            .await
            .map_err(|e| EmailError::Send(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl PasswordResetEmailSender for SmtpEmailSender {
    async fn send_password_reset(&self, email: PasswordResetEmail) -> Result<()> {
        let recipient = email
            .to
            .parse::<Mailbox>()
            .map_err(|e| EmailError::InvalidAddress(format!("Invalid recipient: {e}")))?;

        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject("RusToK password reset")
            .header(ContentType::TEXT_HTML)
            .body(format!(
                "<p>You requested a password reset.</p><p><a href=\"{}\">Reset password</a></p>",
                email.reset_url
            ))
            .map_err(|e| EmailError::Build(e.to_string()))?;

        self.transport
            .send(message)
            .await
            .map_err(|e| EmailError::Send(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl TransactionalEmailSender for SmtpEmailSender {
    async fn send_transactional(
        &self,
        template_id: &str,
        locale: &str,
        to: &str,
        vars: &serde_json::Value,
    ) -> Result<()> {
        for provider in &self.providers {
            if let Some(result) = provider.render(template_id, locale, vars) {
                let rendered = result?;
                return self.send_rendered(to, &rendered).await;
            }
        }
        Err(EmailError::Template(format!(
            "No template provider handles '{template_id}'"
        )))
    }
}
