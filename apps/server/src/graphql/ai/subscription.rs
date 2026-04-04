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
