pub(super) const SITEMAP_SUBMIT_MAX_ERROR_LEN: usize = 4000;
const SITEMAP_SUBMIT_MAX_FAILURE_DETAILS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SitemapSubmissionSummary {
    pub(super) success_count: usize,
    pub(super) failure_count: usize,
    pub(super) failures: Vec<String>,
    pub(super) omitted_failure_count: usize,
}

impl SitemapSubmissionSummary {
    pub(super) fn into_error(self) -> Option<String> {
        if self.failure_count == 0 {
            return None;
        }
        let mut parts = vec![format!(
            "sitemap submission finished with {} success(es) and {} failure(s)",
            self.success_count, self.failure_count
        )];
        parts.extend(self.failures);
        if self.omitted_failure_count > 0 {
            parts.push(format!(
                "... and {} more failure(s) omitted",
                self.omitted_failure_count
            ));
        }
        let mut message = parts.join("; ");
        if message.len() > SITEMAP_SUBMIT_MAX_ERROR_LEN {
            message.truncate(SITEMAP_SUBMIT_MAX_ERROR_LEN);
            message.push_str("...");
        }
        Some(message)
    }
}

pub(super) fn push_submission_failure(summary: &mut SitemapSubmissionSummary, message: String) {
    if summary.failures.len() < SITEMAP_SUBMIT_MAX_FAILURE_DETAILS {
        summary.failures.push(message);
    } else {
        summary.omitted_failure_count += 1;
    }
}
