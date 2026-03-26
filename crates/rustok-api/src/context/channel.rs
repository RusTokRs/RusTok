use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
