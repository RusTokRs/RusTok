use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelResolutionSource {
    HeaderId,
    HeaderSlug,
    Query,
    Host,
    Policy,
    Default,
}

impl ChannelResolutionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HeaderId => "header_id",
            Self::HeaderSlug => "header_slug",
            Self::Query => "query",
            Self::Host => "host",
            Self::Policy => "policy",
            Self::Default => "default",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelContext {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
    pub status: String,
    pub target_type: Option<String>,
    pub target_value: Option<String>,
    pub settings: serde_json::Value,
    pub resolution_source: ChannelResolutionSource,
}

#[derive(Clone)]
pub struct ChannelContextExtension(pub ChannelContext);

pub trait ChannelContextExt {
    fn channel_context(&self) -> Option<&ChannelContext>;
}

impl ChannelContextExt for Parts {
    fn channel_context(&self) -> Option<&ChannelContext> {
        self.extensions
            .get::<ChannelContextExtension>()
            .map(|ext| &ext.0)
    }
}

impl<S> FromRequestParts<S> for ChannelContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ChannelContextExtension>()
            .map(|ext| ext.0.clone())
            .ok_or((
                StatusCode::NOT_FOUND,
                "ChannelContext not found for request".to_string(),
            ))
    }
}

pub struct OptionalChannel(pub Option<ChannelContext>);

impl<S> FromRequestParts<S> for OptionalChannel
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<ChannelContextExtension>()
                .map(|ext| ext.0.clone()),
        ))
    }
}
