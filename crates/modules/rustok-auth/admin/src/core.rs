use chrono::{DateTime, Utc};
use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
};

use crate::model::{
    AppType, CreateOAuthAppInput, CreateUserInput, GraphqlUser, UpdateOAuthAppInput,
    UpdateUserInput,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangePasswordRequest {
    pub token: String,
    pub tenant: String,
    pub current_password: String,
    pub new_password: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePasswordInputError {
    MissingPasswords,
    Unauthorized,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthTransportErrorKind {
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

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthAppListItemViewModel {
    pub app: crate::model::OAuthApp,
    pub description: Option<String>,
    pub scopes_summary: String,
    pub grants_summary: String,
    pub capability_label: &'static str,
    pub client_id: String,
    pub last_used_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserEditFormValues {
    pub name: String,
    pub role: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphqlUserViewModel {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub created_at: String,
    pub tenant_name: String,
    pub detail_href: String,
    pub is_active: bool,
    pub edit_form: UserEditFormValues,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserListPagination {
    pub page: i64,
    pub previous_page: i64,
    pub can_previous: bool,
    pub can_next: bool,
}

pub fn user_list_page(raw: Option<&str>) -> i64 {
    raw.and_then(|value| value.parse::<i64>().ok())
        .filter(|page| *page > 0)
        .unwrap_or(1)
}

pub fn user_list_pagination(page: i64, limit: i64, total: i64) -> UserListPagination {
    let page = page.max(1);
    let limit = limit.max(1);
    UserListPagination {
        page,
        previous_page: user_list_previous_page(page),
        can_previous: page > 1,
        can_next: page.saturating_mul(limit) < total.max(0),
    }
}

pub fn user_list_previous_page(page: i64) -> i64 {
    (page.max(1) - 1).max(1)
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

pub fn user_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("email", if is_ru { "Эл. почта" } else { "Email" })
            .width(240)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Фильтр почты...".to_string()
                } else {
                    "Filter email...".to_string()
                }),
            }),
        GridColumnDef::new("name", if is_ru { "Имя" } else { "Name" })
            .width(200)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Фильтр имени...".to_string()
                } else {
                    "Filter name...".to_string()
                }),
            }),
        GridColumnDef::new("role", if is_ru { "Роль" } else { "Role" })
            .width(140)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Select {
                options: vec![
                    FilterOption {
                        value: "ADMIN".to_string(),
                        label: if is_ru {
                            "Администратор".to_string()
                        } else {
                            "Admin".to_string()
                        },
                    },
                    FilterOption {
                        value: "EDITOR".to_string(),
                        label: if is_ru {
                            "Редактор".to_string()
                        } else {
                            "Editor".to_string()
                        },
                    },
                    FilterOption {
                        value: "CUSTOMER".to_string(),
                        label: if is_ru {
                            "Клиент".to_string()
                        } else {
                            "Customer".to_string()
                        },
                    },
                    FilterOption {
                        value: "VIEWER".to_string(),
                        label: if is_ru {
                            "Наблюдатель".to_string()
                        } else {
                            "Viewer".to_string()
                        },
                    },
                ],
                placeholder: Some(if is_ru {
                    "Все роли".to_string()
                } else {
                    "All roles".to_string()
                }),
            }),
        GridColumnDef::new("status", if is_ru { "Статус" } else { "Status" })
            .width(130)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Select {
                options: vec![
                    FilterOption {
                        value: "active".to_string(),
                        label: if is_ru {
                            "Активен".to_string()
                        } else {
                            "Active".to_string()
                        },
                    },
                    FilterOption {
                        value: "disabled".to_string(),
                        label: if is_ru {
                            "Отключен".to_string()
                        } else {
                            "Disabled".to_string()
                        },
                    },
                    FilterOption {
                        value: "pending".to_string(),
                        label: if is_ru {
                            "В ожидании".to_string()
                        } else {
                            "Pending".to_string()
                        },
                    },
                ],
                placeholder: Some(if is_ru {
                    "Все статусы".to_string()
                } else {
                    "All statuses".to_string()
                }),
            }),
        GridColumnDef::new("created_at", if is_ru { "Создан" } else { "Created" })
            .width(160)
            .align(ColumnAlign::Right),
        GridColumnDef::new("actions", if is_ru { "Действия" } else { "Actions" })
            .width(100)
            .align(ColumnAlign::Right)
            .not_sortable(),
    ]
}

