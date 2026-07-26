use std::time::Instant;

use uuid::Uuid;

use crate::ProfileError;

pub const PROFILE_OPERATION_TARGET: &str = "rustok_profiles::operations";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileOperation {
    Upsert,
    UpdateHandle,
    UpdateContent,
    UpdateLocale,
    UpdateVisibility,
    UpdateMedia,
    PublishUpdatedEvent,
}

impl ProfileOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "profile.upsert",
            Self::UpdateHandle => "profile.update_handle",
            Self::UpdateContent => "profile.update_content",
            Self::UpdateLocale => "profile.update_locale",
            Self::UpdateVisibility => "profile.update_visibility",
            Self::UpdateMedia => "profile.update_media",
            Self::PublishUpdatedEvent => "profile.publish_updated_event",
        }
    }
}

#[derive(Debug)]
pub struct ProfileOperationTimer {
    operation: ProfileOperation,
    tenant_id: Uuid,
    user_id: Uuid,
    started_at: Instant,
}

impl ProfileOperationTimer {
    pub fn start(operation: ProfileOperation, tenant_id: Uuid, user_id: Uuid) -> Self {
        Self {
            operation,
            tenant_id,
            user_id,
            started_at: Instant::now(),
        }
    }

    pub fn finish_profile_result<T>(self, result: &Result<T, ProfileError>) {
        match result {
            Ok(_) => self.finish_success(),
            Err(error) => self.finish_failure(error.code(), error.is_retryable()),
        }
    }

    pub fn finish_success(self) {
        tracing::info!(
            target: PROFILE_OPERATION_TARGET,
            operation = self.operation.as_str(),
            tenant_id = %self.tenant_id,
            user_id = %self.user_id,
            outcome = "success",
            duration_ms = self.started_at.elapsed().as_millis() as u64,
            "Profile owner operation completed"
        );
    }

    pub fn finish_failure(self, error_code: &'static str, retryable: bool) {
        tracing::warn!(
            target: PROFILE_OPERATION_TARGET,
            operation = self.operation.as_str(),
            tenant_id = %self.tenant_id,
            user_id = %self.user_id,
            outcome = "failure",
            error_code,
            retryable,
            duration_ms = self.started_at.elapsed().as_millis() as u64,
            "Profile owner operation failed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::ProfileOperation;

    #[test]
    fn operation_names_are_stable_and_owner_scoped() {
        assert_eq!(ProfileOperation::Upsert.as_str(), "profile.upsert");
        assert_eq!(
            ProfileOperation::UpdateVisibility.as_str(),
            "profile.update_visibility"
        );
        assert_eq!(
            ProfileOperation::PublishUpdatedEvent.as_str(),
            "profile.publish_updated_event"
        );
    }
}
