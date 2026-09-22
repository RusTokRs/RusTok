use crate::error::{EmailError, Result};

/// A rendered, ready-to-send email.
#[derive(Debug, Clone)]
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
    let ctx = tera::Context::from_serialize(vars)
        .map_err(|e| EmailError::Template(format!("Failed to build Tera context: {e}")))?;
    tera::Tera::one_off(template, &ctx, /*autoescape=*/ false)
        .map_err(|e| EmailError::Template(format!("Tera render error: {e}")))
}
