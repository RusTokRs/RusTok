use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::state::InstallStep;

#[derive(Debug, Error)]
pub enum ReceiptError {
    #[error("failed to serialize receipt input for checksum: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptOutcome {
    Success,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReceipt {
    pub session_id: String,
    pub step: InstallStep,
    pub input_checksum: String,
    pub outcome: ReceiptOutcome,
    pub diagnostics: serde_json::Value,
    pub installer_version: String,
    pub created_at: DateTime<Utc>,
}

impl InstallReceipt {
    pub fn success<T: Serialize>(
        session_id: impl Into<String>,
        step: InstallStep,
        input: &T,
        diagnostics: serde_json::Value,
    ) -> Result<Self, ReceiptError> {
        Ok(Self {
            session_id: session_id.into(),
            step,
            input_checksum: checksum_json(input)?,
            outcome: ReceiptOutcome::Success,
            diagnostics,
            installer_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: Utc::now(),
        })
    }

    pub fn can_skip<T: Serialize>(
        &self,
        step: InstallStep,
        input: &T,
    ) -> Result<bool, ReceiptError> {
        Ok(self.step == step
            && self.outcome == ReceiptOutcome::Success
            && self.input_checksum == checksum_json(input)?)
    }
}

pub fn checksum_json<T: Serialize>(value: &T) -> Result<String, ReceiptError> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Input {
        value: &'static str,
    }

    #[test]
    fn checksum_is_stable_for_same_json() {
        let left = checksum_json(&Input { value: "same" }).unwrap();
        let right = checksum_json(&Input { value: "same" }).unwrap();

        assert_eq!(left, right);
    }

    #[test]
    fn receipt_can_skip_matching_successful_step() {
        let input = Input { value: "db-ready" };
        let receipt = InstallReceipt::success(
            "is_01",
            InstallStep::Database,
            &input,
            serde_json::json!({}),
        )
        .unwrap();

        assert!(receipt.can_skip(InstallStep::Database, &input).unwrap());
        assert!(!receipt.can_skip(InstallStep::Migrate, &input).unwrap());
    }
}
