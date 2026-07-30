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

/// Render a non-HTML Tera template string.
///
/// Subjects and plain-text bodies intentionally preserve their original bytes.
pub fn render_tera_string(template: &str, vars: &serde_json::Value) -> Result<String> {
    render_tera(template, vars, false)
}

/// Render an HTML Tera template with variable autoescaping enabled.
///
/// Provider-controlled markup remains intact while untrusted delivery variables
/// cannot break out of text or attribute contexts.
pub fn render_tera_html_string(template: &str, vars: &serde_json::Value) -> Result<String> {
    render_tera(template, vars, true)
}

fn render_tera(template: &str, vars: &serde_json::Value, autoescape: bool) -> Result<String> {
    let ctx = tera::Context::from_serialize(vars)
        .map_err(|e| EmailError::Template(format!("Failed to build Tera context: {e}")))?;
    tera::Tera::one_off(template, &ctx, autoescape)
        .map_err(|e| EmailError::Template(format!("Tera render error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_rendering_escapes_attribute_breakout_payloads() {
        let rendered = render_tera_html_string(
            r#"<a href="{{ reset_url }}">reset</a>"#,
            &serde_json::json!({
                "reset_url": "https://example.test/reset\" onclick=\"alert(1)"
            }),
        )
        .unwrap();

        assert!(rendered.contains("&quot;"));
        assert!(!rendered.contains("\" onclick=\""));
    }

    #[test]
    fn plain_text_rendering_does_not_html_escape_urls() {
        let rendered = render_tera_string(
            "{{ reset_url }}",
            &serde_json::json!({ "reset_url": "https://example.test/?a=1&b=2" }),
        )
        .unwrap();

        assert_eq!(rendered, "https://example.test/?a=1&b=2");
    }
}
