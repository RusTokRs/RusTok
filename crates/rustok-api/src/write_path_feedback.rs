#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritePathIssueKind {
    Validation,
    Sanitization,
    Runtime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WritePathIssue {
    pub kind: WritePathIssueKind,
    pub message: String,
}

impl WritePathIssue {
    pub fn new(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            kind: classify_write_path_issue(&message),
            message,
        }
    }

    pub fn with_context(context: &str, error: &str) -> Self {
        if context.trim().is_empty() {
            return Self::new(error);
        }

        if error.trim().is_empty() {
            return Self::new(context);
        }

        Self::new(format!("{context}: {error}"))
    }
}

pub fn classify_write_path_issue(raw: &str) -> WritePathIssueKind {
    let normalized = raw.trim().to_ascii_lowercase();

    if normalized.is_empty() {
        return WritePathIssueKind::Runtime;
    }

    if contains_any(
        &normalized,
        &[
            "sanitize",
            "sanitiz",
            "sanitized",
            "sanitization",
            "xss",
            "unsafe html",
        ],
    ) {
        return WritePathIssueKind::Sanitization;
    }

    if contains_any(
        &normalized,
        &[
            "validation",
            "invalid",
            "required",
            "must",
            "cannot",
            "unknown format",
            "unprocessable",
            "bad request",
        ],
    ) {
        return WritePathIssueKind::Validation;
    }

    WritePathIssueKind::Runtime
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{classify_write_path_issue, WritePathIssue, WritePathIssueKind};

    #[test]
    fn classify_marks_validation_errors() {
        assert_eq!(
            classify_write_path_issue("Validation error: content_json is required"),
            WritePathIssueKind::Validation
        );
        assert_eq!(
            classify_write_path_issue("Invalid locale"),
            WritePathIssueKind::Validation
        );
    }

    #[test]
    fn classify_marks_sanitize_errors() {
        assert_eq!(
            classify_write_path_issue("Failed to sanitize html fragment"),
            WritePathIssueKind::Sanitization
        );
        assert_eq!(
            classify_write_path_issue("Sanitization policy rejected payload"),
            WritePathIssueKind::Sanitization
        );
    }

    #[test]
    fn classify_falls_back_to_runtime() {
        assert_eq!(
            classify_write_path_issue("Transport timeout while saving post"),
            WritePathIssueKind::Runtime
        );
    }

    #[test]
    fn write_path_issue_with_context_composes_message() {
        let issue = WritePathIssue::with_context("Failed to save page", "validation error");

        assert_eq!(issue.kind, WritePathIssueKind::Validation);
        assert_eq!(issue.message, "Failed to save page: validation error");
    }
}
