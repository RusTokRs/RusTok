// Re-export from rustok-email for backward compatibility.
pub use rustok_email::{EmailService, PasswordResetEmail, PasswordResetEmailSender};

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::AbortHandle;
use uuid::Uuid;

/// Cached SMTP transport stored in `shared_store` to reuse the connection pool.
#[derive(Clone)]
pub struct SharedSmtpEmailService(pub Arc<rustok_email::SmtpEmailSender>);

use crate::models::users;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_email::{EmailError, RenderedEmail};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

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

#[derive(Clone)]
pub struct EmailChangeConfirmationEmail {
    pub to: String,
    pub token: String,
}

#[derive(Clone)]
pub struct EmailChangeNotificationEmail {
    pub to: String,
    pub new_email: String,
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

    async fn send_email_change_confirmation(
        &self,
        email: EmailChangeConfirmationEmail,
    ) -> std::result::Result<(), EmailError>;

    async fn send_email_change_notification(
        &self,
        email: EmailChangeNotificationEmail,
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

    pub const EMAIL_CHANGE_CONFIRMATION_EN_SUBJECT: &str =
        include_str!("../mailers/auth/email_change_confirmation/en/subject.t");
    pub const EMAIL_CHANGE_CONFIRMATION_EN_TEXT: &str =
        include_str!("../mailers/auth/email_change_confirmation/en/text.t");
    pub const EMAIL_CHANGE_CONFIRMATION_EN_HTML: &str =
        include_str!("../mailers/auth/email_change_confirmation/en/html.t");
    pub const EMAIL_CHANGE_CONFIRMATION_RU_SUBJECT: &str =
        include_str!("../mailers/auth/email_change_confirmation/ru/subject.t");
    pub const EMAIL_CHANGE_CONFIRMATION_RU_TEXT: &str =
        include_str!("../mailers/auth/email_change_confirmation/ru/text.t");
    pub const EMAIL_CHANGE_CONFIRMATION_RU_HTML: &str =
        include_str!("../mailers/auth/email_change_confirmation/ru/html.t");

    pub const EMAIL_CHANGE_NOTIFICATION_EN_SUBJECT: &str =
        include_str!("../mailers/auth/email_change_notification/en/subject.t");
    pub const EMAIL_CHANGE_NOTIFICATION_EN_TEXT: &str =
        include_str!("../mailers/auth/email_change_notification/en/text.t");
    pub const EMAIL_CHANGE_NOTIFICATION_EN_HTML: &str =
        include_str!("../mailers/auth/email_change_notification/en/html.t");
    pub const EMAIL_CHANGE_NOTIFICATION_RU_SUBJECT: &str =
        include_str!("../mailers/auth/email_change_notification/ru/subject.t");
    pub const EMAIL_CHANGE_NOTIFICATION_RU_TEXT: &str =
        include_str!("../mailers/auth/email_change_notification/ru/text.t");
    pub const EMAIL_CHANGE_NOTIFICATION_RU_HTML: &str =
        include_str!("../mailers/auth/email_change_notification/ru/html.t");
}

type AuthTemplateTriple = (&'static str, &'static str, &'static str);

fn localized_auth_templates(
    locale: &str,
    en: AuthTemplateTriple,
    ru: AuthTemplateTriple,
) -> AuthTemplateTriple {
    // Match the BCP 47 language subtag, not an arbitrary string prefix.
    // Values such as "ruby" are not Russian locales; case variants such as
    // "RU-RU" are valid and should still use the Russian templates.
    let language = locale.split(['-', '_']).next().unwrap_or("");
    if language.eq_ignore_ascii_case("ru") {
        ru
    } else {
        en
    }
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

pub fn render_email_change_confirmation(
    locale: &str,
    token: &str,
) -> std::result::Result<RenderedEmail, EmailError> {
    use rustok_email::template::render_tera_string;

    let vars = serde_json::json!({ "token": token });
    let (subject, text, html) = localized_auth_templates(
        locale,
        (
            templates::EMAIL_CHANGE_CONFIRMATION_EN_SUBJECT,
            templates::EMAIL_CHANGE_CONFIRMATION_EN_TEXT,
            templates::EMAIL_CHANGE_CONFIRMATION_EN_HTML,
        ),
        (
            templates::EMAIL_CHANGE_CONFIRMATION_RU_SUBJECT,
            templates::EMAIL_CHANGE_CONFIRMATION_RU_TEXT,
            templates::EMAIL_CHANGE_CONFIRMATION_RU_HTML,
        ),
    );

    Ok(RenderedEmail {
        subject: render_tera_string(subject.trim(), &vars)?,
        text: render_tera_string(text, &vars)?,
        html: render_tera_string(html, &vars)?,
    })
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn render_email_change_notification(
    locale: &str,
    new_email: &str,
) -> std::result::Result<RenderedEmail, EmailError> {
    use rustok_email::template::render_tera_string;

    let vars = serde_json::json!({
        "new_email": new_email,
        "new_email_html": escape_html(new_email),
    });
    let (subject, text, html) = localized_auth_templates(
        locale,
        (
            templates::EMAIL_CHANGE_NOTIFICATION_EN_SUBJECT,
            templates::EMAIL_CHANGE_NOTIFICATION_EN_TEXT,
            templates::EMAIL_CHANGE_NOTIFICATION_EN_HTML,
        ),
        (
            templates::EMAIL_CHANGE_NOTIFICATION_RU_SUBJECT,
            templates::EMAIL_CHANGE_NOTIFICATION_RU_TEXT,
            templates::EMAIL_CHANGE_NOTIFICATION_RU_HTML,
        ),
    );

    Ok(RenderedEmail {
        subject: render_tera_string(subject.trim(), &vars)?,
        text: render_tera_string(text, &vars)?,
        html: render_tera_string(html, &vars)?,
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
        _email: PasswordResetEmail,
    ) -> std::result::Result<(), EmailError> {
        tracing::info!("Password reset email provider disabled; skipping outbound send");
        record_email_send_skipped();
        Ok(())
    }

    async fn send_email_verification(
        &self,
        _email: EmailVerificationEmail,
    ) -> std::result::Result<(), EmailError> {
        tracing::info!("Email verification provider disabled; skipping outbound send");
        record_email_send_skipped();
        Ok(())
    }

    async fn send_email_change_confirmation(
        &self,
        _email: EmailChangeConfirmationEmail,
    ) -> std::result::Result<(), EmailError> {
        tracing::info!("Email change confirmation provider disabled; skipping outbound send");
        record_email_send_skipped();
        Ok(())
    }

    async fn send_email_change_notification(
        &self,
        _email: EmailChangeNotificationEmail,
    ) -> std::result::Result<(), EmailError> {
        tracing::info!("Email change notification provider disabled; skipping outbound send");
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

    async fn send_email_change_confirmation(
        &self,
        email: EmailChangeConfirmationEmail,
    ) -> std::result::Result<(), EmailError> {
        let rendered = match render_email_change_confirmation(&self.locale, &email.token) {
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

    async fn send_email_change_notification(
        &self,
        email: EmailChangeNotificationEmail,
    ) -> std::result::Result<(), EmailError> {
        let rendered = match render_email_change_notification(&self.locale, &email.new_email) {
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

/// Verification-token lifetime shared by registration and resend flows.
pub const EMAIL_VERIFICATION_TOKEN_TTL_SECS: u64 = 24 * 60 * 60;

/// Password-reset token lifetime shared by REST and GraphQL flows.
pub const PASSWORD_RESET_TOKEN_TTL_SECS: u64 = 15 * 60;

/// Email-change confirmation challenges are short-lived and one-time.
pub const EMAIL_CHANGE_TOKEN_TTL_SECS: u64 = 30 * 60;

/// Tracks best-effort authentication email sends in the host runtime. The
/// registry remains installed after shutdown so a concurrent request cannot
/// create a fresh, unobserved registry while the drain is in progress.
struct AuthEmailTaskRegistry {
    state: Mutex<AuthEmailTaskState>,
    active_tasks: watch::Sender<usize>,
}

#[derive(Default)]
struct AuthEmailTaskState {
    closed: bool,
    handles: HashMap<Uuid, AbortHandle>,
}

impl AuthEmailTaskRegistry {
    fn new() -> Self {
        let (active_tasks, _receiver) = watch::channel(0);
        Self {
            state: Mutex::new(AuthEmailTaskState::default()),
            active_tasks,
        }
    }

    fn finish(&self, task_id: Uuid) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.handles.remove(&task_id).is_some() {
            self.active_tasks.send_replace(state.handles.len());
        }
    }

    fn close(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed = true;
    }

    fn abort_remaining(&self) -> usize {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for handle in state.handles.values() {
            handle.abort();
        }
        state.handles.len()
    }
}

/// Runs even when a task is cancelled before its first poll. Otherwise a
/// pre-poll abort would strand the registry's active count indefinitely.
struct AuthEmailTaskCompletion {
    registry: Arc<AuthEmailTaskRegistry>,
    task_id: Uuid,
}

impl Drop for AuthEmailTaskCompletion {
    fn drop(&mut self) {
        self.registry.finish(self.task_id);
    }
}

async fn wait_for_auth_email_idle(active: &mut watch::Receiver<usize>) {
    loop {
        if *active.borrow_and_update() == 0 {
            return;
        }
        if active.changed().await.is_err() {
            return;
        }
    }
}

/// Cooperatively waits for accepted auth sends after host shutdown. A bounded
/// timeout aborts blocked sends; this is not durable delivery across crashes.
pub(crate) async fn drain_auth_email_tasks(ctx: &ServerRuntimeContext) {
    let registry = ctx.shared_get_or_insert_with(|| Arc::new(AuthEmailTaskRegistry::new()));
    let mut active = registry.active_tasks.subscribe();
    registry.close();
    let pending = *active.borrow();
    if pending == 0 {
        return;
    }

    tracing::info!(pending, "Waiting for in-flight auth email deliveries");
    if tokio::time::timeout(
        Duration::from_secs(5),
        wait_for_auth_email_idle(&mut active),
    )
    .await
    .is_err()
    {
        let aborted = registry.abort_remaining();
        tracing::warn!(
            aborted,
            error_code = "auth_email_shutdown_timeout",
            "Timed out draining auth email deliveries; cancelling remaining tasks"
        );
        if tokio::time::timeout(
            Duration::from_secs(1),
            wait_for_auth_email_idle(&mut active),
        )
        .await
        .is_err()
        {
            tracing::warn!(
                error_code = "auth_email_shutdown_cancellation_timeout",
                "Some cancelled auth email tasks did not finish during shutdown"
            );
        }
    }
}

/// Dispatches a tracked best-effort email task. Return a normal Tokio join
/// handle for existing callers, but retain independent cancellation ownership
/// even if those callers drop their handles. The shutdown fence is serialized
/// with task registration by the same registry mutex.
pub fn spawn_auth_email_task<F, Fut>(
    ctx: &ServerRuntimeContext,
    task_name: &'static str,
    future_factory: F,
) -> tokio::task::JoinHandle<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = std::result::Result<(), EmailError>> + Send + 'static,
{
    spawn_email_task(ctx, task_name, None, future_factory)
}

/// Credentials are checked at dispatch, not at task registration. JWTs are
/// revalidated against the current keyring; opaque challenges carry the
/// committed deadline. Never log the credential.
enum RecoveryEmailCredential {
    PasswordReset {
        config: crate::auth::AuthConfig,
        token: String,
        recipient: String,
    },
    Verification {
        config: crate::auth::AuthConfig,
        token: String,
        recipient: String,
    },
    EmailChange {
        tenant_id: Uuid,
        user_id: Uuid,
        recipient: String,
        token_hash: String,
        expires_at: DateTime<Utc>,
    },
}

fn recovery_email_deadline_live(
    verified_at: DateTime<Utc>,
    observed_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
) -> bool {
    verified_at <= observed_at && observed_at < expires_at
}

async fn recovery_email_admitted(
    db: &sea_orm::DatabaseConnection,
    credential: RecoveryEmailCredential,
) -> bool {
    // Fail fast on a local fault. The authoritative durable observation
    // belongs after the potentially blocking account/challenge reads.
    if rustok_auth::application_clock::check_local().is_err() {
        return false;
    }
    let window = match &credential {
        RecoveryEmailCredential::PasswordReset {
            config,
            token,
            recipient,
        } => {
            let Ok(claims) = crate::auth::decode_password_reset_token(config, token) else {
                return false;
            };
            let Ok(Some(user)) = users::Entity::find_by_id(claims.user_id)
                .filter(users::Column::TenantId.eq(claims.tenant_id))
                .one(db)
                .await
            else {
                return false;
            };
            if recipient != &claims.sub
                || !user.is_active()
                || !user.matches_recovery_identity(
                    claims.tenant_id,
                    claims.user_id,
                    &claims.sub,
                    claims.recovery_revision,
                )
                || !crate::auth::password_reset_credential_matches(
                    config,
                    &user.password_hash,
                    &claims.credential_fingerprint,
                )
            {
                return false;
            }
            Some((None, claims.expires_at))
        }
        RecoveryEmailCredential::Verification {
            config,
            token,
            recipient,
        } => {
            let Ok(claims) = crate::auth::decode_email_verification_token(config, token) else {
                return false;
            };
            let Ok(Some(user)) = users::Entity::find_by_id(claims.user_id)
                .filter(users::Column::TenantId.eq(claims.tenant_id))
                .one(db)
                .await
            else {
                return false;
            };
            if recipient != &claims.sub
                || !user.is_active()
                || user.is_email_verified()
                || !user.matches_recovery_identity(
                    claims.tenant_id,
                    claims.user_id,
                    &claims.sub,
                    claims.recovery_revision,
                )
            {
                return false;
            }
            Some((None, claims.expires_at))
        }
        RecoveryEmailCredential::EmailChange {
            tenant_id,
            user_id,
            recipient,
            token_hash,
            expires_at,
        } => {
            match super::auth_email_change::pending_email_change_window(
                db, *tenant_id, *user_id, recipient, token_hash,
            )
            .await
            {
                Ok(Some((created_at, stored))) => {
                    Some((Some(created_at), (*expires_at).min(stored)))
                }
                _ => None,
            }
        }
    };
    let Some((not_before, expires_at)) = window else {
        return false;
    };
    // Independently persist the final database-corroborated observation.
    // No domain transaction or business row lock is held during this wait.
    let Ok(verified_at) = rustok_auth::application_clock::admit_now(db).await else {
        return false;
    };
    if verified_at >= expires_at || not_before.is_some_and(|created_at| verified_at < created_at) {
        return false;
    }
    // The clock admission itself can wait. Revalidate the live JWT key
    // gate after that last async step and retain the original deadline.
    let still_valid = match &credential {
        RecoveryEmailCredential::PasswordReset { config, token, .. } => {
            crate::auth::decode_password_reset_token(config, token)
                .is_ok_and(|claims| claims.expires_at == expires_at)
        }
        RecoveryEmailCredential::Verification { config, token, .. } => {
            crate::auth::decode_email_verification_token(config, token)
                .is_ok_and(|claims| claims.expires_at == expires_at)
        }
        RecoveryEmailCredential::EmailChange { .. } => true,
    };
    let observed_at = Utc::now();
    still_valid
        && rustok_auth::application_clock::check_local().is_ok()
        && recovery_email_deadline_live(verified_at, observed_at, expires_at)
}

fn spawn_email_task<F, Fut>(
    ctx: &ServerRuntimeContext,
    task_name: &'static str,
    credential: Option<RecoveryEmailCredential>,
    future_factory: F,
) -> tokio::task::JoinHandle<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = std::result::Result<(), EmailError>> + Send + 'static,
{
    let db = ctx.db_clone();
    let stop_handle = crate::services::app_lifecycle::StopHandle::ensure(ctx);
    let registry = ctx.shared_get_or_insert_with(|| Arc::new(AuthEmailTaskRegistry::new()));
    let mut state = registry
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if stop_handle.is_stopping() || state.closed {
        drop(state);
        record_email_send_skipped();
        tracing::warn!(
            task = task_name,
            "Server runtime is stopping; skipping background email delivery"
        );
        return tokio::spawn(std::future::ready(()));
    }

    let task_id = Uuid::new_v4();
    let completion = AuthEmailTaskCompletion {
        registry: Arc::clone(&registry),
        task_id,
    };
    let handle = tokio::spawn(async move {
        use futures_util::FutureExt;
        use std::panic::AssertUnwindSafe;
        let _completion = completion;

        // Include future construction in the panic boundary: FnOnce itself
        // can panic before returning the asynchronous operation.
        let result = AssertUnwindSafe(async move {
            if let Some(credential) = credential {
                if !recovery_email_admitted(&db, credential).await {
                    return None;
                }
            }
            Some(future_factory().await)
        })
        .catch_unwind()
        .await;
        match result {
            Ok(None) => {
                record_email_send_skipped();
                tracing::warn!(
                    task = task_name,
                    error_code = "auth_email_credential_not_admitted",
                    "Skipping recovery email without a currently admissible credential"
                );
            }
            Ok(Some(Ok(()))) => {
                tracing::debug!(task = task_name, "Background email delivered successfully");
            }
            Ok(Some(Err(_error))) => {
                tracing::warn!(
                    task = task_name,
                    error_code = "auth_email_delivery_failed",
                    "Background email delivery failed"
                );
            }
            Err(_) => {
                tracing::error!(
                    task = task_name,
                    "Background email delivery panicked unexpectedly"
                );
            }
        }
    });
    state.handles.insert(task_id, handle.abort_handle());
    registry.active_tasks.send_replace(state.handles.len());
    handle
}

/// Spawns a monitored background task to deliver a password reset email.
pub fn spawn_password_reset_email(
    ctx: &ServerRuntimeContext,
    email_service: Box<dyn BuiltInAuthEmailSender>,
    recipient: String,
    reset_url: String,
    config: &crate::auth::AuthConfig,
    token: String,
) -> tokio::task::JoinHandle<()> {
    let credential = RecoveryEmailCredential::PasswordReset {
        config: config.clone(),
        token,
        recipient: recipient.clone(),
    };
    spawn_email_task(
        ctx,
        "send_password_reset",
        Some(credential),
        move || async move {
            email_service
                .send_password_reset(PasswordResetEmail {
                    to: recipient,
                    reset_url,
                })
                .await
        },
    )
}

/// Spawns a monitored background task to deliver an email verification email.
pub fn spawn_email_verification_send(
    ctx: &ServerRuntimeContext,
    email_service: Box<dyn BuiltInAuthEmailSender>,
    recipient: String,
    verification_token: String,
    config: &crate::auth::AuthConfig,
) -> tokio::task::JoinHandle<()> {
    let credential = RecoveryEmailCredential::Verification {
        config: config.clone(),
        token: verification_token.clone(),
        recipient: recipient.clone(),
    };
    spawn_email_task(
        ctx,
        "send_email_verification",
        Some(credential),
        move || async move {
            email_service
                .send_email_verification(EmailVerificationEmail {
                    to: recipient,
                    verification_token,
                })
                .await
        },
    )
}

/// Spawns a monitored email-change confirmation message.
pub fn spawn_email_change_confirmation_send(
    ctx: &ServerRuntimeContext,
    email_service: Box<dyn BuiltInAuthEmailSender>,
    recipient: String,
    token: String,
    expires_at: DateTime<Utc>,
    tenant_id: Uuid,
    user_id: Uuid,
) -> tokio::task::JoinHandle<()> {
    let credential = RecoveryEmailCredential::EmailChange {
        tenant_id,
        user_id,
        recipient: recipient.clone(),
        token_hash: crate::auth::hash_refresh_token(&token),
        expires_at,
    };
    spawn_email_task(
        ctx,
        "send_email_change_confirmation",
        Some(credential),
        move || async move {
            email_service
                .send_email_change_confirmation(EmailChangeConfirmationEmail {
                    to: recipient,
                    token,
                })
                .await
        },
    )
}

/// Spawns a monitored notification to the previously verified mailbox.
pub fn spawn_email_change_notification_send(
    ctx: &ServerRuntimeContext,
    email_service: Box<dyn BuiltInAuthEmailSender>,
    recipient: String,
    new_email: String,
) -> tokio::task::JoinHandle<()> {
    spawn_auth_email_task(ctx, "send_email_change_notification", move || async move {
        email_service
            .send_email_change_notification(EmailChangeNotificationEmail {
                to: recipient,
                new_email,
            })
            .await
    })
}

/// Best-effort verification email used by registration flows when the
/// platform requires verified addresses. Failures are logged, never
/// surfaced: the account exists and the resend endpoint stays available.
pub fn spawn_email_verification(
    ctx: &ServerRuntimeContext,
    config: &crate::auth::AuthConfig,
    user: &crate::models::users::Model,
    locale: &str,
) {
    let token = match crate::auth::encode_email_verification_token(
        config,
        user,
        EMAIL_VERIFICATION_TOKEN_TTL_SECS,
    ) {
        Ok(token) => token,
        Err(_error) => {
            tracing::warn!(
                error_code = "email_verification_token_encode_failed",
                "Failed to encode email verification token"
            );
            return;
        }
    };
    let email_service = match email_service_from_ctx(ctx, locale) {
        Ok(service) => service,
        Err(_error) => {
            tracing::warn!(
                error_code = "email_verification_sender_unavailable",
                "Failed to prepare email verification sender"
            );
            return;
        }
    };
    let recipient = user.email.clone();
    let _task = spawn_email_verification_send(ctx, email_service, recipient, token, config);
}

/// Build a localized built-in auth email sender from server runtime context.
///
/// `locale` is used to render localized built-in auth templates. The underlying SMTP transport is cached in
/// `shared_store` to reuse the connection pool.
///
/// Dispatches on `email.provider`:
/// - `smtp` (default) → localized SMTP adapter over cached `SmtpEmailSender`
/// - `none` → `EmailService::Disabled`
pub fn auth_email_delivery_enabled(ctx: &ServerRuntimeContext) -> bool {
    let settings = ctx.settings();
    settings.email.enabled && matches!(settings.email.provider, EmailProvider::Smtp)
}

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
            // Concurrent requests may create provisional transports, but all
            // callers must end up using the same cached pool. Never overwrite
            // a transport another request has already registered.
            let shared = ctx.shared_get_or_insert(SharedSmtpEmailService(Arc::new(*sender)));
            Ok(Box::new(TemplatedSmtpMailerAdapter::new(shared.0, locale)))
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
    fn auth_email_locale_matches_the_primary_language_not_a_prefix() {
        let en = render_password_reset("en", "https://example.test/reset").unwrap();
        for locale in ["ruby", "russian", "rus", "ruin"] {
            let rendered = render_password_reset(locale, "https://example.test/reset").unwrap();
            assert_eq!(rendered.subject, en.subject, "{locale} is not Russian");
        }

        let ru = render_password_reset("ru", "https://example.test/reset").unwrap();
        for locale in ["RU", "RU-RU", "ru_RU"] {
            let rendered = render_password_reset(locale, "https://example.test/reset").unwrap();
            assert_eq!(rendered.subject, ru.subject, "{locale} is Russian");
        }
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

    #[test]
    fn render_email_change_confirmation_uses_token_in_both_locales() {
        let en_token = "a".repeat(64);
        let ru_token = "b".repeat(64);
        let en = render_email_change_confirmation("en", &en_token).unwrap();
        let ru = render_email_change_confirmation("ru-RU", &ru_token).unwrap();
        assert!(en.text.contains(en_token.as_str()));
        assert!(en.html.contains(en_token.as_str()));
        assert!(ru.text.contains(ru_token.as_str()));
        assert!(ru.html.contains(ru_token.as_str()));
        assert_ne!(en.subject, ru.subject);
    }

    #[test]
    fn render_email_change_notification_escapes_html_content() {
        let rendered = render_email_change_notification("en", "new&<script>@example.test").unwrap();
        assert!(rendered.text.contains("new&<script>@example.test"));
        assert!(
            rendered
                .html
                .contains("new&amp;&lt;script&gt;@example.test")
        );
        assert!(!rendered.html.contains("<script>"));
        assert!(!rendered.subject.trim().is_empty());
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
        sender
            .send_email_change_confirmation(EmailChangeConfirmationEmail {
                to: "new@example.test".to_string(),
                token: "email-change-token".to_string(),
            })
            .await
            .unwrap();
        sender
            .send_email_change_notification(EmailChangeNotificationEmail {
                to: "old@example.test".to_string(),
                new_email: "new@example.test".to_string(),
            })
            .await
            .unwrap();

        let after = email_delivery_metrics_snapshot();
        assert_eq!(after.skipped_total, before.skipped_total + 4);
        assert_eq!(after.success_total, before.success_total);
        assert_eq!(after.failure_total, before.failure_total);
    }

    #[test]
    fn recovery_email_deadline_is_exclusive_and_rejects_backwards_observation() {
        let verified = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
        let expiry = verified + chrono::Duration::seconds(1);
        assert!(recovery_email_deadline_live(verified, verified, expiry));
        assert!(!recovery_email_deadline_live(verified, expiry, expiry));
        assert!(!recovery_email_deadline_live(
            verified,
            verified - chrono::Duration::milliseconds(1),
            expiry,
        ));
    }

    #[tokio::test]
    async fn recovery_email_dispatch_revalidates_account_and_clock() {
        use crate::common::settings::RustokSettings;
        use crate::models::{auth_email_change_requests, tenants};
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;
        use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Email tenant", "email-dispatch")
            .insert(&db)
            .await
            .unwrap();
        let user = users::ActiveModel::new(tenant.id, "owner@example.com", "old-hash")
            .insert(&db)
            .await
            .unwrap();
        let config = crate::auth::test_auth_config(
            "recovery-email-state-revalidation-test-secret".to_string(),
        );
        let reset = crate::auth::encode_password_reset_token(&config, &user, 900).unwrap();
        let verify = crate::auth::encode_email_verification_token(&config, &user, 900).unwrap();
        let reset_credential = |recipient: &str| RecoveryEmailCredential::PasswordReset {
            config: config.clone(),
            token: reset.clone(),
            recipient: recipient.to_string(),
        };
        let verify_credential = || RecoveryEmailCredential::Verification {
            config: config.clone(),
            token: verify.clone(),
            recipient: user.email.clone(),
        };
        let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let dispatch = |credential, expected| {
            let ctx = ctx.clone();
            async move {
                let (called_tx, mut called_rx) = tokio::sync::mpsc::channel(1);
                spawn_email_task(
                    &ctx,
                    "test_recovery",
                    Some(credential),
                    move || async move {
                        called_tx.send(()).await.unwrap();
                        Ok(())
                    },
                )
                .await
                .unwrap();
                assert_eq!(called_rx.try_recv().is_ok(), expected);
            }
        };
        dispatch(reset_credential(&user.email), true).await;
        dispatch(reset_credential("other@example.com"), false).await;
        dispatch(verify_credential(), true).await;

        // Exercise the opaque dispatch boundary with a real persisted
        // challenge. Neither the queued deadline nor the row may extend
        // the other. Deleting the exact challenge suppresses delivery.
        let now = Utc::now();
        let challenge = auth_email_change_requests::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            user_id: Set(user.id),
            current_email: Set(user.email.clone()),
            new_email: Set("new@example.com".to_string()),
            password_changed_at: Set(user.password_changed_at),
            recovery_revision: Set(Some(user.recovery_revision)),
            token_hash: Set("a".repeat(64)),
            expires_at: Set((now + chrono::Duration::minutes(5)).into()),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .unwrap();
        let opaque = |expires_at| RecoveryEmailCredential::EmailChange {
            tenant_id: tenant.id,
            user_id: user.id,
            recipient: challenge.new_email.clone(),
            token_hash: challenge.token_hash.clone(),
            expires_at,
        };
        dispatch(opaque(now + chrono::Duration::minutes(10)), true).await;
        dispatch(opaque(now - chrono::Duration::seconds(1)), false).await;
        auth_email_change_requests::ActiveModel {
            id: Set(challenge.id),
            created_at: Set((now + chrono::Duration::minutes(1)).into()),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        dispatch(opaque(now + chrono::Duration::minutes(10)), false).await;
        auth_email_change_requests::ActiveModel {
            id: Set(challenge.id),
            created_at: Set(now.into()),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        auth_email_change_requests::ActiveModel {
            id: Set(challenge.id),
            expires_at: Set((now - chrono::Duration::seconds(1)).into()),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        dispatch(opaque(now + chrono::Duration::minutes(10)), false).await;
        auth_email_change_requests::Entity::delete_by_id(challenge.id)
            .exec(&db)
            .await
            .unwrap();
        dispatch(opaque(now + chrono::Duration::minutes(10)), false).await;

        users::ActiveModel {
            id: Set(user.id),
            email_verified_at: Set(Some(Utc::now().into())),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        dispatch(verify_credential(), false).await;
        dispatch(reset_credential(&user.email), true).await;

        // Restoring the old hash must not resurrect either credential.
        for hash in ["new-hash", "old-hash"] {
            users::ActiveModel {
                id: Set(user.id),
                password_hash: Set(hash.to_string()),
                ..Default::default()
            }
            .update(&db)
            .await
            .unwrap();
            dispatch(reset_credential(&user.email), false).await;
        }
        // The clock failure fixture must retain a valid account/token;
        // a missing users table or stale revision would mask the clock gate.
        let current = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let fresh = crate::auth::encode_password_reset_token(&config, &current, 900).unwrap();
        let fresh_credential = || RecoveryEmailCredential::PasswordReset {
            config: config.clone(),
            token: fresh.clone(),
            recipient: current.email.clone(),
        };
        dispatch(fresh_credential(), true).await;
        db.execute_unprepared("DROP TABLE auth_application_clock")
            .await
            .unwrap();
        dispatch(fresh_credential(), false).await;
    }

    #[tokio::test]
    async fn spawn_auth_email_task_executes_and_logs() {
        use crate::common::settings::RustokSettings;
        use sea_orm::DatabaseConnection;

        let ctx =
            ServerRuntimeContext::new(DatabaseConnection::default(), RustokSettings::default());
        let (called_tx, mut called_rx) = tokio::sync::mpsc::channel(1);

        let handle = spawn_auth_email_task(&ctx, "test_task", move || async move {
            called_tx.send(()).await.unwrap();
            Ok(())
        });

        handle.await.unwrap();
        assert!(called_rx.recv().await.is_some());
    }

    #[tokio::test]
    async fn shutdown_drains_detached_email_and_blocks_new_work() {
        use crate::common::settings::RustokSettings;
        use crate::services::app_lifecycle::StopHandle;
        use sea_orm::DatabaseConnection;

        let ctx =
            ServerRuntimeContext::new(DatabaseConnection::default(), RustokSettings::default());
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        // Explicitly discard the returned handle, as real HTTP callers do.
        drop(spawn_auth_email_task(
            &ctx,
            "detached_send",
            move || async move {
                started_tx.send(()).expect("notify send started");
                release_rx.await.expect("release email");
                Ok(())
            },
        ));
        started_rx.await.expect("email started");

        StopHandle::ensure(&ctx).stop().await;
        let shutdown_ctx = ctx.clone();
        let drain = tokio::spawn(async move {
            drain_auth_email_tasks(&shutdown_ctx).await;
        });
        tokio::task::yield_now().await;
        assert!(!drain.is_finished(), "drain must await the detached send");

        let (late_tx, late_rx) = tokio::sync::oneshot::channel::<()>();
        spawn_auth_email_task(&ctx, "late_send", move || async move {
            let _ = late_tx.send(());
            Ok(())
        })
        .await
        .expect("late spawn returns a completed handle");
        assert!(
            late_rx.await.is_err(),
            "a stopped host must reject new sends"
        );

        release_tx.send(()).expect("complete existing send");
        drain.await.expect("drain completes");
        let registry = ctx
            .shared_get::<Arc<AuthEmailTaskRegistry>>()
            .expect("registry");
        assert_eq!(
            registry.state.lock().expect("registry mutex").handles.len(),
            0
        );
    }

    #[tokio::test]
    async fn cancellation_before_first_poll_releases_auth_email_registration() {
        use crate::common::settings::RustokSettings;
        use sea_orm::DatabaseConnection;

        let ctx =
            ServerRuntimeContext::new(DatabaseConnection::default(), RustokSettings::default());
        let handle = spawn_auth_email_task(&ctx, "cancelled_before_poll", || {
            std::future::pending::<std::result::Result<(), EmailError>>()
        });
        handle.abort();
        let _ = handle.await;
        drain_auth_email_tasks(&ctx).await;
        let registry = ctx
            .shared_get::<Arc<AuthEmailTaskRegistry>>()
            .expect("registry");
        assert_eq!(
            registry.state.lock().expect("registry mutex").handles.len(),
            0
        );
    }

    #[tokio::test]
    async fn email_factory_panics_are_contained_and_registry_is_released() {
        use crate::common::settings::RustokSettings;
        use sea_orm::DatabaseConnection;

        let ctx =
            ServerRuntimeContext::new(DatabaseConnection::default(), RustokSettings::default());
        let handle = spawn_auth_email_task(
            &ctx,
            "factory_panic",
            || -> std::future::Ready<std::result::Result<(), EmailError>> {
                panic!("deliberate factory panic");
            },
        );
        handle
            .await
            .expect("factory panic must be caught by the task");
        drain_auth_email_tasks(&ctx).await;
        let registry = ctx
            .shared_get::<Arc<AuthEmailTaskRegistry>>()
            .expect("registry");
        assert_eq!(
            registry.state.lock().expect("registry mutex").handles.len(),
            0
        );
    }

    #[tokio::test]
    async fn spawn_auth_email_task_skips_when_stopping() {
        use crate::common::settings::RustokSettings;
        use crate::services::app_lifecycle::StopHandle;
        use sea_orm::DatabaseConnection;

        let ctx =
            ServerRuntimeContext::new(DatabaseConnection::default(), RustokSettings::default());
        let stop_handle = StopHandle::ensure(&ctx);
        stop_handle.stop().await;

        let (called_tx, mut called_rx) = tokio::sync::mpsc::channel(1);
        let handle = spawn_auth_email_task(&ctx, "test_stopping_task", move || async move {
            called_tx.send(()).await.unwrap();
            Ok(())
        });

        handle.await.unwrap();
        assert!(called_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn spawn_auth_email_task_catches_panics_cleanly() {
        use crate::common::settings::RustokSettings;
        use sea_orm::DatabaseConnection;

        let ctx =
            ServerRuntimeContext::new(DatabaseConnection::default(), RustokSettings::default());
        let handle = spawn_auth_email_task(&ctx, "test_panicking_task", || async {
            panic!("deliberate panic for test");
            #[allow(unreachable_code)]
            Ok(())
        });

        // The join handle should resolve Ok(()) because the panic is caught inside the task
        let join_res = handle.await;
        assert!(join_res.is_ok());
    }
}
