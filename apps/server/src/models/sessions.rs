use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use rustok_core::generate_id;

pub use super::_entities::sessions::{ActiveModel, Column, Entity, Model};

/// Hard cap on a refresh lineage: no amount of sliding rotation extends
/// a session beyond this many days from the original login.
pub const SESSION_ABSOLUTE_LIFETIME_DAYS: i64 = 90;

impl Model {
    /// Absolute expiry is a second authority fence, not just a refresh-rotation
    /// constraint. Legacy rows without an explicit deadline are bounded by
    /// their original creation time rather than granted a fresh lifetime.
    pub fn absolute_deadline(&self) -> DateTime<Utc> {
        let Some(original_cap) = self
            .created_at
            .with_timezone(&Utc)
            .checked_add_signed(Duration::days(SESSION_ABSOLUTE_LIFETIME_DAYS))
        else {
            // An unrepresentable persisted timestamp must never make an
            // otherwise invalid session valid (or panic in a request).
            return DateTime::<Utc>::MIN_UTC;
        };
        self.absolute_expires_at
            .as_ref()
            .map(|deadline| deadline.with_timezone(&Utc).min(original_cap))
            .unwrap_or(original_cap)
    }

    pub fn is_active(&self) -> bool {
        let now = Utc::now();
        self.revoked_at.is_none() && self.expires_at > now && self.absolute_deadline() > now
    }
}

impl ActiveModel {
    pub fn new(
        tenant_id: Uuid,
        user_id: Uuid,
        token_hash: String,
        expires_at: DateTime<Utc>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        Self {
            id: sea_orm::ActiveValue::Set(generate_id()),
            tenant_id: sea_orm::ActiveValue::Set(tenant_id),
            user_id: sea_orm::ActiveValue::Set(user_id),
            token_hash: sea_orm::ActiveValue::Set(token_hash),
            ip_address: sea_orm::ActiveValue::Set(ip_address),
            user_agent: sea_orm::ActiveValue::Set(user_agent),
            last_used_at: sea_orm::ActiveValue::NotSet,
            expires_at: sea_orm::ActiveValue::Set(expires_at.into()),
            revoked_at: sea_orm::ActiveValue::NotSet,
            previous_token_hash: sea_orm::ActiveValue::NotSet,
            previous_token_revoked_at: sea_orm::ActiveValue::NotSet,
            absolute_expires_at: sea_orm::ActiveValue::Set(Some(
                (Utc::now() + Duration::days(SESSION_ABSOLUTE_LIFETIME_DAYS)).into(),
            )),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(created_at: DateTime<Utc>) -> Model {
        let now = Utc::now();
        Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_hash: "redacted-session-hash".to_string(),
            ip_address: None,
            user_agent: None,
            last_used_at: None,
            expires_at: (now + Duration::days(1)).into(),
            revoked_at: None,
            previous_token_hash: None,
            previous_token_revoked_at: None,
            absolute_expires_at: None,
            created_at: created_at.into(),
            updated_at: created_at.into(),
        }
    }

    #[test]
    fn legacy_session_expires_after_original_absolute_cap() {
        let now = Utc::now();
        let legacy = session(now - Duration::days(SESSION_ABSOLUTE_LIFETIME_DAYS + 1));
        assert!(
            !legacy.is_active(),
            "a long sliding TTL must not revive an old lineage"
        );
    }

    #[test]
    fn earlier_persisted_deadline_blocks_an_otherwise_live_session() {
        let now = Utc::now();
        let mut current = session(now - Duration::days(1));
        assert!(current.is_active());

        current.absolute_expires_at = Some((now - Duration::seconds(1)).into());
        assert!(!current.is_active());
    }

    #[test]
    fn corrupted_timestamp_cannot_overflow_absolute_expiry_check() {
        let created_at = DateTime::<Utc>::MAX_UTC;
        let record = session(created_at);
        assert!(!record.is_active());
        assert_eq!(record.absolute_deadline(), DateTime::<Utc>::MIN_UTC);
    }

    #[test]
    fn persisted_deadline_cannot_extend_beyond_the_original_cap() {
        let now = Utc::now();
        let mut old = session(now - Duration::days(SESSION_ABSOLUTE_LIFETIME_DAYS + 1));
        old.absolute_expires_at = Some((now + Duration::days(200)).into());
        assert!(!old.is_active());
    }
}
