mod native_server_adapter;

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

pub use native_server_adapter::{
    cancel_job_native, fetch_bootstrap_native, retry_job_native, trigger_replay_native,
};

use crate::model::{
    CancelActionResult, CancelJobInput, IndexAdminBootstrap, ReplayActionResult,
    RetryActionResult, RetryJobInput, TriggerReplayInput,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexAdminTransportError {
    NativeServer(String),
}

impl Display for IndexAdminTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NativeServer(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for IndexAdminTransportError {}

impl From<leptos::prelude::ServerFnError> for IndexAdminTransportError {
    fn from(value: leptos::prelude::ServerFnError) -> Self {
        Self::NativeServer(value.to_string())
    }
}

pub async fn fetch_bootstrap() -> Result<IndexAdminBootstrap, IndexAdminTransportError> {
    fetch_bootstrap_native().await.map_err(Into::into)
}

pub async fn trigger_replay(
    input: TriggerReplayInput,
) -> Result<ReplayActionResult, IndexAdminTransportError> {
    trigger_replay_native(input).await.map_err(Into::into)
}

pub async fn cancel_job(
    input: CancelJobInput,
) -> Result<CancelActionResult, IndexAdminTransportError> {
    cancel_job_native(input).await.map_err(Into::into)
}

pub async fn retry_job(
    input: RetryJobInput,
) -> Result<RetryActionResult, IndexAdminTransportError> {
    retry_job_native(input).await.map_err(Into::into)
}

