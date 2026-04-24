#[cfg(feature = "server")]
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelResolutionStage {
    HeaderId,
    HeaderSlug,
    Query,
    Host,
    Policy,
    Default,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelResolutionOutcome {
    Matched,
    Miss,
    Rejected,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelResolutionTraceStep {
    pub stage: ChannelResolutionStage,
    pub outcome: ChannelResolutionOutcome,
    pub detail: String,
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
    pub resolution_trace: Vec<ChannelResolutionTraceStep>,
}

#[cfg(feature = "server")]
#[derive(Clone)]
pub struct ChannelContextExtension(pub ChannelContext);

#[cfg(feature = "server")]
pub trait ChannelContextExt {
    fn channel_context(&self) -> Option<&ChannelContext>;
}

#[cfg(feature = "server")]
impl ChannelContextExt for Parts {
    fn channel_context(&self) -> Option<&ChannelContext> {
        self.extensions
            .get::<ChannelContextExtension>()
            .map(|ext| &ext.0)
    }
}

#[cfg(feature = "server")]
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

#[cfg(feature = "server")]
pub struct OptionalChannel(pub Option<ChannelContext>);

#[cfg(feature = "server")]
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
