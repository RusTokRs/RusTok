#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProductAdminPanel {
    pub title: &'static str,
    pub description: &'static str,
    pub widgets: &'static [&'static str],
}

pub fn ai_product_admin_panel() -> AiProductAdminPanel {
    AiProductAdminPanel {
        title: "AI Product",
        description: "Product copy and attribute-generation controls owned by rustok-ai-product.",
        widgets: &["product copy", "attribute suggestions", "review queue"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_exposes_product_owned_widgets() {
        let panel = ai_product_admin_panel();

        assert!(panel.widgets.contains(&"product copy"));
        assert!(panel.widgets.contains(&"attribute suggestions"));
    }
}
