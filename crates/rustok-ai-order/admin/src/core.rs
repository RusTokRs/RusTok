#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiOrderAdminPanel {
    pub title: &'static str,
    pub description: &'static str,
    pub widgets: &'static [&'static str],
}

pub fn ai_order_admin_panel() -> AiOrderAdminPanel {
    AiOrderAdminPanel {
        title: "AI Order",
        description: "Order analytics and operator-assistant controls owned by rustok-ai-order.",
        widgets: &["order analytics", "ops assistant", "risk summary"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_exposes_order_owned_widgets() {
        let panel = ai_order_admin_panel();

        assert!(panel.widgets.contains(&"order analytics"));
        assert!(panel.widgets.contains(&"ops assistant"));
    }
}
