mod native_server_adapter;

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

pub use native_server_adapter::fetch_bootstrap_native;

use crate::model::IndexAdminBootstrap;

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
