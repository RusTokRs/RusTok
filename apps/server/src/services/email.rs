// Re-export from rustok-email for backward compatibility.
pub use rustok_email::{EmailService, PasswordResetEmail, PasswordResetEmailSender};

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Cached SMTP transport stored in `shared_store` to reuse the connection pool.
#[derive(Clone)]
pub struct SharedSmtpEmailService(pub Arc<rustok_email::SmtpEmailSender>);

use async_trait::async_trait;
use rustok_email::{EmailError, RenderedEmail};

use crate::common::settings::EmailProvider;
use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;

static EMAIL_SEND_SUCCESS_TOTAL: AtomicU64 = AtomicU64::new(0);
static EMAIL_SEND_FAILURE_TOTAL: AtomicU64 = AtomicU64::new(0);
static EMAIL_SEND_SKIPPED_TOTAL: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EmailDeliveryMetricsSnapshot {
    pub success_total: u64,
    pub failure_total: u64,
    pub skipped_total: u64,
}

#[derive(Debug, Clone)]
pub struct EmailVerificationEmail {
    pub to: String,
    pub verification_token: String,
}

#[async_trait]
pub trait BuiltInAuthEmailSender: Send + Sync {
    async fn send_password_reset(
        &self,
        email: PasswordResetEmail,
    ) -> std::result::Result<(), EmailError>;

    async fn send_email_verification(
        &self,
        email: EmailVerificationEmail,
    ) -> std::result::Result<(), EmailError>;
}

#[derive(Default)]
struct DisabledBuiltInAuthEmailSender;

pub fn email_delivery_metrics_snapshot() -> EmailDeliveryMetricsSnapshot {
    EmailDeliveryMetricsSnapshot {
        success_total: EMAIL_SEND_SUCCESS_TOTAL.load(Ordering::Relaxed),
        failure_total: EMAIL_SEND_FAILURE_TOTAL.load(Ordering::Relaxed),
        skipped_total: EMAIL_SEND_SKIPPED_TOTAL.load(Ordering::Relaxed),
    }
}

