use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

use crate::{EmailError, TransactionalEmailSender};

pub const DEFAULT_MAX_EMAIL_IDEMPOTENCY_ENTRIES: usize = 4_096;
pub const DEFAULT_EMAIL_IDEMPOTENCY_TTL: Duration = Duration::from_secs(24 * 60 * 60);
pub const MAX_EMAIL_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub const MAX_EMAIL_TENANT_ID_BYTES: usize = 128;
pub const MAX_EMAIL_TEMPLATE_ID_BYTES: usize = 256;
pub const MAX_EMAIL_LOCALE_BYTES: usize = 64;
pub const MAX_EMAIL_RECIPIENT_BYTES: usize = 320;
pub const MAX_EMAIL_VARS_BYTES: usize = 64 * 1_024;

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EmailDeliveryIdentity {
    tenant_id: String,
    idempotency_key: String,
}

#[derive(Debug)]
enum EmailIdempotencyEntry {
    Pending {
        reservation_id: u64,
        fingerprint: [u8; 32],
        completion: watch::Sender<bool>,
    },
    Completed {
        fingerprint: [u8; 32],
        receipt: EmailDeliveryReceipt,
        completed_at: Instant,
    },
}

#[derive(Debug, Default)]
struct EmailIdempotencyState {
    entries: HashMap<EmailDeliveryIdentity, EmailIdempotencyEntry>,
    completion_order: VecDeque<(Instant, EmailDeliveryIdentity)>,
}

#[derive(Debug)]
struct EmailIdempotencyTracker {
    state: Mutex<EmailIdempotencyState>,
    next_reservation_id: AtomicU64,
    maximum_entries: usize,
    ttl: Duration,
}

impl EmailIdempotencyTracker {
    fn new(maximum_entries: usize, ttl: Duration) -> Self {
        Self {
            state: Mutex::new(EmailIdempotencyState::default()),
            next_reservation_id: AtomicU64::new(1),
            maximum_entries,
            ttl,
        }
    }

    fn begin(
        &'static self,
        identity: EmailDeliveryIdentity,
        fingerprint: [u8; 32],
    ) -> Result<EmailIdempotencyDecision, PortError> {
        let now = Instant::now();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.prune_expired(&mut state, now);

        if let Some(entry) = state.entries.get(&identity) {
            return match entry {
                EmailIdempotencyEntry::Pending {
                    fingerprint: existing,
                    completion,
                    ..
                } if *existing == fingerprint => Ok(EmailIdempotencyDecision::Wait(
                    completion.subscribe(),
                )),
                EmailIdempotencyEntry::Completed {
                    fingerprint: existing,
                    receipt,
                    ..
                } if *existing == fingerprint => {
                    Ok(EmailIdempotencyDecision::Cached(receipt.clone()))
                }
                _ => Err(PortError::conflict(
                    "email.idempotency_conflict",
                    "email idempotency key was already used for another delivery request",
                )),
            };
        }

        if state.entries.len() >= self.maximum_entries {
            return Err(PortError::unavailable(
                "email.idempotency_capacity",
                "email idempotency capacity is temporarily exhausted",
            ));
        }

        let reservation_id = self.next_reservation_id.fetch_add(1, Ordering::Relaxed);
        let (completion, _receiver) = watch::channel(false);
        state.entries.insert(
            identity.clone(),
            EmailIdempotencyEntry::Pending {
                reservation_id,
                fingerprint,
                completion: completion.clone(),
            },
        );
        Ok(EmailIdempotencyDecision::Execute(
            EmailIdempotencyReservation {
                tracker: self,
                identity,
                reservation_id,
                completion,
                finished: false,
            },
        ))
    }

    fn complete(
        &self,
        identity: &EmailDeliveryIdentity,
        reservation_id: u64,
        receipt: EmailDeliveryReceipt,
    ) {
        let now = Instant::now();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(EmailIdempotencyEntry::Pending {
            reservation_id: current_id,
            fingerprint,
            completion,
        }) = state.entries.get(identity)
        else {
            return;
        };
        if *current_id != reservation_id {
            return;
        }

        let fingerprint = *fingerprint;
        let completion = completion.clone();
        state.entries.insert(
            identity.clone(),
            EmailIdempotencyEntry::Completed {
                fingerprint,
                receipt,
                completed_at: now,
            },
        );
        state.completion_order.push_back((now, identity.clone()));
        let _ = completion.send(true);
    }

    fn cancel(&self, identity: &EmailDeliveryIdentity, reservation_id: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let completion = match state.entries.get(identity) {
            Some(EmailIdempotencyEntry::Pending {
                reservation_id: current_id,
                completion,
                ..
            }) if *current_id == reservation_id => Some(completion.clone()),
            _ => None,
        };
        if completion.is_some() {
            state.entries.remove(identity);
        }
        drop(state);
        if let Some(completion) = completion {
            let _ = completion.send(true);
        }
    }

