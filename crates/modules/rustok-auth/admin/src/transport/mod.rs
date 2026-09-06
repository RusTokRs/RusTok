pub mod native_server_adapter;

pub use native_server_adapter::ApiError;

use base64::{Engine, engine::general_purpose::STANDARD};
use rustok_ui_transport::UiTransportPath;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::model::{CreateOAuthAppInput, UpdateOAuthAppInput};

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

pub async fn request_password_reset(email: String, tenant: String) -> Result<String, String> {
    leptos_auth::api::forgot_password(email, tenant)
        .await
        .map_err(|error| error.to_string())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ApiRequestContext {
    pub token: Option<String>,
    pub tenant_slug: Option<String>,
    pub locale: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerGraphqlRequest {
    pub query: String,
    pub variables: Value,
    pub persisted_query_sha256: Option<String>,
    pub context: ApiRequestContext,
}

pub fn get_graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    if let Some(base) = option_env!("RUSTOK_API_URL") {
        return format!("{}/api/graphql", base.trim_end_matches('/'));
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{}/api/graphql", origin)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{}/api/graphql", base)
    }
}

pub fn api_base_url() -> String {
    get_graphql_url()
        .trim_end_matches("/api/graphql")
        .trim_end_matches('/')
        .to_string()
}

fn build_request_context(token: Option<String>, tenant_slug: Option<String>) -> ApiRequestContext {
    ApiRequestContext {
        token,
        tenant_slug,
        locale: None,
    }
}

#[cfg(any(
    all(target_arch = "wasm32", feature = "csr", not(feature = "hydrate")),
    feature = "ssr"
))]
pub(super) async fn execute_server_graphql(
    request: ServerGraphqlRequest,
) -> Result<Value, rustok_graphql::GraphqlHttpError> {
    let mut graphql_request =
        rustok_graphql::GraphqlRequest::new(request.query, Some(request.variables));

    if let Some(sha256_hash) = request.persisted_query_sha256.as_deref() {
        graphql_request =
            graphql_request.with_extensions(rustok_graphql::persisted_query_extension(sha256_hash));
    }

    rustok_graphql::execute(
        &get_graphql_url(),
        graphql_request,
        request.context.token,
        request.context.tenant_slug,
        request.context.locale,
    )
    .await
}

async fn execute_auth_graphql(
    request: ServerGraphqlRequest,
) -> Result<Value, rustok_graphql::GraphqlHttpError> {
    #[cfg(all(target_arch = "wasm32", not(feature = "hydrate")))]
    {
        execute_server_graphql(request).await
    }

    #[cfg(not(all(target_arch = "wasm32", not(feature = "hydrate"))))]
    {
        native_server_adapter::auth_graphql(request)
            .await
            .map_err(|err| {
                let message = err.to_string();
                if message == "Unauthorized" {
                    rustok_graphql::GraphqlHttpError::Unauthorized
                } else if message == "Network error" {
                    rustok_graphql::GraphqlHttpError::Network
                } else if let Some(value) = message.strip_prefix("Http error: ") {
                    rustok_graphql::GraphqlHttpError::Http(value.to_string())
                } else if let Some(value) = message.strip_prefix("GraphQL error: ") {
                    rustok_graphql::GraphqlHttpError::Graphql(value.to_string())
                } else {
                    rustok_graphql::GraphqlHttpError::Graphql(message)
                }
            })
    }
}

pub async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, rustok_graphql::GraphqlHttpError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    let response = execute_auth_graphql(ServerGraphqlRequest {
        query: query.to_string(),
        variables: serde_json::to_value(variables)
            .map_err(|err| rustok_graphql::GraphqlHttpError::Graphql(err.to_string()))?,
        persisted_query_sha256: None,
        context: build_request_context(token, tenant_slug),
    })
    .await?;

    serde_json::from_value(response)
        .map_err(|err| rustok_graphql::GraphqlHttpError::Graphql(err.to_string()))
}