fn record_email_send_result(result: &std::result::Result<(), EmailError>) {
    match result {
        Ok(()) => {
            EMAIL_SEND_SUCCESS_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        Err(_) => {
            EMAIL_SEND_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn record_email_send_skipped() {
    EMAIL_SEND_SKIPPED_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Server error bridge: convert `EmailError` into the host error type.
pub fn email_err(err: EmailError) -> Error {
    Error::Message(err.to_string())
}

/// Build password reset URL from settings + token.
pub fn password_reset_url(ctx: &ServerRuntimeContext, token: &str) -> Result<String> {
    let config = rustok_email::EmailConfig {
        reset_base_url: ctx.settings().email.reset_base_url.clone(),
        ..Default::default()
    };

    Ok(rustok_email::EmailService::password_reset_url(
        &config, token,
    ))
}

// ── Template rendering for the built-in auth emails ─────────────────────────

/// Embedded Tera template strings for auth emails (compiled in at build time).
mod templates {
    pub const PASSWORD_RESET_EN_SUBJECT: &str =
        include_str!("../mailers/auth/password_reset/en/subject.t");
    pub const PASSWORD_RESET_EN_TEXT: &str =
        include_str!("../mailers/auth/password_reset/en/text.t");
    pub const PASSWORD_RESET_EN_HTML: &str =
        include_str!("../mailers/auth/password_reset/en/html.t");

    pub const PASSWORD_RESET_RU_SUBJECT: &str =
        include_str!("../mailers/auth/password_reset/ru/subject.t");
    pub const PASSWORD_RESET_RU_TEXT: &str =
        include_str!("../mailers/auth/password_reset/ru/text.t");
    pub const PASSWORD_RESET_RU_HTML: &str =
        include_str!("../mailers/auth/password_reset/ru/html.t");

    pub const EMAIL_VERIFICATION_EN_SUBJECT: &str =
        include_str!("../mailers/auth/email_verification/en/subject.t");
    pub const EMAIL_VERIFICATION_EN_TEXT: &str =
        include_str!("../mailers/auth/email_verification/en/text.t");
    pub const EMAIL_VERIFICATION_EN_HTML: &str =
        include_str!("../mailers/auth/email_verification/en/html.t");

    pub const EMAIL_VERIFICATION_RU_SUBJECT: &str =
        include_str!("../mailers/auth/email_verification/ru/subject.t");
    pub const EMAIL_VERIFICATION_RU_TEXT: &str =
        include_str!("../mailers/auth/email_verification/ru/text.t");
    pub const EMAIL_VERIFICATION_RU_HTML: &str =
        include_str!("../mailers/auth/email_verification/ru/html.t");
}

type AuthTemplateTriple = (&'static str, &'static str, &'static str);

fn localized_auth_templates(
    locale: &str,
    en: AuthTemplateTriple,
    ru: AuthTemplateTriple,
) -> AuthTemplateTriple {
    if locale.starts_with("ru") { ru } else { en }
}

/// Render the password-reset email for the given locale.
///
/// Falls back to English for unknown locales.
pub fn render_password_reset(
    locale: &str,
    reset_url: &str,
) -> std::result::Result<RenderedEmail, EmailError> {
    use rustok_email::template::render_tera_string;

    let vars = serde_json::json!({ "reset_url": reset_url });

    let (subj_t, text_t, html_t) = localized_auth_templates(
        locale,
        (
            templates::PASSWORD_RESET_EN_SUBJECT,
            templates::PASSWORD_RESET_EN_TEXT,
            templates::PASSWORD_RESET_EN_HTML,
        ),
        (
            templates::PASSWORD_RESET_RU_SUBJECT,
            templates::PASSWORD_RESET_RU_TEXT,
            templates::PASSWORD_RESET_RU_HTML,
        ),
    );

    Ok(RenderedEmail {
        subject: render_tera_string(subj_t.trim(), &vars)?,
        text: render_tera_string(text_t, &vars)?,
        html: render_tera_string(html_t, &vars)?,
    })
}

pub fn render_email_verification(
    locale: &str,
    verification_token: &str,
) -> std::result::Result<RenderedEmail, EmailError> {
    use rustok_email::template::render_tera_string;

    let vars = serde_json::json!({ "verification_token": verification_token });

    let (subj_t, text_t, html_t) = localized_auth_templates(
        locale,
        (
            templates::EMAIL_VERIFICATION_EN_SUBJECT,
            templates::EMAIL_VERIFICATION_EN_TEXT,
            templates::EMAIL_VERIFICATION_EN_HTML,
        ),
        (
            templates::EMAIL_VERIFICATION_RU_SUBJECT,
            templates::EMAIL_VERIFICATION_RU_TEXT,
            templates::EMAIL_VERIFICATION_RU_HTML,
        ),
    );

    Ok(RenderedEmail {
        subject: render_tera_string(subj_t.trim(), &vars)?,
        text: render_tera_string(text_t, &vars)?,
        html: render_tera_string(html_t, &vars)?,
    })
}

// ── SMTP mailer adapter ──────────────────────────────────────────────────────

/// SMTP bridge that preserves app-level localized templates while reusing the
/// shared SMTP transport across requests.
pub struct TemplatedSmtpMailerAdapter {
    sender: Arc<rustok_email::SmtpEmailSender>,
    locale: String,
}

impl TemplatedSmtpMailerAdapter {
    pub fn new(sender: Arc<rustok_email::SmtpEmailSender>, locale: impl Into<String>) -> Self {
        Self {
            sender,
            locale: locale.into(),
        }
    }
}

#[async_trait]
impl BuiltInAuthEmailSender for DisabledBuiltInAuthEmailSender {
    async fn send_password_reset(
        &self,
        email: PasswordResetEmail,
    ) -> std::result::Result<(), EmailError> {
        tracing::info!(
            recipient = %email.to,
            "Password reset email provider disabled; skipping outbound send"
        );
        record_email_send_skipped();
        Ok(())
    }

    async fn send_email_verification(
        &self,
        email: EmailVerificationEmail,
    ) -> std::result::Result<(), EmailError> {
        tracing::info!(
            recipient = %email.to,
            "Email verification provider disabled; skipping outbound send"
        );
        record_email_send_skipped();
        Ok(())
    }
}

#[async_trait]
impl BuiltInAuthEmailSender for TemplatedSmtpMailerAdapter {
    async fn send_password_reset(
        &self,
        email: PasswordResetEmail,
    ) -> std::result::Result<(), EmailError> {
        let rendered = match render_password_reset(&self.locale, &email.reset_url) {
            Ok(rendered) => rendered,
            Err(error) => {
                let result = Err(error);
                record_email_send_result(&result);
                return result;
            }
        };
        let result = self.sender.send_rendered(&email.to, &rendered).await;
        record_email_send_result(&result);
        result
    }

    async fn send_email_verification(
        &self,
        email: EmailVerificationEmail,
    ) -> std::result::Result<(), EmailError> {
        let rendered = match render_email_verification(&self.locale, &email.verification_token) {
            Ok(rendered) => rendered,
            Err(error) => {
                let result = Err(error);
                record_email_send_result(&result);
                return result;
            }
        };
        let result = self.sender.send_rendered(&email.to, &rendered).await;
        record_email_send_result(&result);
        result
    }
}

// ── Factory ──────────────────────────────────────────────────────────────────

/// Build a localized built-in auth email sender from server runtime context.
///
/// `locale` is used to render localized built-in auth templates. The underlying SMTP transport is cached in
/// `shared_store` to reuse the connection pool.
///
/// Dispatches on `email.provider`:
/// - `smtp` (default) → localized SMTP adapter over cached `SmtpEmailSender`
/// - `none` → `EmailService::Disabled`
pub fn email_service_from_ctx(
    ctx: &ServerRuntimeContext,
    locale: &str,
) -> Result<Box<dyn BuiltInAuthEmailSender>> {
    let settings = ctx.settings();

    match settings.email.provider {
        EmailProvider::None => Ok(Box::new(DisabledBuiltInAuthEmailSender)),

        EmailProvider::Smtp => {
            // Return cached transport if already initialised (connection pool reuse).
            if let Some(shared) = ctx.shared_get::<SharedSmtpEmailService>() {
                return Ok(Box::new(TemplatedSmtpMailerAdapter::new(
                    shared.0.clone(),
                    locale,
                )));
            }

            let config = rustok_email::EmailConfig {
                enabled: settings.email.enabled,
                smtp: rustok_email::SmtpConfig {
                    host: settings.email.smtp.host.clone(),
                    port: settings.email.smtp.port,
                    username: settings.email.smtp.username.clone(),
                    password: settings.email.smtp.password.clone(),
                },
                from: settings.email.from.clone(),
                reset_base_url: settings.email.reset_base_url.clone(),
            };
            let service = EmailService::from_config(&config).map_err(email_err)?;
            let EmailService::Smtp(sender) = service else {
                return Ok(Box::new(DisabledBuiltInAuthEmailSender));
            };
            let sender = Arc::new(*sender);
            ctx.shared_insert(SharedSmtpEmailService(sender.clone()));
            Ok(Box::new(TemplatedSmtpMailerAdapter::new(sender, locale)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_password_reset_en_contains_url() {
        let rendered = render_password_reset("en", "https://example.com/reset?token=abc").unwrap();
        assert!(
            rendered
                .html
                .contains("https://example.com/reset?token=abc")
        );
        assert!(
            rendered
                .text
                .contains("https://example.com/reset?token=abc")
        );
        assert!(!rendered.subject.is_empty());
    }

    #[test]
    fn render_password_reset_ru_contains_url() {
        let rendered = render_password_reset("ru", "https://example.com/reset?token=xyz").unwrap();
        assert!(
            rendered
                .html
                .contains("https://example.com/reset?token=xyz")
        );
        assert!(
            rendered
                .text
                .contains("https://example.com/reset?token=xyz")
        );
        assert!(!rendered.subject.is_empty());
    }

    #[test]
    fn render_password_reset_unknown_locale_falls_back_to_en() {
        let en = render_password_reset("en", "https://x.com/r").unwrap();
        let de = render_password_reset("de", "https://x.com/r").unwrap();
        assert_eq!(
            en.subject, de.subject,
            "unknown locale should fall back to English subject"
        );
    }

    #[test]
    fn render_password_reset_ru_subject_is_non_empty() {
        let rendered = render_password_reset("ru", "https://x.com/r").unwrap();
        assert!(!rendered.subject.trim().is_empty());
    }

    #[test]
    fn render_password_reset_regional_russian_locale_uses_russian_templates() {
        let base = render_password_reset("ru", "https://x.com/r").unwrap();
        let regional = render_password_reset("ru-RU", "https://x.com/r").unwrap();

        assert_eq!(regional.subject, base.subject);
        assert_eq!(regional.text, base.text);
        assert_eq!(regional.html, base.html);
    }

    #[test]
    fn render_email_verification_en_contains_token() {
        let rendered = render_email_verification("en", "verify-token-123").unwrap();
        assert!(rendered.html.contains("verify-token-123"));
        assert!(rendered.text.contains("verify-token-123"));
        assert!(!rendered.subject.is_empty());
    }

    #[test]
    fn render_email_verification_ru_contains_token() {
        let rendered = render_email_verification("ru", "verify-token-xyz").unwrap();
        assert!(rendered.html.contains("verify-token-xyz"));
        assert!(rendered.text.contains("verify-token-xyz"));
        assert!(!rendered.subject.is_empty());
    }

    #[test]
    fn render_email_verification_unknown_locale_falls_back_to_en() {
        let en = render_email_verification("en", "verify-token").unwrap();
        let de = render_email_verification("de", "verify-token").unwrap();
        assert_eq!(
            en.subject, de.subject,
            "unknown locale should fall back to English subject"
        );
    }

    #[test]
    fn render_email_verification_regional_russian_locale_uses_russian_templates() {
        let base = render_email_verification("ru", "verify-token").unwrap();
        let regional = render_email_verification("ru-RU", "verify-token").unwrap();

        assert_eq!(regional.subject, base.subject);
        assert_eq!(regional.text, base.text);
        assert_eq!(regional.html, base.html);
    }

    #[tokio::test]
    async fn disabled_email_sender_records_skipped_metric() {
        let sender = DisabledBuiltInAuthEmailSender;
        let before = email_delivery_metrics_snapshot();

        sender
            .send_password_reset(PasswordResetEmail {
                to: "user@example.test".to_string(),
                reset_url: "https://example.test/reset?token=t".to_string(),
            })
            .await
            .unwrap();
        sender
            .send_email_verification(EmailVerificationEmail {
                to: "user@example.test".to_string(),
                verification_token: "verify-token".to_string(),
            })
            .await
            .unwrap();

        let after = email_delivery_metrics_snapshot();
        assert_eq!(after.skipped_total, before.skipped_total + 2);
        assert_eq!(after.success_total, before.success_total);
        assert_eq!(after.failure_total, before.failure_total);
    }
}
