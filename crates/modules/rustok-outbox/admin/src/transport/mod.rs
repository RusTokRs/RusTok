mod native_server_adapter;

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::core::OutboxAdminBootstrap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutboxTransportError {
    ServerFn(String),
}

impl Display for OutboxTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for OutboxTransportError {}

impl From<leptos::prelude::ServerFnError> for OutboxTransportError {
    fn from(value: leptos::prelude::ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_bootstrap() -> Result<OutboxAdminBootstrap, OutboxTransportError> {
    native_server_adapter::fetch_bootstrap_native()
        .await
        .map_err(Into::into)
}
