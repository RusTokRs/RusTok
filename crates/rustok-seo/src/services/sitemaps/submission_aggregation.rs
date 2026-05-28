pub(super) const SITEMAP_SUBMIT_MAX_ERROR_LEN: usize = 4000;
pub(super) const SITEMAP_SUBMIT_MAX_ERRORS: usize = 8;
pub(super) const SITEMAP_SUBMIT_MAX_TIMEOUT_DETAILS: usize = 4;

const SITEMAP_SUBMIT_MAX_ERROR_DETAIL_LEN: usize = 512;
const SITEMAP_SUBMIT_MAX_ENDPOINT_STATUS_COUNT: usize = 24;
const SITEMAP_SUBMIT_MAX_ENDPOINT_VALUE_LEN: usize = 160;
const SITEMAP_SUBMIT_MAX_ENDPOINT_STATUS_SAMPLE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SitemapSubmissionEndpointState {
    Success,
    Failure,
    Timeout,
    InvalidEndpoint,
}

impl SitemapSubmissionEndpointState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Timeout => "timeout",
            Self::InvalidEndpoint => "invalid_endpoint",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SitemapSubmissionEndpointStatus {
    pub(super) endpoint: String,
    pub(super) state: SitemapSubmissionEndpointState,
    pub(super) detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct SitemapSubmissionTelemetrySnapshot {
    pub(super) endpoint_statuses: Vec<SitemapSubmissionEndpointStatus>,
    pub(super) omitted_endpoint_status_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct SitemapSubmissionSummary {
    pub(super) success_count: usize,
    pub(super) failure_count: usize,
    endpoint_results: Vec<SitemapSubmissionEndpointStatus>,
}

impl SitemapSubmissionSummary {
    pub(super) fn into_error(self) -> Option<String> {
        if self.failure_count == 0 {
            return None;
        }

        let mut errors = Vec::new();
        let mut timeout_details = Vec::new();
        let mut omitted_errors = 0_usize;
        let mut omitted_timeout_details = 0_usize;

        for status in sorted_endpoint_results(self.endpoint_results.as_slice()) {
            let Some(detail) = status.detail else {
                continue;
            };
            match status.state {
                SitemapSubmissionEndpointState::Timeout => {
                    if timeout_details.len() < SITEMAP_SUBMIT_MAX_TIMEOUT_DETAILS {
                        timeout_details.push(detail);
                    } else {
                        omitted_timeout_details += 1;
                    }
                }
                SitemapSubmissionEndpointState::Failure
                | SitemapSubmissionEndpointState::InvalidEndpoint => {
                    if errors.len() < SITEMAP_SUBMIT_MAX_ERRORS {
                        errors.push(detail);
                    } else {
                        omitted_errors += 1;
                    }
                }
                SitemapSubmissionEndpointState::Success => {}
            }
        }

        let telemetry = self.telemetry_snapshot();
        let mut parts = vec![format!(
            "sitemap submission finished with {} success(es) and {} failure(s)",
            self.success_count, self.failure_count
        )];

        if !errors.is_empty() {
            parts.push(format!("errors: [{}]", errors.join(" | ")));
        }
        if omitted_errors > 0 {
            parts.push(format!("errors omitted: {omitted_errors}"));
        }
        if !timeout_details.is_empty() {
            parts.push(format!("timeouts: [{}]", timeout_details.join(" | ")));
        }
        if omitted_timeout_details > 0 {
            parts.push(format!(
                "timeout details omitted: {omitted_timeout_details}"
            ));
        }

        if !telemetry.endpoint_statuses.is_empty() {
            let statuses = telemetry
                .endpoint_statuses
                .iter()
                .take(SITEMAP_SUBMIT_MAX_ENDPOINT_STATUS_SAMPLE)
                .map(|status| {
                    format!(
                        "{}:{}",
                        status.endpoint,
                        status.state.as_str(),
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("endpoint statuses: [{statuses}]"));
        }
        if telemetry.omitted_endpoint_status_count > 0 {
            parts.push(format!(
                "endpoint statuses omitted: {}",
                telemetry.omitted_endpoint_status_count
            ));
        }

        let message = parts.join("; ");
        Some(truncate_with_ellipsis(message, SITEMAP_SUBMIT_MAX_ERROR_LEN))
    }

    pub(super) fn telemetry_snapshot(&self) -> SitemapSubmissionTelemetrySnapshot {
        let mut statuses = sorted_endpoint_results(self.endpoint_results.as_slice());
        let omitted_endpoint_status_count =
            statuses.len().saturating_sub(SITEMAP_SUBMIT_MAX_ENDPOINT_STATUS_COUNT);
        statuses.truncate(SITEMAP_SUBMIT_MAX_ENDPOINT_STATUS_COUNT);

        SitemapSubmissionTelemetrySnapshot {
            endpoint_statuses: statuses,
            omitted_endpoint_status_count,
        }
    }
}

pub(super) fn record_submission_success(summary: &mut SitemapSubmissionSummary, endpoint: &str) {
    summary.success_count += 1;
    summary.endpoint_results.push(SitemapSubmissionEndpointStatus {
        endpoint: truncate_endpoint(endpoint),
        state: SitemapSubmissionEndpointState::Success,
        detail: None,
    });
}

pub(super) fn record_invalid_endpoint(summary: &mut SitemapSubmissionSummary, endpoint: &str) {
    summary.failure_count += 1;
    summary.endpoint_results.push(SitemapSubmissionEndpointStatus {
        endpoint: truncate_endpoint(endpoint),
        state: SitemapSubmissionEndpointState::InvalidEndpoint,
        detail: Some(truncate_error_detail(format!(
            "invalid endpoint: {}",
            endpoint.trim()
        ))),
    });
}

pub(super) fn record_submission_failure(
    summary: &mut SitemapSubmissionSummary,
    endpoint: &str,
    detail: String,
) {
    summary.failure_count += 1;
    let state = if is_timeout_error(detail.as_str()) {
        SitemapSubmissionEndpointState::Timeout
    } else {
        SitemapSubmissionEndpointState::Failure
    };

    summary.endpoint_results.push(SitemapSubmissionEndpointStatus {
        endpoint: truncate_endpoint(endpoint),
        state,
        detail: Some(truncate_error_detail(detail)),
    });
}

fn sorted_endpoint_results(
    values: &[SitemapSubmissionEndpointStatus],
) -> Vec<SitemapSubmissionEndpointStatus> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| {
        left.endpoint
            .cmp(&right.endpoint)
            .then(left.state.cmp(&right.state))
            .then(left.detail.cmp(&right.detail))
    });
    sorted
}

fn truncate_endpoint(value: &str) -> String {
    truncate_with_ellipsis(value.trim().to_string(), SITEMAP_SUBMIT_MAX_ENDPOINT_VALUE_LEN)
}

fn truncate_error_detail(value: String) -> String {
    truncate_with_ellipsis(value, SITEMAP_SUBMIT_MAX_ERROR_DETAIL_LEN)
}

fn is_timeout_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("timeout")
        || normalized.contains("timed out")
        || normalized.contains("deadline exceeded")
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
