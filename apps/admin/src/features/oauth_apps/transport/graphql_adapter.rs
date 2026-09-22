use crate::entities::oauth_app::model::OAuthApp;
use crate::features::oauth_apps::model::{
    CreateOAuthAppInput, CreateOAuthAppResult, UpdateOAuthAppInput,
};
use crate::shared::api::{ApiError, request};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const OAUTH_APPS_QUERY: &str = r#"
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

const CREATE_OAUTH_APP_MUTATION: &str = r#"
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

const UPDATE_OAUTH_APP_MUTATION: &str = r#"
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

const ROTATE_OAUTH_APP_SECRET_MUTATION: &str = r#"
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

const REVOKE_OAUTH_APP_MUTATION: &str = r#"
mutation RevokeOAuthApp($id: UUID!) {
  revokeOAuthApp(id: $id) {
    id
  }
}
"#;

#[derive(Clone, Debug, Default, Serialize)]
struct OAuthAppsVariables {
    limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
struct OAuthAppsResponse {
    #[serde(rename = "oauthApps")]
    oauth_apps: Vec<OAuthApp>,
}

#[derive(Clone, Debug, Serialize)]
struct CreateOAuthAppVariables {
    input: CreateOAuthAppInput,
}

#[derive(Clone, Debug, Deserialize)]
struct CreateOAuthAppResponse {
    #[serde(rename = "createOAuthApp")]
    create_oauth_app: CreateOAuthAppResult,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateOAuthAppVariables {
    id: Uuid,
    input: UpdateOAuthAppInput,
}

#[derive(Clone, Debug, Deserialize)]
struct UpdateOAuthAppResponse {
    #[serde(rename = "updateOAuthApp")]
    update_oauth_app: OAuthApp,
}

#[derive(Clone, Debug, Serialize)]
struct OAuthAppIdVariables {
    id: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
struct RotateOAuthAppSecretResponse {
    #[serde(rename = "rotateOAuthAppSecret")]
    rotate_oauth_app_secret: CreateOAuthAppResult,
}

#[derive(Clone, Debug, Deserialize)]
struct RevokeOAuthAppResponse {
    #[serde(rename = "revokeOAuthApp")]
    _revoke_oauth_app: RevokeOAuthAppPayload,
}

#[derive(Clone, Debug, Deserialize)]
struct RevokeOAuthAppPayload {
    #[serde(rename = "id")]
    _id: Uuid,
}

pub async fn list_oauth_apps(
    token: Option<String>,
    tenant: Option<String>,
) -> Result<Vec<OAuthApp>, ApiError> {
    let response = request::<OAuthAppsVariables, OAuthAppsResponse>(
        OAUTH_APPS_QUERY,
        OAuthAppsVariables { limit: Some(100) },
        token,
        tenant,
    )
    .await?;

    Ok(response.oauth_apps)
}

pub async fn create_oauth_app(
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
    .await?;

    Ok(response.create_oauth_app)
}

pub async fn update_oauth_app(
    token: Option<String>,
    tenant: Option<String>,
    id: Uuid,
    input: UpdateOAuthAppInput,
) -> Result<OAuthApp, ApiError> {
    let response = request::<UpdateOAuthAppVariables, UpdateOAuthAppResponse>(
        UPDATE_OAUTH_APP_MUTATION,
        UpdateOAuthAppVariables { id, input },
        token,
        tenant,
    )
    .await?;

    Ok(response.update_oauth_app)
}

pub async fn rotate_oauth_app_secret(
    token: Option<String>,
    tenant: Option<String>,
    id: Uuid,
) -> Result<CreateOAuthAppResult, ApiError> {
    let response = request::<OAuthAppIdVariables, RotateOAuthAppSecretResponse>(
        ROTATE_OAUTH_APP_SECRET_MUTATION,
        OAuthAppIdVariables { id },
        token,
        tenant,
    )
    .await?;

    Ok(response.rotate_oauth_app_secret)
}

pub async fn revoke_oauth_app(
    token: Option<String>,
    tenant: Option<String>,
    id: Uuid,
) -> Result<(), ApiError> {
    let _response = request::<OAuthAppIdVariables, RevokeOAuthAppResponse>(
        REVOKE_OAUTH_APP_MUTATION,
        OAuthAppIdVariables { id },
        token,
        tenant,
    )
    .await?;

    Ok(())
}
