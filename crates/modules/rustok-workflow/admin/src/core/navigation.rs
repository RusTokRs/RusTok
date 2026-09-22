#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowAdminNavViewModel {
    pub module_href: String,
    pub template_href: String,
    pub toggle_href: String,
}

pub fn workflow_admin_nav_view_model(
    route_segment: Option<&str>,
    showing_templates: bool,
) -> WorkflowAdminNavViewModel {
    let route_segment = route_segment.unwrap_or("workflow").trim_matches('/');
    let route_segment = if route_segment.is_empty() {
        "workflow"
    } else {
        route_segment
    };
    let module_href = format!("/modules/{route_segment}");
    let template_href = format!("{module_href}/templates");
    let toggle_href = if showing_templates {
        module_href.clone()
    } else {
        template_href.clone()
    };

    WorkflowAdminNavViewModel {
        module_href,
        template_href,
        toggle_href,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_admin_nav_view_model_owns_module_routes() {
        let overview = workflow_admin_nav_view_model(Some("workflow"), false);
        assert_eq!(overview.module_href, "/modules/workflow");
        assert_eq!(overview.template_href, "/modules/workflow/templates");
        assert_eq!(overview.toggle_href, "/modules/workflow/templates");
        let templates = workflow_admin_nav_view_model(Some("/workflow/"), true);
        assert_eq!(templates.toggle_href, "/modules/workflow");

        let defaulted = workflow_admin_nav_view_model(Some(""), false);
        assert_eq!(defaulted.module_href, "/modules/workflow");
    }
}
