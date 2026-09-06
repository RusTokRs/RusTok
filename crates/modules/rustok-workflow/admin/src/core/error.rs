#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowErrorViewModel {
    pub message: String,
}

pub fn workflow_error_view_model(
    prefix: &str,
    error: impl std::fmt::Display,
) -> WorkflowErrorViewModel {
    WorkflowErrorViewModel {
        message: format!("{}: {error}", prefix.trim()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_error_view_model_formats_transport_errors_outside_ui() {
        let error = workflow_error_view_model("Failed to load workflows", "network unavailable");
        assert_eq!(
            error.message,
            "Failed to load workflows: network unavailable"
        );
    }
}
