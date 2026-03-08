use async_graphql::{Context, FieldError, Result, Subscription};
use futures_util::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::graphql::types::BuildProgressEvent;
use crate::services::auth::AuthService;
use crate::services::build_event_hub::BuildEventHub;
use rustok_core::{Action, Permission, Resource};

#[derive(Default)]
pub struct BuildSubscription;

async fn ensure_modules_read_permission(ctx: &Context<'_>) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
    let tenant = ctx.data::<TenantContext>()?;

    let can_read_modules = AuthService::has_any_permission(
        &app_ctx.db,
        &tenant.id,
        &auth.user_id,
        &[
            Permission::new(Resource::Modules, Action::Read),
            Permission::new(Resource::Modules, Action::List),
            Permission::new(Resource::Modules, Action::Manage),
        ],
    )
    .await
    .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

    if !can_read_modules {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: modules:read required",
        ));
    }

    Ok(())
}

#[Subscription]
impl BuildSubscription {
    async fn build_progress(
        &self,
        ctx: &Context<'_>,
        build_id: Option<String>,
    ) -> Result<impl futures_util::Stream<Item = BuildProgressEvent>> {
        ensure_modules_read_permission(ctx).await?;

        let hub = ctx.data::<std::sync::Arc<BuildEventHub>>()?;
        let receiver = hub.subscribe();
        let build_filter = build_id.filter(|value| !value.trim().is_empty());

        Ok(BroadcastStream::new(receiver).filter_map(move |event| {
            let build_filter = build_filter.clone();
            async move {
                match event {
                    Ok(event) => {
                        let payload = BuildProgressEvent::from_event(event);
                        let passes_filter = match build_filter.as_ref() {
                            Some(build_id) => payload.build_id == *build_id,
                            None => true,
                        };
                        if passes_filter {
                            Some(payload)
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            }
        }))
    }
}
