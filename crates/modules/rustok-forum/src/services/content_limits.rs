//! Length limits for user-authored topic titles and bodies, and the posting rate-limit intervals.
//!
//! Limits come from the tenant's Forum module settings. A maximum of `0` means
//! "no maximum"; a minimum of `0` means "no minimum" (the empty-title check in
//! `validate_topic_title` still applies). Lengths are counted in Unicode scalar
//! values (`chars().count()`), and titles are measured after trimming. A rate-limit
//! interval of `0` disables that limit. The rate check itself lives in `posting_rate`.
//!
//! Imports and UGC translation apply intentionally do not use these limits;
//! see `DECISIONS/2026-10-09-forum-soft-default-settings.md`.

use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};
use crate::services::engagement_mode::{ForumSettings, ForumSettingsProviders};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ForumContentLimits {
    min_title_chars: u32,
    max_title_chars: u32,
    min_body_chars: u32,
    max_body_chars: u32,
    new_topic_rate_limit_seconds: u32,
    new_reply_rate_limit_seconds: u32,
}

impl ForumContentLimits {
    pub(crate) async fn resolve(
        providers: &ForumSettingsProviders,
        tenant_id: Uuid,
    ) -> ForumResult<Self> {
        Ok(Self::from_settings(
            &providers.module_settings(tenant_id).await?,
        ))
    }

    pub(crate) async fn resolve_in_tx(
        providers: &ForumSettingsProviders,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> ForumResult<Self> {
        Ok(Self::from_settings(
            &providers.module_settings_in_tx(txn, tenant_id).await?,
        ))
    }

    fn from_settings(settings: &ForumSettings) -> Self {
        Self {
            min_title_chars: settings.min_topic_title_length,
            max_title_chars: settings.max_topic_title_length,
            min_body_chars: settings.min_post_body_length,
            max_body_chars: settings.max_post_body_length,
            new_topic_rate_limit_seconds: settings.rate_limit_new_topic_seconds,
            new_reply_rate_limit_seconds: settings.rate_limit_new_reply_seconds,
        }
    }

    pub(crate) fn new_topic_rate_limit_seconds(self) -> u32 {
        self.new_topic_rate_limit_seconds
    }

    pub(crate) fn new_reply_rate_limit_seconds(self) -> u32 {
        self.new_reply_rate_limit_seconds
    }

    pub(crate) fn validate_title(self, title: &str) -> ForumResult<()> {
        let length = char_count(title.trim());
        check_bounds(
            "Topic title",
            length,
            self.min_title_chars,
            self.max_title_chars,
        )
    }

    /// `plain_text` must be the body projected to plain text (see `richtext::project_discussion`).
    pub(crate) fn validate_body(self, plain_text: &str) -> ForumResult<()> {
        check_bounds(
            "Post body",
            char_count(plain_text),
            self.min_body_chars,
            self.max_body_chars,
        )
    }
}

fn char_count(text: &str) -> u32 {
    // Lengths above u32::MAX cannot be stored within the configured maxima anyway,
    // so saturating keeps the check conservative instead of panicking.
    u32::try_from(text.chars().count()).unwrap_or(u32::MAX)
}

fn check_bounds(subject: &str, length: u32, min: u32, max: u32) -> ForumResult<()> {
    if max != 0 && min > max {
        return Err(ForumError::Validation(format!(
            "{subject} length limits are inconsistent: minimum {min} exceeds maximum {max}"
        )));
    }
    if length < min {
        return Err(ForumError::Validation(format!(
            "{subject} must be at least {min} characters"
        )));
    }
    if max != 0 && length > max {
        return Err(ForumError::Validation(format!(
            "{subject} must not exceed {max} characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_bounds;

    #[test]
    fn zero_maximum_means_unlimited() {
        assert!(check_bounds("Post body", 1_000_000, 1, 0).is_ok());
    }

    #[test]
    fn enforces_minimum_and_maximum_inclusively() {
        assert!(check_bounds("Topic title", 1, 1, 255).is_ok());
        assert!(check_bounds("Topic title", 255, 1, 255).is_ok());
        assert!(check_bounds("Topic title", 0, 1, 255).is_err());
        assert!(check_bounds("Topic title", 256, 1, 255).is_err());
    }

    #[test]
    fn rejects_inconsistent_limits() {
        assert!(check_bounds("Topic title", 10, 20, 15).is_err());
    }
}
