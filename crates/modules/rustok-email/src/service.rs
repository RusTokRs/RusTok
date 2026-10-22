use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart, SinglePart, header::ContentType},
    transport::smtp::authentication::Credentials,
};

use crate::config::EmailConfig;
use crate::error::{EmailError, Result};
use crate::template::{EmailTemplateProvider, RenderedEmail};

/// Email to send for password reset.
#[derive(Clone, PartialEq, Eq)]
pub struct PasswordResetEmail {
    pub to: String,
    pub reset_url: String,
}

impl std::fmt::Debug for PasswordResetEmail {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PasswordResetEmail")
            .field("to", &self.to)
            .field("reset_url", &"<redacted>")
            .finish()
    }
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
        if let Ok(mut url) = url::Url::parse(&config.reset_base_url) {
            url.query_pairs_mut().append_pair("token", token);
            return url.to_string();
        }

        let encoded_token: String =
            url::form_urlencoded::byte_serialize(token.as_bytes()).collect();
        let trimmed = config.reset_base_url.trim();
        let separator = if trimmed.ends_with('?') || trimmed.ends_with('&') {
            ""
        } else if trimmed.contains('?') {
            "&"
        } else {
            "?"
        };
        format!(
            "{trimmed}{separator}token={encoded_token}"
        )
    }

    /// Register an additional template provider for transactional email.
    pub fn with_provider(mut self, provider: std::sync::Arc<dyn EmailTemplateProvider>) -> Self {
        if let Self::Smtp(ref mut sender) = self {
            sender.providers.push(provider);
        }
        self
    }

    /// Add a template provider to this email service instance in place.
    pub fn add_provider(&mut self, provider: std::sync::Arc<dyn EmailTemplateProvider>) {
        if let Self::Smtp(ref mut sender) = self {
            sender.providers.push(provider);
        }
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
            Self::Smtp(sender) => {
                sender
                    .send_transactional(template_id, locale, to, vars)
                    .await
            }
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

fn html_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

impl SmtpEmailSender {
    fn try_new(config: &EmailConfig) -> Result<Self> {
        if config.from.contains('\r') || config.from.contains('\n') {
            return Err(EmailError::InvalidAddress(
                "From address contains invalid control characters".to_string(),
            ));
        }

        let from = config
            .from
            .parse::<Mailbox>()
            .map_err(|e| EmailError::InvalidAddress(format!("Invalid from address: {e}")))?;

        let host = config.smtp.host.trim();
        if host.is_empty() {
            return Err(EmailError::SmtpConfig("SMTP host cannot be empty".to_string()));
        }

        if config.smtp.port == 0 {
            return Err(EmailError::SmtpConfig("SMTP port cannot be 0".to_string()));
        }

        let mut transport_builder = AsyncSmtpTransport::<Tokio1Executor>::relay(host)
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
        if to.contains('\r') || to.contains('\n') {
            return Err(EmailError::InvalidAddress(
                "Recipient address contains invalid control characters".to_string(),
            ));
        }

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
        if email.to.contains('\r') || email.to.contains('\n') {
            return Err(EmailError::InvalidAddress(
                "Recipient address contains invalid control characters".to_string(),
            ));
        }

        let recipient = email
            .to
            .parse::<Mailbox>()
            .map_err(|e| EmailError::InvalidAddress(format!("Invalid recipient: {e}")))?;

        if email.reset_url.contains('\r') || email.reset_url.contains('\n') {
            return Err(EmailError::Build(
                "Password reset URL contains invalid control characters".to_string(),
            ));
        }

        let escaped_url = html_escape(&email.reset_url);

        let body = MultiPart::alternative()
            .singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(format!(
                        "You requested a password reset for RusToK.\nUse the following link to reset your password:\n{}\n",
                        email.reset_url
                    )),
            )
            .singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_HTML)
                    .body(format!(
                        "<p>You requested a password reset for RusToK.</p><p><a href=\"{}\">Reset password</a></p>",
                        escaped_url
                    )),
            );

        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject("RusToK password reset")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_reset_base_url(reset_base_url: &str) -> EmailConfig {
        EmailConfig {
            reset_base_url: reset_base_url.to_string(),
            ..EmailConfig::default()
        }
    }

    #[test]
    fn password_reset_email_debug_redacts_reset_token() {
        let email = PasswordResetEmail {
            to: "user@example.com".to_string(),
            reset_url: "https://example.test/reset?token=super-secret-reset-token".to_string(),
        };
        let rendered = format!("{email:?}");

        assert!(!rendered.contains("super-secret-reset-token"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn password_reset_url_percent_encodes_token() {
        let config = config_with_reset_base_url("https://admin.example.test/reset-password");

        let url = EmailService::password_reset_url(&config, "abc+/=&?");

        assert_eq!(
            url,
            "https://admin.example.test/reset-password?token=abc%2B%2F%3D%26%3F"
        );
    }

    #[test]
    fn password_reset_url_preserves_existing_query() {
        let config =
            config_with_reset_base_url("https://admin.example.test/reset-password?tenant=demo");

        let url = EmailService::password_reset_url(&config, "reset-token");

        assert_eq!(
            url,
            "https://admin.example.test/reset-password?tenant=demo&token=reset-token"
        );
    }

    #[test]
    fn password_reset_url_encodes_token_for_relative_fallback() {
        let config = config_with_reset_base_url("/reset-password?tenant=demo");

        let url = EmailService::password_reset_url(&config, "abc+/=&?");

        assert_eq!(url, "/reset-password?tenant=demo&token=abc%2B%2F%3D%26%3F");
    }

    #[test]
    fn password_reset_url_handles_trailing_separator() {
        let config_q = config_with_reset_base_url("/reset-password?");
        assert_eq!(
            EmailService::password_reset_url(&config_q, "my-token"),
            "/reset-password?token=my-token"
        );

        let config_amp = config_with_reset_base_url("/reset-password?tenant=demo&");
        assert_eq!(
            EmailService::password_reset_url(&config_amp, "my-token"),
            "/reset-password?tenant=demo&token=my-token"
        );
    }

    #[test]
    fn test_html_escape_special_characters() {
        assert_eq!(
            html_escape("https://example.test/reset?token=1&foo=<script>\"'"),
            "https://example.test/reset?token=1&amp;foo=&lt;script&gt;&quot;&#x27;"
        );
    }

    #[test]
    fn test_smtp_sender_preconditions_and_bounds() {
        // CRLF in from address
        let mut config = EmailConfig {
            enabled: true,
            from: "attacker@example.test\r\nBcc: evil@example.test".to_string(),
            ..EmailConfig::default()
        };
        assert!(matches!(
            SmtpEmailSender::try_new(&config),
            Err(EmailError::InvalidAddress(_))
        ));

        // Empty host
        config.from = "valid@example.test".to_string();
        config.smtp.host = "   ".to_string();
        assert!(matches!(
            SmtpEmailSender::try_new(&config),
            Err(EmailError::SmtpConfig(_))
        ));

        // Zero port
        config.smtp.host = "localhost".to_string();
        config.smtp.port = 0;
        assert!(matches!(
            SmtpEmailSender::try_new(&config),
            Err(EmailError::SmtpConfig(_))
        ));
    }

    struct DummyProvider;
    impl EmailTemplateProvider for DummyProvider {
        fn namespace(&self) -> &str {
            "test"
        }
        fn render(
            &self,
            _template_id: &str,
            _locale: &str,
            _vars: &serde_json::Value,
        ) -> Option<Result<RenderedEmail>> {
            None
        }
    }

    #[test]
    fn test_email_service_provider_registration() {
        let dummy = std::sync::Arc::new(DummyProvider);
        let service = EmailService::Disabled.with_provider(dummy.clone());
        assert!(matches!(service, EmailService::Disabled));

        let config = EmailConfig {
            enabled: true,
            from: "valid@example.test".to_string(),
            ..EmailConfig::default()
        };
        let mut smtp_service = EmailService::from_config(&config).expect("valid smtp config");
        smtp_service.add_provider(dummy);
        if let EmailService::Smtp(sender) = smtp_service {
            assert_eq!(sender.providers.len(), 1);
        } else {
            panic!("Expected EmailService::Smtp");
        }
    }

    #[tokio::test]
    async fn test_password_reset_crlf_rejections() {
        let config = EmailConfig {
            enabled: true,
            from: "valid@example.test".to_string(),
            ..EmailConfig::default()
        };
        let sender = SmtpEmailSender::try_new(&config).expect("valid config");

        // CRLF in recipient
        let bad_recipient = PasswordResetEmail {
            to: "user@example.test\r\nBcc: evil@example.test".to_string(),
            reset_url: "https://example.test/reset".to_string(),
        };
        assert!(matches!(
            sender.send_password_reset(bad_recipient).await,
            Err(EmailError::InvalidAddress(_))
        ));

        // CRLF in reset_url
        let bad_url = PasswordResetEmail {
            to: "user@example.test".to_string(),
            reset_url: "https://example.test/reset\r\n".to_string(),
        };
        assert!(matches!(
            sender.send_password_reset(bad_url).await,
            Err(EmailError::Build(_))
        ));
    }
}
