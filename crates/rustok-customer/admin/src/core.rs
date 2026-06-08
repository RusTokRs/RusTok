use crate::model::CustomerDraft;

pub const DEFAULT_CUSTOMER_PAGE: u64 = 1;
pub const DEFAULT_CUSTOMER_PER_PAGE: u64 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerListRequest {
    pub search: String,
    pub page: u64,
    pub per_page: u64,
}

pub fn customer_list_request(search: impl Into<String>) -> CustomerListRequest {
    CustomerListRequest {
        search: search.into(),
        page: DEFAULT_CUSTOMER_PAGE,
        per_page: DEFAULT_CUSTOMER_PER_PAGE,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAdminDraftInput {
    pub editing_customer_id: Option<String>,
    pub user_id: String,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAdminSubmitCommand {
    pub customer_id: Option<String>,
    pub draft: CustomerDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomerAdminSubmitCommandError {
    EmailRequired,
    LocaleUnavailable,
}

pub fn build_customer_admin_submit_command(
    input: CustomerAdminDraftInput,
    effective_locale: Option<String>,
) -> Result<CustomerAdminSubmitCommand, CustomerAdminSubmitCommandError> {
    if input.email.trim().is_empty() {
        return Err(CustomerAdminSubmitCommandError::EmailRequired);
    }

    let Some(locale) = effective_locale else {
        return Err(CustomerAdminSubmitCommandError::LocaleUnavailable);
    };

    Ok(CustomerAdminSubmitCommand {
        customer_id: input.editing_customer_id,
        draft: CustomerDraft {
            user_id: input.user_id,
            email: input.email,
            first_name: input.first_name,
            last_name: input.last_name,
            phone: input.phone,
            locale,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customer_list_request_uses_admin_defaults() {
        let request = customer_list_request("alice");

        assert_eq!(request.search, "alice");
        assert_eq!(request.page, DEFAULT_CUSTOMER_PAGE);
        assert_eq!(request.per_page, DEFAULT_CUSTOMER_PER_PAGE);
    }

    #[test]
    fn submit_command_preserves_payload_and_update_target() {
        let command = build_customer_admin_submit_command(
            CustomerAdminDraftInput {
                editing_customer_id: Some("customer-1".to_string()),
                user_id: "user-1".to_string(),
                email: "alice@example.com".to_string(),
                first_name: "Alice".to_string(),
                last_name: "Example".to_string(),
                phone: "+10000000000".to_string(),
            },
            Some("en".to_string()),
        )
        .expect("valid command");

        assert_eq!(command.customer_id.as_deref(), Some("customer-1"));
        assert_eq!(command.draft.user_id, "user-1");
        assert_eq!(command.draft.email, "alice@example.com");
        assert_eq!(command.draft.first_name, "Alice");
        assert_eq!(command.draft.last_name, "Example");
        assert_eq!(command.draft.phone, "+10000000000");
        assert_eq!(command.draft.locale, "en");
    }

    #[test]
    fn submit_command_rejects_missing_email_before_locale() {
        let error = build_customer_admin_submit_command(
            CustomerAdminDraftInput {
                editing_customer_id: None,
                user_id: String::new(),
                email: "   ".to_string(),
                first_name: String::new(),
                last_name: String::new(),
                phone: String::new(),
            },
            None,
        )
        .expect_err("email policy should run first");

        assert_eq!(error, CustomerAdminSubmitCommandError::EmailRequired);
    }

    #[test]
    fn submit_command_requires_host_locale() {
        let error = build_customer_admin_submit_command(
            CustomerAdminDraftInput {
                editing_customer_id: None,
                user_id: String::new(),
                email: "alice@example.com".to_string(),
                first_name: String::new(),
                last_name: String::new(),
                phone: String::new(),
            },
            None,
        )
        .expect_err("host locale is required");

        assert_eq!(error, CustomerAdminSubmitCommandError::LocaleUnavailable);
    }
}
