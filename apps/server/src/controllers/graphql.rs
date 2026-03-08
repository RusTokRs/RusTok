use axum::{extract::State, routing::get, Extension, Json};
use loco_rs::prelude::*;

use crate::context::{AuthContext, TenantContext};
use crate::extractors::auth::OptionalCurrentUser;
use crate::graphql::build_schema;
use crate::graphql::persisted::is_admin_persisted_hash;
use crate::services::build_event_hub::build_event_hub_from_context;
use crate::services::event_bus::{event_bus_from_context, transactional_event_bus_from_context};
use rustok_core::ModuleRegistry;

async fn graphql_handler(
    State(ctx): State<AppContext>,
    Extension(registry): Extension<ModuleRegistry>,
    Extension(alloy_state): Extension<crate::graphql::alloy::AlloyState>,
    tenant_ctx: TenantContext,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    Json(req): Json<async_graphql::Request>,
) -> Json<async_graphql::Response> {
    if is_critical_admin_operation(&req) {
        let hash = persisted_query_hash(&req);
        if hash.is_none_or(|hash| !is_admin_persisted_hash(hash)) {
            return Json(async_graphql::Response::from_errors(vec![
                async_graphql::ServerError::new(
                    "Critical admin operations require an approved persisted query hash",
                    None,
                ),
            ]));
        }
    }

    let schema = build_schema(
        ctx.db.clone(),
        event_bus_from_context(&ctx),
        transactional_event_bus_from_context(&ctx),
        build_event_hub_from_context(&ctx),
        alloy_state,
    );
    let mut request = req.data(ctx).data(tenant_ctx).data(registry);

    if let Some(current_user) = current_user {
        let auth_ctx = AuthContext {
            user_id: current_user.user.id,
            session_id: current_user.session_id,
            tenant_id: current_user.user.tenant_id,
            permissions: current_user.permissions,
            client_id: current_user.client_id,
            scopes: current_user.scopes.clone(),
            grant_type: current_user.grant_type.clone(),
        };
        request = request.data(auth_ctx);
    }

    Json(schema.execute(request).await)
}

fn is_critical_admin_operation(req: &async_graphql::Request) -> bool {
    let op_name = req.operation_name.as_deref().unwrap_or_default();
    matches!(op_name, "Users" | "User")
}

fn persisted_query_hash(req: &async_graphql::Request) -> Option<&str> {
    use async_graphql::Value;

    let value = req.extensions.get("persistedQuery")?;
    let Value::Object(obj) = value else {
        return None;
    };
    let Value::String(hash) = obj.get("sha256Hash")? else {
        return None;
    };
    Some(hash.as_ref())
}

async fn graphql_playground() -> impl axum::response::IntoResponse {
    axum::response::Html(async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/api/graphql"),
    ))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/graphql")
        .add("/", get(graphql_playground).post(graphql_handler))
}
