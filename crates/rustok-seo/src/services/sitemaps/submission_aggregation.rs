pub(super) const SITEMAP_SUBMIT_MAX_ERROR_LEN: usize = 4000;
const SITEMAP_SUBMIT_MAX_FAILURE_DETAILS: usize = 8;
const SITEMAP_SUBMIT_MAX_FAILURE_DETAIL_LEN: usize = 512;
const SITEMAP_SUBMIT_MAX_ENDPOINT_STATUSES: usize = 24;
const SITEMAP_SUBMIT_MAX_ENDPOINT_STATUS_LEN: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SitemapSubmissionSummary {
    pub(super) success_count: usize,
    pub(super) failure_count: usize,
    pub(super) failures: Vec<String>,
    pub(super) omitted_failure_count: usize,
    pub(super) endpoint_statuses: Vec<String>,
    pub(super) omitted_endpoint_status_count: usize,
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
        summary.failures.push(truncate_detail(message));
    } else {
        summary.omitted_failure_count += 1;
    }
}

pub(super) fn push_endpoint_status(summary: &mut SitemapSubmissionSummary, status: String) {
    if summary.endpoint_statuses.len() < SITEMAP_SUBMIT_MAX_ENDPOINT_STATUSES {
        summary.endpoint_statuses.push(truncate_endpoint_status(status));
    } else {
        summary.omitted_endpoint_status_count += 1;
    }
}

fn truncate_detail(value: String) -> String {
    truncate_with_ellipsis(value, SITEMAP_SUBMIT_MAX_FAILURE_DETAIL_LEN)
}

fn truncate_endpoint_status(value: String) -> String {
    truncate_with_ellipsis(value, SITEMAP_SUBMIT_MAX_ENDPOINT_STATUS_LEN)
}

fn truncate_with_ellipsis(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut cut = 0usize;
    for (idx, _) in value.char_indices() {
        if idx > max_bytes {
            break;
        }
        cut = idx;
    }
    if cut == 0 {
        value.clear();
    } else {
        value.truncate(cut);
    }
    value.push_str("...");
    value
}
