use async_graphql::{Context, Result, Subscription};
use futures_util::stream;
use uuid::Uuid;

use crate::context::AuthContext;

use super::{ensure_ai_session_read, types::AiRunStreamEventGql};

#[derive(Default)]
pub struct AiSubscription;

#[Subscription]
impl AiSubscription {
    async fn ai_session_events(
        &self,
        ctx: &Context<'_>,
        session_id: Uuid,
    ) -> Result<impl futures_util::Stream<Item = AiRunStreamEventGql>> {
        let auth = ctx.data::<AuthContext>()?;
        ensure_ai_session_read(auth)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let exists =
            rustok_ai::AiManagementService::chat_session_detail(db, auth.tenant_id, session_id)
                .await
                .map_err(|err| async_graphql::Error::new(err.to_string()))?
                .is_some();
        if !exists {
            return Err(async_graphql::Error::new(
                "Session not found or access denied",
            ));
        }

        let receiver = rustok_ai::ai_run_stream_hub().subscribe();

        Ok(stream::unfold(receiver, move |mut receiver| async move {
            loop {
                match receiver.recv().await {
                    Ok(event) if event.session_id == session_id => {
                        return Some((event.into(), receiver));
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        }))
    }
}