pub async fn request_with_persisted<V, T>(
    query: &str,
    variables: V,
    sha256_hash: &str,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, rustok_graphql::GraphqlHttpError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    let response = execute_auth_graphql(ServerGraphqlRequest {
        query: query.to_string(),
        variables: serde_json::to_value(variables)
            .map_err(|err| rustok_graphql::GraphqlHttpError::Graphql(err.to_string()))?,
        persisted_query_sha256: Some(sha256_hash.to_string()),
        context: build_request_context(token, tenant_slug),
    })
    .await?;

    serde_json::from_value(response)
        .map_err(|err| rustok_graphql::GraphqlHttpError::Graphql(err.to_string()))
}

use crate::model::{GraphqlUserResponse, GraphqlUsersResponse, OAuthApp};

pub const USERS_QUERY: &str = "query Users($pagination: PaginationInput, $filter: UsersFilter, $search: String) { users(pagination: $pagination, filter: $filter, search: $search) { edges { cursor node { id email name role status createdAt tenantName } } pageInfo { totalCount hasNextPage endCursor } } }";
pub const USERS_QUERY_HASH: &str =
    "ff1e132e28d2e1c804d8d5ade5966307e17685b9f4b39262d70ecaa4d49abb66";

pub const USER_DETAILS_QUERY: &str =
    "query User($id: UUID!) { user(id: $id) { id email name role status createdAt tenantName } }";
pub const USER_DETAILS_QUERY_HASH: &str =
    "85f7f7ba212ab47e951fcf7dbb30bb918e66b88710574a576b0088877653f3b7";

pub const OAUTH_APPS_QUERY: &str = r#"
query OAuthApps($limit: Int) {
  oauthApps(limit: $limit) {
    id
    name
    slug
    description
    iconUrl
    appType
    clientId
    redirectUris
    scopes
    grantTypes
    manifestRef
    autoCreated
    managedByManifest
    isActive
    canEdit
    canRotateSecret
    canRevoke
    activeTokenCount
    lastUsedAt
    createdAt
  }
}
"#;

#[derive(serde::Serialize)]
struct UsersVariables {
    pagination: PaginationInput,
    filter: Option<UsersFilterInput>,
    search: Option<String>,
}

#[derive(serde::Serialize)]
struct PaginationInput {
    first: i64,
    after: Option<String>,
}

#[derive(serde::Serialize)]
struct UsersFilterInput {
    role: Option<String>,
    status: Option<String>,
}

#[derive(serde::Serialize)]
struct UserVariables {
    id: String,
}

#[derive(serde::Serialize, Default)]
struct OAuthAppsVariables {
    limit: Option<i64>,
}

#[derive(serde::Deserialize)]
struct OAuthAppsResponse {
    #[serde(rename = "oauthApps")]
    oauth_apps: Vec<OAuthApp>,
}

fn cursor_for_page(page: i64, limit: i64) -> String {
    let index = ((page - 1) * limit).saturating_sub(1).max(0);
    STANDARD.encode(index.to_string())
}

async fn fetch_users_graphql(
    page: i64,
    limit: i64,
    search: String,
    role: String,
    status: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlUsersResponse, ApiError> {
    let after = if page > 1 {
        Some(cursor_for_page(page, limit))
    } else {
        None
    };

    request_with_persisted::<UsersVariables, GraphqlUsersResponse>(
        USERS_QUERY,
        UsersVariables {
            pagination: PaginationInput {
                first: limit,
                after,
            },
            filter: Some(UsersFilterInput {
                role: if role.is_empty() {
                    None
                } else {
                    Some(role.to_uppercase())
                },
                status: if status.is_empty() {
                    None
                } else {
                    Some(status.to_uppercase())
                },
            }),
            search: if search.is_empty() {
                None
            } else {
                Some(search)
            },
        },
        USERS_QUERY_HASH,
        token,
        tenant_slug,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))
}

