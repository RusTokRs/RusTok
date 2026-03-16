use axum::{extract::State, routing::get, Extension, Json};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use std::sync::Arc;

use crate::common::RequestContext;
use rustok_core::i18n::Locale;
use crate::context::{AuthContext, TenantContext};
use crate::extractors::auth::OptionalCurrentUser;
use crate::graphql::persisted::is_admin_persisted_hash;
use crate::graphql::AppSchema;
use rustok_core::ModuleRegistry;

async fn graphql_handler(
    State(ctx): State<AppContext>,
    Extension(registry): Extension<ModuleRegistry>,
    Extension(schema): Extension<Arc<AppSchema>>,
    tenant_ctx: TenantContext,
    request_context: RequestContext,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    Extension(locale): Extension<Locale>,
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

    let mut request = req
        .data(ctx)
        .data(tenant_ctx)
        .data(request_context)
        .data(registry)
        .data(locale);

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
