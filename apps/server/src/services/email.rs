// Re-export from rustok-email for backward compatibility.
pub use rustok_email::{EmailService, PasswordResetEmail, PasswordResetEmailSender};

use std::sync::Arc;

/// Cached SMTP email service stored in `shared_store` to reuse the connection pool.
#[derive(Clone)]
pub struct SharedSmtpEmailService(pub Arc<EmailService>);

use async_trait::async_trait;
use loco_rs::app::AppContext;
use loco_rs::mailer::{Email, EmailSender};
use rustok_email::{EmailError, RenderedEmail};

use crate::common::settings::{EmailProvider, RustokSettings};
use crate::error::{Error, Result};

/// Loco bridge: convert `EmailError` → `loco_rs::Error`.
pub fn email_err(err: EmailError) -> Error {
    Error::Message(err.to_string())
}

/// Build password reset URL from settings + token.
pub fn password_reset_url(ctx: &AppContext, token: &str) -> Result<String> {
    let settings = RustokSettings::from_settings(&ctx.config.settings)
        .map_err(|e| Error::Message(e.to_string()))?;

    let config = rustok_email::EmailConfig {
        reset_base_url: settings.email.reset_base_url.clone(),
        ..Default::default()
    };

    Ok(rustok_email::EmailService::password_reset_url(
        &config, token,
    ))
}

// ── Template rendering for the built-in auth emails ─────────────────────────

/// Embedded Tera template strings for auth emails (compiled in at build time).
mod templates {
    pub const EN_SUBJECT: &str = include_str!("../mailers/auth/password_reset/en/subject.t");
    pub const EN_TEXT: &str = include_str!("../mailers/auth/password_reset/en/text.t");
    pub const EN_HTML: &str = include_str!("../mailers/auth/password_reset/en/html.t");

    pub const RU_SUBJECT: &str = include_str!("../mailers/auth/password_reset/ru/subject.t");
    pub const RU_TEXT: &str = include_str!("../mailers/auth/password_reset/ru/text.t");
    pub const RU_HTML: &str = include_str!("../mailers/auth/password_reset/ru/html.t");
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

    let (subj_t, text_t, html_t) = if locale.starts_with("ru") {
        (
            templates::RU_SUBJECT,
            templates::RU_TEXT,
            templates::RU_HTML,
        )
    } else {
        (
            templates::EN_SUBJECT,
            templates::EN_TEXT,
            templates::EN_HTML,
        )
    };

    Ok(RenderedEmail {
        subject: render_tera_string(subj_t.trim(), &vars)?,
        text: render_tera_string(text_t, &vars)?,
        html: render_tera_string(html_t, &vars)?,
    })
}

// ── Loco Mailer adapter ──────────────────────────────────────────────────────

/// Sends emails via Loco's `ctx.mailer` (`EmailSender`) and Tera templates.
///
/// Use this when `email.provider = "loco"` in settings.  The `ctx.mailer`
/// field must be initialised before use (done in `after_context()` in `app.rs`).
pub struct LocoMailerAdapter {
    mailer: EmailSender,
    from: String,
    locale: String,
}

impl LocoMailerAdapter {
    pub fn new(mailer: EmailSender, from: impl Into<String>, locale: impl Into<String>) -> Self {
        Self {
            mailer,
            from: from.into(),
            locale: locale.into(),
        }
    }
}

#[async_trait]
impl PasswordResetEmailSender for LocoMailerAdapter {
    async fn send_password_reset(
        &self,
        email: PasswordResetEmail,
    ) -> std::result::Result<(), EmailError> {
        let rendered = render_password_reset(&self.locale, &email.reset_url)?;

        let msg = Email {
            from: Some(self.from.clone()),
            to: email.to,
            reply_to: None,
            subject: rendered.subject,
            text: rendered.text,
            html: rendered.html,
            bcc: None,
            cc: None,
        };

        self.mailer
            .mail(&msg)
            .await
            .map_err(|e| EmailError::Send(e.to_string()))
    }
}

// ── Factory ──────────────────────────────────────────────────────────────────

/// Build a `PasswordResetEmailSender` from `AppContext`.
///
/// `locale` is used only when `email.provider = "loco"` (Tera template rendering).
/// For `smtp`, the Smtp transport is cached in `shared_store` to reuse the connection pool.
///
/// Dispatches on `email.provider`:
/// - `loco` → `LocoMailerAdapter` with per-request locale (requires `ctx.mailer` initialized)
/// - `smtp` (default) → `EmailService::Smtp` cached in `shared_store`
/// - `none` → `EmailService::Disabled`
pub fn email_service_from_ctx(
    ctx: &AppContext,
    locale: &str,
) -> Result<Box<dyn PasswordResetEmailSender>> {
    let settings = RustokSettings::from_settings(&ctx.config.settings)
        .map_err(|e| Error::Message(e.to_string()))?;

    match settings.email.provider {
        EmailProvider::None => Ok(Box::new(EmailService::Disabled)),

        EmailProvider::Loco => {
            // Cannot cache: LocoMailerAdapter carries a per-request locale.
            let Some(mailer) = ctx.mailer.clone() else {
                tracing::warn!(
                    "email.provider = \"loco\" but ctx.mailer is not initialized; \
                     falling back to disabled"
                );
                return Ok(Box::new(EmailService::Disabled));
            };
            Ok(Box::new(LocoMailerAdapter::new(
                mailer,
                settings.email.from,
                locale,
            )))
        }

        EmailProvider::Smtp => {
            // Return cached transport if already initialised (connection pool reuse).
            if let Some(shared) = ctx.shared_store.get::<SharedSmtpEmailService>() {
                return Ok(Box::new((*shared.0).clone()));
            }

            let config = rustok_email::EmailConfig {
                enabled: settings.email.enabled,
                smtp: rustok_email::SmtpConfig {
                    host: settings.email.smtp.host,
                    port: settings.email.smtp.port,
                    username: settings.email.smtp.username,
                    password: settings.email.smtp.password,
                },
                from: settings.email.from,
                reset_base_url: settings.email.reset_base_url,
            };
            let service = EmailService::from_config(&config).map_err(email_err)?;
            ctx.shared_store
                .insert(SharedSmtpEmailService(Arc::new(service.clone())));
            Ok(Box::new(service))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_password_reset_en_contains_url() {
        let rendered = render_password_reset("en", "https://example.com/reset?token=abc").unwrap();
        assert!(rendered
            .html
            .contains("https://example.com/reset?token=abc"));
        assert!(rendered
            .text
            .contains("https://example.com/reset?token=abc"));
        assert!(!rendered.subject.is_empty());
    }

    #[test]
    fn render_password_reset_ru_contains_url() {
        let rendered = render_password_reset("ru", "https://example.com/reset?token=xyz").unwrap();
        assert!(rendered
            .html
            .contains("https://example.com/reset?token=xyz"));
        assert!(rendered
            .text
            .contains("https://example.com/reset?token=xyz"));
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
}