async fn fetch_user_graphql(
    user_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlUserResponse, ApiError> {
    request_with_persisted::<UserVariables, GraphqlUserResponse>(
        USER_DETAILS_QUERY,
        UserVariables { id: user_id },
        USER_DETAILS_QUERY_HASH,
        token,
        tenant_slug,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))
}

async fn list_oauth_apps_graphql(
    token: Option<String>,
    tenant: Option<String>,
) -> Result<Vec<OAuthApp>, ApiError> {
    let response = request::<OAuthAppsVariables, OAuthAppsResponse>(
        OAUTH_APPS_QUERY,
        OAuthAppsVariables { limit: Some(100) },
        token,
        tenant,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.oauth_apps)
}

pub async fn fetch_users(
    page: i64,
    limit: i64,
    search: String,
    role: String,
    status: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlUsersResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::list_users_native(page, limit, search, role, status)
                .await
                .map_err(|err| err.to_string())
        }
        UiTransportPath::Graphql => {
            fetch_users_graphql(page, limit, search, role, status, token, tenant_slug)
                .await
                .map_err(|err| err.to_string())
        }
    }
}

pub async fn fetch_user(
    user_id: String,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<GraphqlUserResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::user_details_native(user_id)
            .await
            .map_err(|err| err.to_string()),
        UiTransportPath::Graphql => fetch_user_graphql(user_id, token, tenant_slug)
            .await
            .map_err(|err| err.to_string()),
    }
}

pub async fn list_oauth_apps(
    token: Option<String>,
    tenant: Option<String>,
) -> Result<Vec<OAuthApp>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::list_oauth_apps_native(100)
            .await
            .map_err(|err| err.to_string()),
        UiTransportPath::Graphql => list_oauth_apps_graphql(token, tenant)
            .await
            .map_err(|err| err.to_string()),
    }
}

pub const UPDATE_PROFILE_MUTATION: &str = r#"
mutation UpdateProfile($input: UpdateProfileInput!) {
    updateProfile(input: $input) {
        id
        email
        name
        role
    }
}
"#;

#[derive(serde::Serialize)]
struct UpdateProfileInput {
    input: ProfileData,
}

#[derive(serde::Serialize)]
struct ProfileData {
    name: Option<String>,
}

#[derive(serde::Deserialize)]
struct UpdateProfileResponse {
    #[serde(rename = "updateProfile")]
    update_profile: native_server_adapter::ProfileUser,
}

pub async fn update_profile(
    token: String,
    tenant: String,
    name: Option<String>,
) -> Result<native_server_adapter::ProfileUser, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::update_profile_native(token, tenant, name)
                .await
                .map_err(|err| err.to_string())
        }
        UiTransportPath::Graphql => update_profile_graphql(token, tenant, name)
            .await
            .map_err(|err| err.to_string()),
    }
}

async fn update_profile_graphql(
    token: String,
    tenant: String,
    name: Option<String>,
) -> Result<native_server_adapter::ProfileUser, ApiError> {
    let response = request::<UpdateProfileInput, UpdateProfileResponse>(
        UPDATE_PROFILE_MUTATION,
        UpdateProfileInput {
            input: ProfileData { name },
        },
        Some(token),
        Some(tenant),
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.update_profile)
}

pub const CHANGE_PASSWORD_MUTATION: &str = r#"
mutation ChangePassword($input: ChangePasswordInput!) {
    changePassword(input: $input) {
        success
    }
}
"#;

#[derive(serde::Serialize)]
struct ChangePasswordVariables {
    input: ChangePasswordInput,
}

#[derive(serde::Serialize)]
struct ChangePasswordInput {
    #[serde(rename = "currentPassword")]
    current_password: String,
    #[serde(rename = "newPassword")]
    new_password: String,
}

#[derive(serde::Deserialize)]
struct ChangePasswordResponse {
    #[serde(rename = "changePassword")]
    change_password: native_server_adapter::SuccessPayload,
}

