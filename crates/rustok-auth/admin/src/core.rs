use chrono::{DateTime, Utc};

use crate::model::{
    AppType, CreateOAuthAppInput, CreateUserInput, UpdateOAuthAppInput, UpdateUserInput,
};

// Leptos-free copy definitions and auth UI helpers.
pub struct AuthCopy {
    pub title: String,
    pub subtitle: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateUserInputError {
    MissingCredentials,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthFormInputError {
    MissingRequiredFields,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginRequest {
    pub tenant: String,
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterRequest {
    pub tenant: String,
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordResetRequest {
    pub tenant: String,
    pub email: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileUpdateErrorKind {
    Unauthorized,
    Http,
    Network,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OAuthAppTypeDefaults {
    pub redirect_uris: &'static str,
    pub grant_types: &'static str,
}

pub fn user_list_page(raw: Option<&str>) -> i64 {
    raw.and_then(|value| value.parse::<i64>().ok())
        .filter(|page| *page > 0)
        .unwrap_or(1)
}

pub fn user_list_query_params(
    search: String,
    role: String,
    status: String,
    page: i64,
) -> Vec<(&'static str, String)> {
    let mut params = Vec::new();
    push_non_empty_query(&mut params, "search", search);
    push_non_empty_query(&mut params, "role", role);
    push_non_empty_query(&mut params, "status", status);
    if page > 1 {
        params.push(("page", page.to_string()));
    }
    params
}

pub fn prepare_create_user_input(
    email: String,
    password: String,
    name: String,
    role: String,
    status: String,
) -> Result<CreateUserInput, CreateUserInputError> {
    if email.is_empty() || password.is_empty() {
        return Err(CreateUserInputError::MissingCredentials);
    }

    Ok(CreateUserInput {
        email,
        password,
        name: optional_value(name),
        role: optional_value(role).map(|value| value.to_uppercase()),
        status: optional_value(status).map(|value| value.to_uppercase()),
    })
}

pub fn prepare_update_user_input(name: String, role: String, status: String) -> UpdateUserInput {
    UpdateUserInput {
        name: optional_value(name),
        role,
        status,
    }
}

pub fn prepare_login_request(
    tenant: String,
    email: String,
    password: String,
) -> Result<LoginRequest, AuthFormInputError> {
    let tenant = tenant.trim().to_string();
    let email = email.trim().to_string();
    if tenant.is_empty() || email.is_empty() || password.is_empty() {
        return Err(AuthFormInputError::MissingRequiredFields);
    }
    Ok(LoginRequest {
        tenant,
        email,
        password,
    })
}

pub fn prepare_register_request(
    tenant: String,
    email: String,
    password: String,
    name: String,
) -> Result<RegisterRequest, AuthFormInputError> {
    let login = prepare_login_request(tenant, email, password)?;
    Ok(RegisterRequest {
        tenant: login.tenant,
        email: login.email,
        password: login.password,
        name: optional_trimmed(name),
    })
}

pub fn prepare_password_reset_request(
    tenant: String,
    email: String,
) -> Result<PasswordResetRequest, AuthFormInputError> {
    let tenant = tenant.trim().to_string();
    let email = email.trim().to_string();
    if tenant.is_empty() || email.is_empty() {
        return Err(AuthFormInputError::MissingRequiredFields);
    }
    Ok(PasswordResetRequest { tenant, email })
}

pub fn prepare_profile_name(name: String) -> Option<String> {
    optional_trimmed(name)
}

pub fn classify_profile_update_error(error: &str) -> ProfileUpdateErrorKind {
    if error.contains("Unauthorized") {
        ProfileUpdateErrorKind::Unauthorized
    } else if error.contains("HTTP") {
        ProfileUpdateErrorKind::Http
    } else if error.contains("Network") {
        ProfileUpdateErrorKind::Network
    } else {
        ProfileUpdateErrorKind::Unknown
    }
}

pub fn oauth_app_type_defaults(app_type: &str) -> OAuthAppTypeDefaults {
    match app_type {
        "Mobile" => OAuthAppTypeDefaults {
            redirect_uris: "myapp://auth/callback",
            grant_types: "authorization_code\nrefresh_token",
        },
        "Service" => OAuthAppTypeDefaults {
            redirect_uris: "",
            grant_types: "client_credentials",
        },
        _ => OAuthAppTypeDefaults {
            redirect_uris: "http://localhost:3000/auth/callback",
            grant_types: "authorization_code\nrefresh_token",
        },
    }
}

pub fn prepare_create_oauth_app_input(
    name: String,
    slug: String,
    description: String,
    icon_url: String,
    app_type: String,
    redirect_uris: String,
    scopes: String,
    grant_types: String,
) -> CreateOAuthAppInput {
    let redirect_uris = normalize_lines(&redirect_uris);
    CreateOAuthAppInput {
        name: name.trim().to_string(),
        slug: slug.trim().to_string(),
        description: optional_trimmed(description),
        icon_url: optional_trimmed(icon_url),
        app_type: match app_type.as_str() {
            "Mobile" => AppType::Mobile,
            "Service" => AppType::Service,
            _ => AppType::ThirdParty,
        },
        redirect_uris: (!redirect_uris.is_empty()).then_some(redirect_uris),
        scopes: normalize_lines(&scopes),
        grant_types: normalize_lines(&grant_types),
    }
}

pub fn prepare_update_oauth_app_input(
    name: String,
    description: String,
    icon_url: String,
    redirect_uris: String,
    scopes: String,
    grant_types: String,
) -> UpdateOAuthAppInput {
    UpdateOAuthAppInput {
        name: name.trim().to_string(),
        description: optional_trimmed(description),
        icon_url: optional_trimmed(icon_url),
        redirect_uris: normalize_lines(&redirect_uris),
        scopes: normalize_lines(&scopes),
        grant_types: normalize_lines(&grant_types),
    }
}

pub fn format_oauth_app_timestamp(value: Option<DateTime<Utc>>) -> String {
    value
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "Never".to_string())
}

fn push_non_empty_query(
    params: &mut Vec<(&'static str, String)>,
    key: &'static str,
    value: String,
) {
    if !value.is_empty() {
        params.push((key, value));
    }
}

fn optional_value(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn optional_trimmed(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn normalize_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_page_rejects_missing_invalid_and_non_positive_values() {
        assert_eq!(user_list_page(None), 1);
        assert_eq!(user_list_page(Some("invalid")), 1);
        assert_eq!(user_list_page(Some("0")), 1);
        assert_eq!(user_list_page(Some("3")), 3);
    }

    #[test]
    fn list_query_omits_default_and_empty_values() {
        assert_eq!(
            user_list_query_params("alice".into(), String::new(), "active".into(), 2),
            vec![
                ("search", "alice".to_string()),
                ("status", "active".to_string()),
                ("page", "2".to_string()),
            ]
        );
        assert!(user_list_query_params(String::new(), String::new(), String::new(), 1).is_empty());
    }

    #[test]
    fn create_user_input_requires_credentials_and_normalizes_enums() {
        assert_eq!(
            prepare_create_user_input(
                String::new(),
                "secret".into(),
                String::new(),
                String::new(),
                String::new(),
            ),
            Err(CreateUserInputError::MissingCredentials)
        );

        let input = prepare_create_user_input(
            "user@example.com".into(),
            "secret".into(),
            "Alice".into(),
            "manager".into(),
            "active".into(),
        )
        .expect("valid input");
        assert_eq!(input.name.as_deref(), Some("Alice"));
        assert_eq!(input.role.as_deref(), Some("MANAGER"));
        assert_eq!(input.status.as_deref(), Some("ACTIVE"));
    }

    #[test]
    fn update_user_input_keeps_required_role_and_status() {
        let input = prepare_update_user_input(String::new(), "ADMIN".into(), "ACTIVE".into());
        assert_eq!(input.name, None);
        assert_eq!(input.role, "ADMIN");
        assert_eq!(input.status, "ACTIVE");
    }

    #[test]
    fn auth_form_requests_trim_identity_fields_and_preserve_passwords() {
        let login = prepare_login_request(
            " demo ".into(),
            " admin@example.com ".into(),
            " password with spaces ".into(),
        )
        .expect("valid login");
        assert_eq!(login.tenant, "demo");
        assert_eq!(login.email, "admin@example.com");
        assert_eq!(login.password, " password with spaces ");

        let register = prepare_register_request(
            " demo ".into(),
            " user@example.com ".into(),
            "secret".into(),
            " Alice ".into(),
        )
        .expect("valid registration");
        assert_eq!(register.name.as_deref(), Some("Alice"));
    }

    #[test]
    fn auth_form_requests_reject_whitespace_only_required_fields() {
        assert_eq!(
            prepare_login_request(" ".into(), "user@example.com".into(), "secret".into()),
            Err(AuthFormInputError::MissingRequiredFields)
        );
        assert_eq!(
            prepare_password_reset_request("demo".into(), "  ".into()),
            Err(AuthFormInputError::MissingRequiredFields)
        );
    }

    #[test]
    fn profile_name_normalization_and_error_classification_are_stable() {
        assert_eq!(
            prepare_profile_name(" Alice ".into()).as_deref(),
            Some("Alice")
        );
        assert_eq!(prepare_profile_name("  ".into()), None);
        assert_eq!(
            classify_profile_update_error("Unauthorized request"),
            ProfileUpdateErrorKind::Unauthorized
        );
        assert_eq!(
            classify_profile_update_error("HTTP 500"),
            ProfileUpdateErrorKind::Http
        );
        assert_eq!(
            classify_profile_update_error("Network unavailable"),
            ProfileUpdateErrorKind::Network
        );
        assert_eq!(
            classify_profile_update_error("unexpected"),
            ProfileUpdateErrorKind::Unknown
        );
    }

    #[test]
    fn oauth_app_defaults_follow_app_type_policy() {
        assert_eq!(
            oauth_app_type_defaults("Mobile"),
            OAuthAppTypeDefaults {
                redirect_uris: "myapp://auth/callback",
                grant_types: "authorization_code\nrefresh_token",
            }
        );
        assert_eq!(
            oauth_app_type_defaults("Service"),
            OAuthAppTypeDefaults {
                redirect_uris: "",
                grant_types: "client_credentials",
            }
        );
    }

    #[test]
    fn create_oauth_app_input_normalizes_form_values() {
        let input = prepare_create_oauth_app_input(
            " Integration ".into(),
            " com.example.app ".into(),
            "  Description  ".into(),
            "   ".into(),
            "Mobile".into(),
            " myapp://one \n\n myapp://two ".into(),
            " read \n write ".into(),
            " authorization_code ".into(),
        );

        assert_eq!(input.name, "Integration");
        assert_eq!(input.slug, "com.example.app");
        assert_eq!(input.description.as_deref(), Some("Description"));
        assert_eq!(input.icon_url, None);
        assert_eq!(input.app_type, AppType::Mobile);
        assert_eq!(
            input.redirect_uris,
            Some(vec!["myapp://one".to_string(), "myapp://two".to_string()])
        );
        assert_eq!(input.scopes, vec!["read".to_string(), "write".to_string()]);
    }

    #[test]
    fn update_oauth_app_input_preserves_empty_collections() {
        let input = prepare_update_oauth_app_input(
            " App ".into(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            " client_credentials ".into(),
        );

        assert_eq!(input.name, "App");
        assert_eq!(input.description, None);
        assert!(input.redirect_uris.is_empty());
        assert!(input.scopes.is_empty());
        assert_eq!(input.grant_types, vec!["client_credentials".to_string()]);
    }

    #[test]
    fn oauth_app_timestamp_uses_stable_never_fallback() {
        assert_eq!(format_oauth_app_timestamp(None), "Never");
    }
}