pub fn matches_user_filter(user: &GraphqlUserViewModel, filters: &ColumnFilters) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("email", FilterValue::Text(q)) => {
                if !user.email.to_lowercase().contains(&q.to_lowercase()) {
                    return false;
                }
            }
            ("name", FilterValue::Text(q)) => {
                if !user.name.to_lowercase().contains(&q.to_lowercase()) {
                    return false;
                }
            }
            ("role", FilterValue::Select(r)) => {
                if !user.role.eq_ignore_ascii_case(r) {
                    return false;
                }
            }
            ("status", FilterValue::Select(s)) if !user.status.eq_ignore_ascii_case(s) => {
                return false;
            }
            _ => {}
        }
    }
    true
}

pub fn filter_users(
    users: &[GraphqlUserViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<GraphqlUserViewModel> {
    let search_term = search
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    users
        .iter()
        .filter(|user| {
            if let Some(ref term) = search_term {
                let matches_global = user.email.to_lowercase().contains(term)
                    || user.name.to_lowercase().contains(term)
                    || user.role.to_lowercase().contains(term);
                if !matches_global {
                    return false;
                }
            }

            matches_user_filter(user, filters)
        })
        .cloned()
        .collect()
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

pub fn prepare_change_password_request(
    token: Option<String>,
    tenant: Option<String>,
    current_password: String,
    new_password: String,
) -> Result<ChangePasswordRequest, ChangePasswordInputError> {
    if current_password.is_empty() || new_password.is_empty() {
        return Err(ChangePasswordInputError::MissingPasswords);
    }

    let token = token.ok_or(ChangePasswordInputError::Unauthorized)?;
    Ok(ChangePasswordRequest {
        token,
        tenant: tenant.unwrap_or_default(),
        current_password,
        new_password,
    })
}

pub fn prepare_profile_name(name: String) -> Option<String> {
    optional_trimmed(name)
}

pub fn initial_profile_preferred_locale(host_locale: Option<&str>) -> String {
    match host_locale
        .and_then(|locale| locale.split(['-', '_']).next())
        .map(|language| language.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("ru") => "ru".to_string(),
        _ => "en".to_string(),
    }
}

pub fn classify_auth_transport_error(error: &str) -> AuthTransportErrorKind {
    if error.contains("Unauthorized") {
        AuthTransportErrorKind::Unauthorized
    } else if error.contains("HTTP") {
        AuthTransportErrorKind::Http
    } else if error.contains("Network") {
        AuthTransportErrorKind::Network
    } else {
        AuthTransportErrorKind::Unknown
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

pub struct CreateOAuthAppForm {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub icon_url: String,
    pub app_type: String,
    pub redirect_uris: String,
    pub scopes: String,
    pub grant_types: String,
}

pub fn prepare_create_oauth_app_input(form: CreateOAuthAppForm) -> CreateOAuthAppInput {
    let redirect_uris = normalize_lines(&form.redirect_uris);
    CreateOAuthAppInput {
        name: form.name.trim().to_string(),
        slug: form.slug.trim().to_string(),
        description: optional_trimmed(form.description),
        icon_url: optional_trimmed(form.icon_url),
        app_type: match form.app_type.as_str() {
            "Mobile" => AppType::Mobile,
            "Service" => AppType::Service,
            _ => AppType::ThirdParty,
        },
        redirect_uris: (!redirect_uris.is_empty()).then_some(redirect_uris),
        scopes: normalize_lines(&form.scopes),
        grant_types: normalize_lines(&form.grant_types),
        granted_permissions: Vec::new(),
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
        granted_permissions: Vec::new(),
    }
}

pub fn format_oauth_app_timestamp(value: Option<DateTime<Utc>>) -> String {
    value
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "Never".to_string())
}

pub fn oauth_app_list_item_view(app: crate::model::OAuthApp) -> OAuthAppListItemViewModel {
    let description = app.description.clone().filter(|value| !value.is_empty());
    let scopes_summary = list_summary(&app.scopes);
    let grants_summary = list_summary(&app.grant_types);
    let capability_label = if app.managed_by_manifest {
        "Managed by config/manifest"
    } else {
        "Manual app"
    };
    let client_id = app.client_id.to_string();
    let last_used_at = format_oauth_app_timestamp(app.last_used_at);

    OAuthAppListItemViewModel {
        app,
        description,
        scopes_summary,
        grants_summary,
        capability_label,
        client_id,
        last_used_at,
    }
}

pub fn oauth_app_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("name", if is_ru { "Приложение" } else { "App" })
            .width(240)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Фильтр приложений...".to_string()
                } else {
                    "Filter apps...".to_string()
                }),
            }),
        GridColumnDef::new("type", if is_ru { "Тип" } else { "Type" })
            .width(130)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Select {
                options: vec![
                    FilterOption {
                        value: "ThirdParty".to_string(),
                        label: if is_ru {
                            "Стороннее".to_string()
                        } else {
                            "Third Party".to_string()
                        },
                    },
                    FilterOption {
                        value: "FirstParty".to_string(),
                        label: if is_ru {
                            "Собственное".to_string()
                        } else {
                            "First Party".to_string()
                        },
                    },
                    FilterOption {
                        value: "Mobile".to_string(),
                        label: if is_ru {
                            "Мобильное".to_string()
                        } else {
                            "Mobile".to_string()
                        },
                    },
                    FilterOption {
                        value: "Service".to_string(),
                        label: if is_ru {
                            "Служебное".to_string()
                        } else {
                            "Service".to_string()
                        },
                    },
                    FilterOption {
                        value: "Embedded".to_string(),
                        label: if is_ru {
                            "Встроенное".to_string()
                        } else {
                            "Embedded".to_string()
                        },
                    },
                ],
                placeholder: Some(if is_ru {
                    "Все типы".to_string()
                } else {
                    "All types".to_string()
                }),
            }),
        GridColumnDef::new(
            "scopes_grants",
            if is_ru {
                "Области / Доступы"
            } else {
                "Scopes / Grants"
            },
        )
        .width(220)
        .align(ColumnAlign::Left),
        GridColumnDef::new(
            "client_id",
            if is_ru {
                "ID клиента"
            } else {
                "Client ID"
            },
        )
        .width(160)
        .align(ColumnAlign::Left),
        GridColumnDef::new("tokens", if is_ru { "Токены" } else { "Tokens" })
            .width(90)
            .align(ColumnAlign::Right),
        GridColumnDef::new(
            "last_used",
            if is_ru {
                "Использовано"
            } else {
                "Last Used"
            },
        )
        .width(140)
        .align(ColumnAlign::Right),
        GridColumnDef::new("actions", if is_ru { "Действия" } else { "Actions" })
            .width(240)
            .align(ColumnAlign::Right)
            .not_sortable(),
    ]
}