pub async fn change_password(
    token: String,
    tenant: String,
    current_password: String,
    new_password: String,
) -> Result<native_server_adapter::SuccessPayload, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::change_password_native(
            token,
            tenant,
            current_password,
            new_password,
        )
        .await
        .map_err(|err| err.to_string()),
        UiTransportPath::Graphql => {
            change_password_graphql(token, tenant, current_password, new_password)
                .await
                .map_err(|err| err.to_string())
        }
    }
}

async fn change_password_graphql(
    token: String,
    tenant: String,
    current_password: String,
    new_password: String,
) -> Result<native_server_adapter::SuccessPayload, ApiError> {
    let response = request::<ChangePasswordVariables, ChangePasswordResponse>(
        CHANGE_PASSWORD_MUTATION,
        ChangePasswordVariables {
            input: ChangePasswordInput {
                current_password,
                new_password,
            },
        },
        Some(token),
        Some(tenant),
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.change_password)
}

pub const CREATE_USER_MUTATION: &str = r#"
mutation CreateUser($input: CreateUserInput!) {
    createUser(input: $input) {
        id email name role status createdAt tenantName
    }
}
"#;

#[derive(serde::Serialize)]
struct CreateUserVariables {
    input: crate::model::CreateUserInput,
}

#[derive(serde::Deserialize)]
struct CreateUserResponse {
    #[serde(rename = "createUser")]
    create_user: Option<crate::model::GraphqlUser>,
}

async fn create_user_graphql(
    token: Option<String>,
    tenant: Option<String>,
    input: crate::model::CreateUserInput,
) -> Result<Option<crate::model::GraphqlUser>, ApiError> {
    let response = request::<CreateUserVariables, CreateUserResponse>(
        CREATE_USER_MUTATION,
        CreateUserVariables { input },
        token,
        tenant,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.create_user)
}

pub async fn create_user(
    token: Option<String>,
    tenant: Option<String>,
    input: crate::model::CreateUserInput,
) -> Result<Option<crate::model::GraphqlUser>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::create_user_native(input)
            .await
            .map_err(ApiError::from),
        UiTransportPath::Graphql => create_user_graphql(token, tenant, input).await,
    }
}

pub const UPDATE_USER_MUTATION: &str = r#"
mutation UpdateUser($id: UUID!, $input: UpdateUserInput!) {
    updateUser(id: $id, input: $input) {
        id email name role status createdAt tenantName
    }
}
"#;

pub const DELETE_USER_MUTATION: &str = r#"
mutation DeleteUser($id: UUID!) {
    deleteUser(id: $id) {
        success
    }
}
"#;

#[derive(serde::Serialize)]
struct UpdateUserVariables {
    id: String,
    input: crate::model::UpdateUserInput,
}

#[derive(serde::Deserialize)]
struct UpdateUserResponse {
    #[serde(rename = "updateUser")]
    update_user: Option<crate::model::GraphqlUser>,
}

#[derive(serde::Serialize)]
struct DeleteUserVariables {
    id: String,
}

#[derive(serde::Deserialize)]
struct DeleteUserResponse {
    #[serde(rename = "deleteUser")]
    delete_user: Option<DeleteResult>,
}

#[derive(serde::Deserialize)]
struct DeleteResult {
    success: bool,
}

async fn update_user_graphql(
    token: Option<String>,
    tenant: Option<String>,
    id: String,
    input: crate::model::UpdateUserInput,
) -> Result<Option<crate::model::GraphqlUser>, ApiError> {
    let response = request::<UpdateUserVariables, UpdateUserResponse>(
        UPDATE_USER_MUTATION,
        UpdateUserVariables { id, input },
        token,
        tenant,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.update_user)
}

