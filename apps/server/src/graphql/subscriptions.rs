use async_graphql::{Context, FieldError, Result, Subscription};
use futures_util::stream;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::graphql::types::BuildProgressEvent;
use crate::services::build_event_hub::BuildEventHub;
use crate::services::rbac_service::RbacService;
use rustok_core::{Action, EventConsumerRuntime, Permission, Resource};

#[derive(Default)]
pub struct BuildSubscription;

async fn ensure_modules_read_permission(ctx: &Context<'_>) -> Result<()> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
    let tenant = ctx.data::<TenantContext>()?;

    let can_read_modules = RbacService::has_any_permission(
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
        let consumer_runtime = EventConsumerRuntime::new("graphql_build_progress");

        Ok(stream::unfold(
            (receiver, build_filter),
            move |(mut receiver, build_filter)| async move {
                loop {
                    match receiver.recv().await {
                        Ok(event) => {
                            let payload = BuildProgressEvent::from_event(event);
                            let passes_filter = match build_filter.as_ref() {
                                Some(build_id) => payload.build_id == *build_id,
                                None => true,
                            };
                            if passes_filter {
                                return Some((payload, (receiver, build_filter)));
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            consumer_runtime.lagged(skipped);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            consumer_runtime.closed();
                            return None;
                        }
                    }
                }
            },
        ))
    }
}
