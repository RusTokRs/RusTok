#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiContentAdminPanel {
    pub title: &'static str,
    pub description: &'static str,
    pub widgets: &'static [&'static str],
}

pub fn ai_content_admin_panel() -> AiContentAdminPanel {
    AiContentAdminPanel {
        title: "AI Content",
        description: "Moderation and generated-content review controls owned by rustok-ai-content.",
        widgets: &["moderation queue", "blog draft review", "approval routing"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_exposes_content_owned_widgets() {
        let panel = ai_content_admin_panel();

        assert!(panel.widgets.contains(&"moderation queue"));
        assert!(panel.widgets.contains(&"blog draft review"));
    }
}