async fn delete_user_graphql(
    token: Option<String>,
    tenant: Option<String>,
    id: String,
) -> Result<bool, ApiError> {
    let response = request::<DeleteUserVariables, DeleteUserResponse>(
        DELETE_USER_MUTATION,
        DeleteUserVariables { id },
        token,
        tenant,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.delete_user.map(|res| res.success).unwrap_or(false))
}

pub async fn update_user_details(
    token: Option<String>,
    tenant: Option<String>,
    id: String,
    input: crate::model::UpdateUserInput,
) -> Result<Option<crate::model::GraphqlUser>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::update_user_native(id, input)
            .await
            .map_err(ApiError::from),
        UiTransportPath::Graphql => update_user_graphql(token, tenant, id, input).await,
    }
}

pub async fn delete_user_details(
    token: Option<String>,
    tenant: Option<String>,
    id: String,
) -> Result<bool, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::delete_user_native(id)
            .await
            .map_err(ApiError::from),
        UiTransportPath::Graphql => delete_user_graphql(token, tenant, id).await,
    }
}

pub const CREATE_OAUTH_APP_MUTATION: &str = r#"
mutation CreateOAuthApp($input: CreateOAuthAppInput!) {
  createOAuthApp(input: $input) {
    app {
      id
      name
      slug
      description
      iconUrl
      appType
      clientId
      redirectUris
      scopes
      grantTypes
      manifestRef
      autoCreated
      managedByManifest
      isActive
      canEdit
      canRotateSecret
      canRevoke
      activeTokenCount
      lastUsedAt
      createdAt
    }
    clientSecret
  }
}
"#;

pub const UPDATE_OAUTH_APP_MUTATION: &str = r#"
mutation UpdateOAuthApp($id: UUID!, $input: UpdateOAuthAppInput!) {
  updateOAuthApp(id: $id, input: $input) {
    id
    name
    slug
    description
    iconUrl
    appType
    clientId
    redirectUris
    scopes
    grantTypes
    manifestRef
    autoCreated
    managedByManifest
    isActive
    canEdit
    canRotateSecret
    canRevoke
    activeTokenCount
    lastUsedAt
    createdAt
  }
}
"#;

pub const ROTATE_OAUTH_APP_SECRET_MUTATION: &str = r#"
mutation RotateOAuthAppSecret($id: UUID!) {
  rotateOAuthAppSecret(id: $id) {
    app {
      id
      name
      slug
      description
      iconUrl
      appType
      clientId
      redirectUris
      scopes
      grantTypes
      manifestRef
      autoCreated
      managedByManifest
      isActive
      canEdit
      canRotateSecret
      canRevoke
      activeTokenCount
      lastUsedAt
      createdAt
    }
    clientSecret
  }
}
"#;

pub const REVOKE_OAUTH_APP_MUTATION: &str = r#"
mutation RevokeOAuthApp($id: UUID!) {
  revokeOAuthApp(id: $id) {
    id
  }
}
"#;

#[derive(serde::Serialize)]
pub struct CreateOAuthAppVariables {
    pub input: CreateOAuthAppInput,
}

