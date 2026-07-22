use rustok_graphql::{GraphqlRequest, execute};
use serde_json::json;

use super::{
    CURRENT_USER_QUERY, CurrentUserResponse, FORGOT_PASSWORD_MUTATION, ForgotPasswordResponse,
    REFRESH_TOKEN_MUTATION, RefreshTokenResponse, SIGN_IN_MUTATION, SIGN_OUT_MUTATION,
    SIGN_UP_MUTATION, SignInResponse, SignOutResponse, SignUpResponse, auth_payload_to_session,
    get_graphql_url, map_graphql_auth_error,
};
use crate::{AuthError, AuthSession, AuthUser};

pub(super) async fn sign_in_graphql(
    email: String,
    password: String,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    let variables = json!({
        "input": {
            "email": email,
            "password": password,
        }
    });

    let response: SignInResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(SIGN_IN_MUTATION, Some(variables)),
        None,
        Some(tenant.clone()),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, true))?;

    Ok(auth_payload_to_session(response.sign_in, tenant))
}

pub(super) async fn sign_up_graphql(
    email: String,
    password: String,
    name: Option<String>,
    tenant: String,
) -> Result<(AuthUser, AuthSession), AuthError> {
    let variables = json!({
        "input": {
            "email": email,
            "password": password,
            "name": name,
        }
    });

    let response: SignUpResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(SIGN_UP_MUTATION, Some(variables)),
        None,
        Some(tenant.clone()),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    Ok(auth_payload_to_session(response.sign_up, tenant))
}

pub(super) async fn sign_out_graphql(token: String, tenant: String) -> Result<(), AuthError> {
    let _response: SignOutResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(SIGN_OUT_MUTATION, None::<serde_json::Value>),
        Some(token),
        Some(tenant),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    Ok(())
}

pub(super) async fn refresh_token_graphql(
    refresh_tok: String,
    tenant: String,
) -> Result<(AuthSession, AuthUser), AuthError> {
    let variables = json!({
        "input": {
            "refreshToken": refresh_tok,
        }
    });

    let response: RefreshTokenResponse = execute(
        &get_graphql_url(),
        GraphqlRequest::new(REFRESH_TOKEN_MUTATION, Some(variables)),
        None,
        Some(tenant.clone()),
        None,
    )
    .await
    .map_err(|error| map_graphql_auth_error(error, false))?;

    let (user, session) = auth_payload_to_session(response.refresh_token, tenant);
    Ok((session, user))
}

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
