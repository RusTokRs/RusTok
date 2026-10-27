use rustok_graphql::{GraphqlRequest, execute};
use serde_json::json;

use super::{
    CURRENT_USER_QUERY, CurrentUserResponse, FORGOT_PASSWORD_MUTATION, ForgotPasswordResponse,
    get_graphql_url, map_graphql_auth_error,
};
use crate::{AuthError, AuthUser};

pub(super) async fn forgot_password_graphql(
    email: String,
    tenant: String,
) -> Result<String, AuthError> {
    let variables = json!({
        "input": {
            "email": email,
        }
    });

    let response: ForgotPasswordResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(FORGOT_PASSWORD_MUTATION, Some(variables)),
        None,
        Some(tenant),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    Ok(response.forgot_password.message)
}

pub(super) async fn fetch_current_user_graphql(
    token: String,
    tenant: String,
) -> Result<Option<AuthUser>, AuthError> {
    let response: CurrentUserResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(CURRENT_USER_QUERY, None::<serde_json::Value>),
        Some(token),
        Some(tenant),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    Ok(response.me.map(|user| AuthUser {
        id: user.id,
        email: user.email,
        name: user.name,
        role: user.role,
    }))
}