#[derive(serde::Deserialize)]
pub struct CreateOAuthAppResponse {
    #[serde(rename = "createOAuthApp")]
    pub create_oauth_app: CreateOAuthAppResult,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateOAuthAppResult {
    pub app: OAuthApp,
    pub client_secret: String,
}

#[derive(serde::Serialize)]
pub struct UpdateOAuthAppVariables {
    pub id: uuid::Uuid,
    pub input: UpdateOAuthAppInput,
}

#[derive(serde::Deserialize)]
pub struct UpdateOAuthAppResponse {
    #[serde(rename = "updateOAuthApp")]
    pub update_oauth_app: OAuthApp,
}

#[derive(serde::Serialize)]
pub struct OAuthAppIdVariables {
    pub id: uuid::Uuid,
}

#[derive(serde::Deserialize)]
pub struct RotateOAuthAppSecretResponse {
    #[serde(rename = "rotateOAuthAppSecret")]
    pub rotate_oauth_app_secret: CreateOAuthAppResult,
}

#[derive(serde::Deserialize)]
pub struct RevokeOAuthAppResponse {
    #[serde(rename = "revokeOAuthApp")]
    pub revoke_oauth_app: RevokeOAuthAppPayload,
}

#[derive(serde::Deserialize)]
pub struct RevokeOAuthAppPayload {
    pub id: uuid::Uuid,
}

async fn create_oauth_app_graphql(
    token: Option<String>,
    tenant: Option<String>,
    input: CreateOAuthAppInput,
) -> Result<CreateOAuthAppResult, ApiError> {
    let response = request::<CreateOAuthAppVariables, CreateOAuthAppResponse>(
        CREATE_OAUTH_APP_MUTATION,
        CreateOAuthAppVariables { input },
        token,
        tenant,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.create_oauth_app)
}

async fn update_oauth_app_graphql(
    token: Option<String>,
    tenant: Option<String>,
    id: uuid::Uuid,
    input: UpdateOAuthAppInput,
) -> Result<OAuthApp, ApiError> {
    let response = request::<UpdateOAuthAppVariables, UpdateOAuthAppResponse>(
        UPDATE_OAUTH_APP_MUTATION,
        UpdateOAuthAppVariables { id, input },
        token,
        tenant,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.update_oauth_app)
}

async fn rotate_oauth_app_secret_graphql(
    token: Option<String>,
    tenant: Option<String>,
    id: uuid::Uuid,
) -> Result<CreateOAuthAppResult, ApiError> {
    let response = request::<OAuthAppIdVariables, RotateOAuthAppSecretResponse>(
        ROTATE_OAUTH_APP_SECRET_MUTATION,
        OAuthAppIdVariables { id },
        token,
        tenant,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.rotate_oauth_app_secret)
}

async fn revoke_oauth_app_graphql(
    token: Option<String>,
    tenant: Option<String>,
    id: uuid::Uuid,
) -> Result<uuid::Uuid, ApiError> {
    let response = request::<OAuthAppIdVariables, RevokeOAuthAppResponse>(
        REVOKE_OAUTH_APP_MUTATION,
        OAuthAppIdVariables { id },
        token,
        tenant,
    )
    .await
    .map_err(|err| ApiError::Graphql(err.to_string()))?;

    Ok(response.revoke_oauth_app.id)
}

pub async fn create_oauth_app(
    token: Option<String>,
    tenant: Option<String>,
    input: CreateOAuthAppInput,
) -> Result<CreateOAuthAppResult, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::create_oauth_app_native(input)
            .await
            .map_err(ApiError::from),
        UiTransportPath::Graphql => create_oauth_app_graphql(token, tenant, input).await,
    }
}

pub async fn update_oauth_app(
    token: Option<String>,
    tenant: Option<String>,
    id: uuid::Uuid,
    input: UpdateOAuthAppInput,
) -> Result<OAuthApp, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::update_oauth_app_native(id, input)
            .await
            .map_err(ApiError::from),
        UiTransportPath::Graphql => update_oauth_app_graphql(token, tenant, id, input).await,
    }
}

pub async fn rotate_oauth_app_secret(
    token: Option<String>,
    tenant: Option<String>,
    id: uuid::Uuid,
) -> Result<CreateOAuthAppResult, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::rotate_oauth_app_secret_native(id)
            .await
            .map_err(ApiError::from),
        UiTransportPath::Graphql => rotate_oauth_app_secret_graphql(token, tenant, id).await,
    }
}

pub async fn revoke_oauth_app(
    token: Option<String>,
    tenant: Option<String>,
    id: uuid::Uuid,
) -> Result<uuid::Uuid, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::revoke_oauth_app_native(id)
            .await
            .map_err(ApiError::from),
        UiTransportPath::Graphql => revoke_oauth_app_graphql(token, tenant, id).await,
    }
}
