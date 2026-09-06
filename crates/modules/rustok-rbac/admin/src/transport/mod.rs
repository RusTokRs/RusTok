mod native_server_adapter;

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

pub use native_server_adapter::fetch_bootstrap_native;

use crate::model::RbacAdminBootstrap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RbacAdminTransportError {
    NativeServer(String),
}

impl Display for RbacAdminTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NativeServer(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RbacAdminTransportError {}

impl From<leptos::prelude::ServerFnError> for RbacAdminTransportError {
    fn from(value: leptos::prelude::ServerFnError) -> Self {
        Self::NativeServer(value.to_string())
    }
}

pub async fn fetch_bootstrap() -> Result<RbacAdminBootstrap, RbacAdminTransportError> {
    fetch_bootstrap_native().await.map_err(Into::into)
}