    fn prune_expired(&self, state: &mut EmailIdempotencyState, now: Instant) {
        while let Some((completed_at, identity)) = state.completion_order.front().cloned() {
            if now.saturating_duration_since(completed_at) < self.ttl {
                break;
            }
            state.completion_order.pop_front();
            let remove = matches!(
                state.entries.get(&identity),
                Some(EmailIdempotencyEntry::Completed {
                    completed_at: current,
                    ..
                }) if *current == completed_at
            );
            if remove {
                state.entries.remove(&identity);
            }
        }
    }
}

fn email_idempotency_tracker() -> &'static EmailIdempotencyTracker {
    static TRACKER: OnceLock<EmailIdempotencyTracker> = OnceLock::new();
    TRACKER.get_or_init(|| {
        EmailIdempotencyTracker::new(
            DEFAULT_MAX_EMAIL_IDEMPOTENCY_ENTRIES,
            DEFAULT_EMAIL_IDEMPOTENCY_TTL,
        )
    })
}

enum EmailIdempotencyDecision {
    Cached(EmailDeliveryReceipt),
    Wait(watch::Receiver<bool>),
    Execute(EmailIdempotencyReservation),
}

struct EmailIdempotencyReservation {
    tracker: &'static EmailIdempotencyTracker,
    identity: EmailDeliveryIdentity,
    reservation_id: u64,
    completion: watch::Sender<bool>,
    finished: bool,
}

impl EmailIdempotencyReservation {
    fn commit(mut self, receipt: EmailDeliveryReceipt) {
        self.tracker
            .complete(&self.identity, self.reservation_id, receipt);
        self.finished = true;
    }
}

impl Drop for EmailIdempotencyReservation {
    fn drop(&mut self) {
        if !self.finished {
            self.tracker
                .cancel(&self.identity, self.reservation_id);
            let _ = self.completion.send(true);
        }
    }
}

#[async_trait]
impl EmailDeliveryPort for crate::EmailService {
    async fn send_transactional_email(
        &self,
        context: PortContext,
        request: EmailDeliveryRequest,
    ) -> Result<EmailDeliveryReceipt, PortError> {
        require_email_delivery_policy(&context)?;
        validate_delivery_context(&context)?;
        let vars = validate_delivery_request(&request)?;
        let identity = EmailDeliveryIdentity {
            tenant_id: context.tenant_id.clone(),
            idempotency_key: context
                .idempotency_key
                .clone()
                .expect("write policy validated an idempotency key"),
        };
        let fingerprint = delivery_fingerprint(&context.tenant_id, &request, &vars);
        let budget = Duration::from_millis(
            context
                .deadline_ms
                .expect("write policy validated a deadline"),
        );
        let started_at = Instant::now();

        loop {
            match email_idempotency_tracker().begin(identity.clone(), fingerprint)? {
                EmailIdempotencyDecision::Cached(receipt) => return Ok(receipt),
                EmailIdempotencyDecision::Wait(mut completion) => {
                    if !*completion.borrow() {
                        let remaining = remaining_delivery_budget(started_at, budget)?;
                        execute_with_deadline(remaining, async {
                            let _ = completion.changed().await;
                        })
                        .await?;
                    }
                }
                EmailIdempotencyDecision::Execute(reservation) => {
                    let remaining = remaining_delivery_budget(started_at, budget)?;
                    let send_result = execute_with_deadline(
                        remaining,
                        self.send_transactional(
                            &request.template_id,
                            &request.locale,
                            &request.to,
                            &request.vars,
                        ),
                    )
                    .await?;
                    send_result.map_err(map_email_error)?;

                    let receipt = EmailDeliveryReceipt {
                        accepted: true,
                        provider_mode: match self {
                            crate::EmailService::Disabled => EmailProviderMode::DisabledNoop,
                            crate::EmailService::Smtp(_) => EmailProviderMode::Smtp,
                        },
                    };
                    reservation.commit(receipt.clone());
                    return Ok(receipt);
                }
            }
        }
    }
}

fn validate_delivery_context(context: &PortContext) -> Result<(), PortError> {
    if context.tenant_id.trim().is_empty() || context.tenant_id.len() > MAX_EMAIL_TENANT_ID_BYTES {
        return Err(PortError::validation(
            "email.tenant_invalid",
            "email delivery requires a bounded non-empty tenant identity",
        ));
    }
    let key = context
        .idempotency_key
        .as_deref()
        .expect("write policy validated an idempotency key");
    if key.len() > MAX_EMAIL_IDEMPOTENCY_KEY_BYTES {
        return Err(PortError::validation(
            "email.idempotency_key_too_large",
            "email idempotency key exceeds the supported size",
        ));
    }
    Ok(())
}

