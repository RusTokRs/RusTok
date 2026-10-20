use crate::error::{EmailError, Result};

/// A rendered, ready-to-send email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedEmail {
    pub subject: String,
    pub text: String,
    pub html: String,
}

/// Contract for providing email templates.
///
/// Modules that need to send transactional emails (order confirmations,
/// forum notifications, etc.) implement this trait and register their
/// templates with the server on startup.
///
/// # Template IDs
/// Convention: `{module_slug}/{action}`, e.g. `commerce/order_confirmed`,
/// `forum/new_reply`.
///
/// For core auth templates the IDs are:
/// - `auth/password_reset`
/// - `auth/email_verification` (future)
/// - `auth/invite` (future)
pub trait EmailTemplateProvider: Send + Sync {
    /// A unique prefix for all template IDs provided by this provider.
    /// Typically the module slug, e.g. `"commerce"`, `"forum"`, `"auth"`.
    fn namespace(&self) -> &str;

    /// Render a template identified by `template_id` for the given `locale`.
    ///
    /// `vars` is a JSON object whose keys depend on the template.
    /// Returns `None` if this provider doesn't handle the given `template_id`.
    fn render(
        &self,
        template_id: &str,
        locale: &str,
        vars: &serde_json::Value,
    ) -> Option<Result<RenderedEmail>>;
}

/// Render a Tera template string, returning the rendered output.
pub fn render_tera_string(template: &str, vars: &serde_json::Value) -> Result<String> {
    let ctx = if vars.is_null() {
        tera::Context::new()
    } else if vars.is_object() {
        tera::Context::from_serialize(vars)
            .map_err(|e| EmailError::Template(format!("Failed to build Tera context: {e}")))?
    } else {
        return Err(EmailError::Template(
            "Tera template context vars must be a JSON object or null".to_string(),
        ));
    };
    tera::Tera::one_off(template, &ctx, /*autoescape=*/ false)
        .map_err(|e| EmailError::Template(format!("Tera render error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_render_tera_string_with_object() {
        let template = "Hello {{ name }}!";
        let vars = json!({ "name": "Alice" });
        let res = render_tera_string(template, &vars).unwrap();
        assert_eq!(res, "Hello Alice!");
    }

    #[test]
    fn test_render_tera_string_with_null() {
        let template = "Hello Static World!";
        let res = render_tera_string(template, &serde_json::Value::Null).unwrap();
        assert_eq!(res, "Hello Static World!");
    }

    #[test]
    fn test_render_tera_string_invalid_vars() {
        let template = "Hello {{ name }}!";
        let vars = json!(["not", "an", "object"]);
        let err = render_tera_string(template, &vars).unwrap_err();
        assert!(matches!(err, EmailError::Template(_)));
    }
}