pub fn matches_oauth_app_filter(item: &OAuthAppListItemViewModel, filters: &ColumnFilters) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("name", FilterValue::Text(q)) => {
                let term = q.to_lowercase();
                if !item.app.name.to_lowercase().contains(&term)
                    && !item.app.slug.to_lowercase().contains(&term)
                    && !item.client_id.to_lowercase().contains(&term)
                {
                    return false;
                }
            }
            ("type", FilterValue::Select(s)) => {
                let app_type_str = format!("{:?}", item.app.app_type);
                if !app_type_str.eq_ignore_ascii_case(s) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_oauth_apps(
    apps: &[OAuthAppListItemViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<OAuthAppListItemViewModel> {
    let search_term = search
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    apps.iter()
        .filter(|item| {
            if let Some(ref term) = search_term {
                let matches_global = item.app.name.to_lowercase().contains(term)
                    || item.app.slug.to_lowercase().contains(term)
                    || item.client_id.to_lowercase().contains(term)
                    || item
                        .description
                        .as_deref()
                        .map(|d| d.to_lowercase().contains(term))
                        .unwrap_or(false);
                if !matches_global {
                    return false;
                }
            }

            matches_oauth_app_filter(item, filters)
        })
        .cloned()
        .collect()
}

pub fn graphql_user_view(user: GraphqlUser, missing_value: String) -> GraphqlUserViewModel {
    let edit_form = UserEditFormValues {
        name: user.name.clone().unwrap_or_default(),
        role: user.role.clone(),
        status: user.status.clone(),
    };

    GraphqlUserViewModel {
        detail_href: format!("/users/{}", user.id),
        is_active: user.status.eq_ignore_ascii_case("active"),
        id: user.id,
        email: user.email,
        name: user.name.unwrap_or_else(|| missing_value.clone()),
        role: user.role,
        status: user.status,
        created_at: user.created_at,
        tenant_name: user.tenant_name.unwrap_or(missing_value),
        edit_form,
    }
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

fn list_summary(values: &[String]) -> String {
    if values.is_empty() {
        "None".to_string()
    } else {
        values.join(", ")
    }
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
    fn list_pagination_bounds_previous_and_next_navigation() {
        assert_eq!(
            user_list_pagination(0, 0, 12),
            UserListPagination {
                page: 1,
                previous_page: 1,
                can_previous: false,
                can_next: true,
            }
        );
        assert_eq!(
            user_list_pagination(2, 12, 24),
            UserListPagination {
                page: 2,
                previous_page: 1,
                can_previous: true,
                can_next: false,
            }
        );
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

        let generated_password = format!(
            "pw-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let register = prepare_register_request(
            " demo ".into(),
            " user@example.com ".into(),
            generated_password,
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
            initial_profile_preferred_locale(Some("ru-RU")).as_str(),
            "ru"
        );
        assert_eq!(
            initial_profile_preferred_locale(Some("en-US")).as_str(),
            "en"
        );
        assert_eq!(initial_profile_preferred_locale(None).as_str(), "en");
        assert_eq!(
            classify_auth_transport_error("Unauthorized request"),
            AuthTransportErrorKind::Unauthorized
        );
        assert_eq!(
            classify_auth_transport_error("HTTP 500"),
            AuthTransportErrorKind::Http
        );
        assert_eq!(
            classify_auth_transport_error("Network unavailable"),
            AuthTransportErrorKind::Network
        );
        assert_eq!(
            classify_auth_transport_error("unexpected"),
            AuthTransportErrorKind::Unknown
        );
    }

    #[test]
    fn change_password_request_preserves_credentials_and_requires_auth() {
        assert_eq!(
            prepare_change_password_request(
                Some("token".into()),
                Some("tenant".into()),
                String::new(),
                "new-secret".into(),
            ),
            Err(ChangePasswordInputError::MissingPasswords)
        );
        assert_eq!(
            prepare_change_password_request(
                None,
                Some("tenant".into()),
                "current-secret".into(),
                "new-secret".into(),
            ),
            Err(ChangePasswordInputError::Unauthorized)
        );

        let request = prepare_change_password_request(
            Some("token".into()),
            None,
            " current secret ".into(),
            " new secret ".into(),
        )
        .expect("valid request");
        assert_eq!(request.tenant, "");
        assert_eq!(request.current_password, " current secret ");
        assert_eq!(request.new_password, " new secret ");
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
        let input = prepare_create_oauth_app_input(CreateOAuthAppForm {
            name: " Integration ".into(),
            slug: " com.example.app ".into(),
            description: "  Description  ".into(),
            icon_url: "   ".into(),
            app_type: "Mobile".into(),
            redirect_uris: " myapp://one \n\n myapp://two ".into(),
            scopes: " read \n write ".into(),
            grant_types: " authorization_code ".into(),
        });

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

    #[test]
    fn oauth_app_list_item_view_owns_summary_and_capability_fallbacks() {
        let app = crate::model::OAuthApp {
            id: uuid::Uuid::nil(),
            name: "Managed app".into(),
            slug: "managed-app".into(),
            description: Some(String::new()),
            icon_url: None,
            app_type: AppType::FirstParty,
            client_id: uuid::Uuid::nil(),
            redirect_uris: Vec::new(),
            scopes: vec!["read".into(), "write".into()],
            grant_types: Vec::new(),
            manifest_ref: Some("module.toml".into()),
            auto_created: true,
            managed_by_manifest: true,
            is_active: true,
            can_edit: false,
            can_rotate_secret: false,
            can_revoke: false,
            active_token_count: 0,
            last_used_at: None,
            created_at: Utc::now(),
        };

        let view = oauth_app_list_item_view(app);
        assert_eq!(view.description, None);
        assert_eq!(view.scopes_summary, "read, write");
        assert_eq!(view.grants_summary, "None");
        assert_eq!(view.capability_label, "Managed by config/manifest");
        assert_eq!(view.client_id, uuid::Uuid::nil().to_string());
        assert_eq!(view.last_used_at, "Never");
    }

    #[test]
    fn graphql_user_view_is_shared_by_list_detail_and_edit_surfaces() {
        let view = graphql_user_view(
            GraphqlUser {
                id: "user-1".into(),
                email: "user@example.com".into(),
                name: None,
                role: "ADMIN".into(),
                status: "active".into(),
                created_at: "2026-06-29".into(),
                tenant_name: None,
            },
            "—".into(),
        );

        assert_eq!(view.name, "—");
        assert_eq!(view.tenant_name, "—");
        assert_eq!(view.detail_href, "/users/user-1");
        assert!(view.is_active);
        assert_eq!(view.edit_form.name, "");
        assert_eq!(view.edit_form.role, "ADMIN");
        assert_eq!(view.edit_form.status, "active");
    }

    #[test]
    fn user_grid_columns_localization() {
        let cols_en = user_grid_columns(Some("en"));
        assert_eq!(cols_en[0].title, "Email");
        assert_eq!(cols_en[1].title, "Name");
        assert_eq!(cols_en[2].title, "Role");
        assert_eq!(cols_en[3].title, "Status");

        let cols_ru = user_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Эл. почта");
        assert_eq!(cols_ru[1].title, "Имя");
        assert_eq!(cols_ru[2].title, "Роль");
        assert_eq!(cols_ru[3].title, "Статус");
    }

    #[test]
    fn filter_users_by_search_and_role_and_status() {
        let u1 = GraphqlUserViewModel {
            id: "1".into(),
            email: "alice@example.com".into(),
            name: "Alice Smith".into(),
            role: "ADMIN".into(),
            status: "active".into(),
            created_at: "2026-01-01".into(),
            tenant_name: "Tenant 1".into(),
            detail_href: "/users/1".into(),
            is_active: true,
            edit_form: UserEditFormValues {
                name: "Alice Smith".into(),
                role: "ADMIN".into(),
                status: "active".into(),
            },
        };
        let u2 = GraphqlUserViewModel {
            id: "2".into(),
            email: "bob@example.com".into(),
            name: "Bob Jones".into(),
            role: "EDITOR".into(),
            status: "disabled".into(),
            created_at: "2026-02-01".into(),
            tenant_name: "Tenant 1".into(),
            detail_href: "/users/2".into(),
            is_active: false,
            edit_form: UserEditFormValues {
                name: "Bob Jones".into(),
                role: "EDITOR".into(),
                status: "disabled".into(),
            },
        };
        let list = vec![u1.clone(), u2.clone()];

        // Filter by search
        let res = filter_users(&list, &ColumnFilters::new(), Some("alice"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");

        // Filter by role
        let mut filters = ColumnFilters::new();
        filters.set("role", FilterValue::Select("EDITOR".into()));
        let res = filter_users(&list, &filters, None);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "2");

        // Filter by status
        let mut filters = ColumnFilters::new();
        filters.set("status", FilterValue::Select("active".into()));
        let res = filter_users(&list, &filters, None);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");
    }

    #[test]
    fn oauth_app_grid_columns_localization() {
        let cols_en = oauth_app_grid_columns(Some("en"));
        assert_eq!(cols_en[0].title, "App");
        assert_eq!(cols_en[1].title, "Type");
        assert_eq!(cols_en[6].title, "Actions");

        let cols_ru = oauth_app_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Приложение");
        assert_eq!(cols_ru[1].title, "Тип");
        assert_eq!(cols_ru[6].title, "Действия");
    }

    #[test]
    fn filter_oauth_apps_by_search_and_type() {
        let app1 = crate::model::OAuthApp {
            id: uuid::Uuid::nil(),
            name: "Mobile Client".into(),
            slug: "mobile-client".into(),
            description: Some("Android and iOS app".into()),
            icon_url: None,
            app_type: AppType::Mobile,
            client_id: uuid::Uuid::nil(),
            redirect_uris: vec![],
            scopes: vec!["read".into()],
            grant_types: vec!["authorization_code".into()],
            manifest_ref: None,
            auto_created: false,
            managed_by_manifest: false,
            is_active: true,
            can_edit: true,
            can_rotate_secret: true,
            can_revoke: true,
            active_token_count: 5,
            last_used_at: None,
            created_at: Utc::now(),
        };
        let app2 = crate::model::OAuthApp {
            id: uuid::Uuid::new_v4(),
            name: "Third Party ERP".into(),
            slug: "erp-sync".into(),
            description: None,
            icon_url: None,
            app_type: AppType::ThirdParty,
            client_id: uuid::Uuid::new_v4(),
            redirect_uris: vec![],
            scopes: vec!["write".into()],
            grant_types: vec!["client_credentials".into()],
            manifest_ref: None,
            auto_created: false,
            managed_by_manifest: false,
            is_active: true,
            can_edit: true,
            can_rotate_secret: false,
            can_revoke: true,
            active_token_count: 0,
            last_used_at: None,
            created_at: Utc::now(),
        };
        let item1 = oauth_app_list_item_view(app1);
        let item2 = oauth_app_list_item_view(app2);
        let list = vec![item1.clone(), item2.clone()];

        // Filter by search
        let res = filter_oauth_apps(&list, &ColumnFilters::new(), Some("mobile"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].app.name, "Mobile Client");

        // Filter by type
        let mut filters = ColumnFilters::new();
        filters.set("type", FilterValue::Select("ThirdParty".into()));
        let res = filter_oauth_apps(&list, &filters, None);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].app.name, "Third Party ERP");
    }
}