fn validate_delivery_request(request: &EmailDeliveryRequest) -> Result<Vec<u8>, PortError> {
    if request.template_id.trim().is_empty() {
        return Err(PortError::validation(
            "email.template_id_empty",
            "email delivery requires a non-empty template id",
        ));
    }
    if request.template_id.len() > MAX_EMAIL_TEMPLATE_ID_BYTES {
        return Err(PortError::validation(
            "email.template_id_too_large",
            "email template id exceeds the supported size",
        ));
    }
    if request.locale.trim().is_empty() {
        return Err(PortError::validation(
            "email.locale_empty",
            "email delivery requires a non-empty locale",
        ));
    }
    if request.locale.len() > MAX_EMAIL_LOCALE_BYTES {
        return Err(PortError::validation(
            "email.locale_too_large",
            "email locale exceeds the supported size",
        ));
    }
    if request.to.trim().is_empty() {
        return Err(PortError::validation(
            "email.recipient_empty",
            "email delivery requires a non-empty recipient",
        ));
    }
    if request.to.len() > MAX_EMAIL_RECIPIENT_BYTES {
        return Err(PortError::validation(
            "email.recipient_too_large",
            "email recipient exceeds the supported size",
        ));
    }
    request
        .to
        .parse::<lettre::message::Mailbox>()
        .map_err(|_| {
            PortError::validation(
                "email.recipient_invalid",
                "email delivery requires a valid recipient address",
            )
        })?;

    let vars = serde_json::to_vec(&request.vars).map_err(|_| {
        PortError::validation(
            "email.vars_invalid",
            "email delivery variables could not be encoded",
        )
    })?;
    if vars.len() > MAX_EMAIL_VARS_BYTES {
        return Err(PortError::validation(
            "email.vars_too_large",
            "email delivery variables exceed the supported size",
        ));
    }
    Ok(vars)
}

fn delivery_fingerprint(
    tenant_id: &str,
    request: &EmailDeliveryRequest,
    vars: &[u8],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_fingerprint_part(&mut digest, tenant_id.as_bytes());
    hash_fingerprint_part(&mut digest, request.template_id.as_bytes());
    hash_fingerprint_part(&mut digest, request.locale.as_bytes());
    hash_fingerprint_part(&mut digest, request.to.as_bytes());
    hash_fingerprint_part(&mut digest, vars);
    digest.finalize().into()
}

fn hash_fingerprint_part(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn remaining_delivery_budget(started_at: Instant, budget: Duration) -> Result<Duration, PortError> {
    let elapsed = started_at.elapsed();
    let remaining = budget.saturating_sub(elapsed);
    if remaining.is_zero() {
        return Err(delivery_timeout_error());
    }
    Ok(remaining)
}

async fn execute_with_deadline<T, F>(budget: Duration, future: F) -> Result<T, PortError>
where
    F: Future<Output = T>,
{
    tokio::time::timeout(budget, future)
        .await
        .map_err(|_| delivery_timeout_error())
}

fn delivery_timeout_error() -> PortError {
    PortError::timeout(
        "email.delivery_timeout",
        "email delivery exceeded the caller deadline",
    )
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use rustok_api::{PortActor, PortContext, PortErrorKind};

    use super::*;

    static TEST_ID: AtomicU64 = AtomicU64::new(1);

    fn unique_id(prefix: &str) -> String {
        format!("{prefix}-{}", TEST_ID.fetch_add(1, Ordering::Relaxed))
    }

    fn base_context() -> PortContext {
        PortContext::new(
            unique_id("tenant-email"),
            PortActor::service("email-contract-test"),
            "ru",
            unique_id("corr-email"),
        )
    }

    fn write_context() -> PortContext {
        base_context()
            .with_idempotency_key(unique_id("email-send"))
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
    async fn duplicate_request_reuses_the_completed_receipt() {
        let context = write_context();
        let request = delivery_request();
        let first = EmailDeliveryPort::send_transactional_email(
            &crate::EmailService::Disabled,
            context.clone(),
            request.clone(),
        )
        .await
        .unwrap();
        let second = EmailDeliveryPort::send_transactional_email(
            &crate::EmailService::Disabled,
            context,
            request,
        )
        .await
        .unwrap();

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn idempotency_key_reuse_with_different_request_is_rejected() {
        let context = write_context();
        EmailDeliveryPort::send_transactional_email(
            &crate::EmailService::Disabled,
            context.clone(),
            delivery_request(),
        )
        .await
        .unwrap();

        let mut changed = delivery_request();
        changed.to = "other@example.test".to_string();
        let error = EmailDeliveryPort::send_transactional_email(
            &crate::EmailService::Disabled,
            context,
            changed,
        )
        .await
        .expect_err("idempotency key reuse must not alias another request");

        assert_eq!(error.kind, PortErrorKind::Conflict);
        assert_eq!(error.code, "email.idempotency_conflict");
    }

    #[tokio::test]
    async fn delivery_deadline_is_applied_to_the_operation() {
        let error = execute_with_deadline(Duration::from_millis(1), std::future::pending::<()>())
            .await
            .expect_err("pending delivery must time out");

        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert_eq!(error.code, "email.delivery_timeout");
        assert!(error.retryable);
    }

    #[tokio::test]
    async fn disabled_provider_rejects_invalid_recipient_like_smtp() {
        let mut request = delivery_request();
        request.to = "not-an-email".to_string();

        let error = EmailDeliveryPort::send_transactional_email(
            &crate::EmailService::Disabled,
            write_context(),
            request,
        )
        .await
        .expect_err("disabled provider must preserve recipient validation");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "email.recipient_invalid");
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
