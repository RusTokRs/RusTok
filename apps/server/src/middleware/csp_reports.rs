use axum::{
    body::to_bytes,
    extract::Request,
    http::{Method, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::Value;

pub(crate) const CSP_REPORT_PATH: &str = "/api/security/csp-report";
const MAX_CSP_REPORT_BYTES: usize = 64 * 1024;
const MAX_REPORTS_PER_REQUEST: usize = 20;
const MAX_LOGGED_ORIGIN_BYTES: usize = 512;

#[derive(Debug, Deserialize)]
struct CspViolation {
    #[serde(default, alias = "document-uri", alias = "documentURL")]
    document_uri: Option<String>,
    #[serde(default, alias = "blocked-uri", alias = "blockedURL")]
    blocked_uri: Option<String>,
    #[serde(
        default,
        alias = "effective-directive",
        alias = "effectiveDirective"
    )]
    effective_directive: Option<String>,
    #[serde(default, alias = "violated-directive")]
    violated_directive: Option<String>,
    #[serde(default, alias = "source-file", alias = "sourceFile")]
    source_file: Option<String>,
    #[serde(default, alias = "line-number", alias = "lineNumber")]
    line_number: Option<u64>,
    #[serde(default, alias = "column-number", alias = "columnNumber")]
    column_number: Option<u64>,
    #[serde(default, alias = "status-code", alias = "statusCode")]
    status_code: Option<u16>,
    #[serde(default)]
    disposition: Option<String>,
}

#[derive(Debug)]
struct ParsedCspReport {
    format: &'static str,
    violation: CspViolation,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CspReportParseError {
    InvalidJson,
    UnsupportedShape,
}

pub(crate) fn is_report_request(request: &Request) -> bool {
    request.method() == Method::POST && request.uri().path() == CSP_REPORT_PATH
}

pub(crate) async fn handle(request: Request) -> Response {
    if !content_type_is_supported(&request) {
        record_invalid_report("unsupported_content_type");
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }

    let body = match to_bytes(request.into_body(), MAX_CSP_REPORT_BYTES).await {
        Ok(body) => body,
        Err(_error) => {
            record_invalid_report("payload_too_large");
            tracing::warn!(
                target: "rustok.security.csp",
                error_code = "csp_report_body_unreadable_or_oversized",
                max_bytes = MAX_CSP_REPORT_BYTES,
                "Rejected oversized or unreadable CSP report body"
            );
            return StatusCode::PAYLOAD_TOO_LARGE.into_response();
        }
    };

    let reports = match parse_reports(&body) {
        Ok(reports) => reports,
        Err(CspReportParseError::InvalidJson) => {
            record_invalid_report("invalid_json");
            return StatusCode::BAD_REQUEST.into_response();
        }
        Err(CspReportParseError::UnsupportedShape) => {
            record_invalid_report("unsupported_shape");
            return StatusCode::UNPROCESSABLE_ENTITY.into_response();
        }
    };

    for report in reports {
        record_report(report);
    }

    StatusCode::NO_CONTENT.into_response()
}

fn content_type_is_supported(request: &Request) -> bool {
    let mut values = request.headers().get_all(CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return true;
    };
    // Content-Type is a singleton field, not a list. Selecting the first
    // value would accept ambiguous or contradictory request metadata.
    if values.next().is_some() {
        return false;
    }
    let Ok(value) = value.to_str() else {
        return false;
    };
    let media_type = value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    matches!(
        media_type.as_str(),
        "application/csp-report" | "application/reports+json" | "application/json"
    )
}

fn parse_reports(body: &[u8]) -> Result<Vec<ParsedCspReport>, CspReportParseError> {
    let value: Value =
        serde_json::from_slice(body).map_err(|_| CspReportParseError::InvalidJson)?;

    if let Some(report) = value.get("csp-report") {
        let violation = parse_violation(report)?;
        return Ok(vec![ParsedCspReport {
            format: "legacy",
            violation,
        }]);
    }

    let Some(entries) = value.as_array() else {
        return Err(CspReportParseError::UnsupportedShape);
    };

    let mut reports = Vec::new();
    // The reporting API may mix deprecation, intervention, and CSP entries.
    // Apply the bounded budget to CSP reports, not to unrelated entries at
    // the front of the batch, which could otherwise hide every CSP report.
    for entry in entries
        .iter()
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("csp-violation"))
        .take(MAX_REPORTS_PER_REQUEST)
    {
        let body = entry
            .get("body")
            .ok_or(CspReportParseError::UnsupportedShape)?;
        let violation = parse_violation(body)?;
        reports.push(ParsedCspReport {
            format: "reporting_api",
            violation,
        });
    }

    if reports.is_empty() {
        return Err(CspReportParseError::UnsupportedShape);
    }

    Ok(reports)
}

fn parse_violation(value: &Value) -> Result<CspViolation, CspReportParseError> {
    // Serde structs also accept positional sequences. Browser reports are
    // objects, and a payload without a directive is not evidence of a violation.
    if !value.is_object() {
        return Err(CspReportParseError::UnsupportedShape);
    }
    let violation: CspViolation =
        serde_json::from_value(value.clone()).map_err(|_| CspReportParseError::UnsupportedShape)?;
    if selected_directive(&violation).is_none() {
        return Err(CspReportParseError::UnsupportedShape);
    }
    Ok(violation)
}

fn record_report(report: ParsedCspReport) {
    let directive = report_directive(&report.violation);
    rustok_telemetry::metrics::record_module_error("security", directive, "warning");

    tracing::warn!(
        target: "rustok.security.csp",
        report_format = report.format,
        directive,
        disposition = normalized_disposition(report.violation.disposition.as_deref()),
        document_origin = ?sanitized_location(report.violation.document_uri.as_deref()),
        blocked_origin = ?sanitized_location(report.violation.blocked_uri.as_deref()),
        source_origin = ?sanitized_location(report.violation.source_file.as_deref()),
        line = report.violation.line_number,
        column = report.violation.column_number,
        status = report.violation.status_code,
        "CSP report-only policy violation"
    );
}

fn record_invalid_report(reason: &'static str) {
    rustok_telemetry::metrics::record_module_error("security", reason, "warning");
    tracing::warn!(target: "rustok.security.csp", reason, "Rejected CSP report payload");
}

fn normalized_disposition(value: Option<&str>) -> &'static str {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "enforce" => "enforce",
        "report" => "report",
        _ => "other",
    }
}

fn selected_directive(violation: &CspViolation) -> Option<&str> {
    // Legacy browser reports can carry both fields. Prefer the actual check.
    violation
        .effective_directive
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            violation
                .violated_directive
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
}

fn report_directive(violation: &CspViolation) -> &'static str {
    normalized_directive(selected_directive(violation))
}

fn normalized_directive(value: Option<&str>) -> &'static str {
    // Older violated-directive values may include the directive source list.
    let value = value
        .unwrap_or_default()
        .split_ascii_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(value.as_str(), "script-src" | "script-src-elem" | "script-src-attr") {
        "script-src"
    } else if matches!(value.as_str(), "style-src" | "style-src-elem" | "style-src-attr") {
        "style-src"
    } else {
        match value.as_str() {
            "connect-src" => "connect-src",
            "img-src" => "img-src",
            "font-src" => "font-src",
            "worker-src" => "worker-src",
            "frame-src" => "frame-src",
            "frame-ancestors" => "frame-ancestors",
            "object-src" => "object-src",
            "base-uri" => "base-uri",
            "form-action" => "form-action",
            "default-src" => "default-src",
            _ => "other",
        }
    }
}

fn sanitized_location(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    if matches!(value, "inline" | "eval" | "self" | "data" | "blob") {
        return Some(value.to_string());
    }

    if let Ok(parsed) = url::Url::parse(value) {
        let origin = parsed.origin().ascii_serialization();
        let location = if origin == "null" {
            let scheme = parsed.scheme();
            if scheme.len() > MAX_LOGGED_ORIGIN_BYTES {
                return Some("oversized".to_string());
            }
            // An opaque scheme is itself untrusted text, not a browser origin.
            // Retain only known scheme labels; arbitrary schemes may contain
            // private identifiers and must not be copied to telemetry.
            match scheme {
                "about" | "blob" | "data" | "file" | "filesystem" | "javascript"
                | "mailto" | "chrome" | "chrome-extension" | "moz-extension"
                | "resource" => scheme.to_string(),
                _ => return Some("opaque".to_string()),
            }
        } else {
            origin
        };
        if location.len() > MAX_LOGGED_ORIGIN_BYTES {
            return Some("oversized".to_string());
        }
        return Some(location);
    }

    Some("opaque".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_duplicate_content_type_fields_before_collecting_reports() {
        use axum::{body::Body, http::HeaderValue};

        for second in ["application/csp-report", "text/plain"] {
            let mut request = axum::http::Request::builder()
                .method(Method::POST)
                .uri(CSP_REPORT_PATH)
                .header(CONTENT_TYPE, "application/csp-report")
                .body(Body::from(
                    r#"{"csp-report":{"effective-directive":"script-src"}}"#,
                ))
                .expect("report request");
            request
                .headers_mut()
                .append(CONTENT_TYPE, HeaderValue::from_static(second));

            assert_eq!(
                handle(request).await.status(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE
            );
        }
    }

    #[tokio::test]
    async fn accepts_single_or_missing_content_type_for_valid_report() {
        use axum::body::Body;

        for content_type in [
            None,
            Some("application/csp-report"),
            Some("application/json; charset=utf-8"),
        ] {
            let mut request = axum::http::Request::builder().method(Method::POST).uri(CSP_REPORT_PATH);
            if let Some(content_type) = content_type {
                request = request.header(CONTENT_TYPE, content_type);
            }
            let request = request
                .body(Body::from(
                    r#"{"csp-report":{"effective-directive":"script-src"}}"#,
                ))
                .expect("report request");
            assert_eq!(handle(request).await.status(), StatusCode::NO_CONTENT);
        }
    }

    #[test]
    fn parses_legacy_csp_report_without_script_sample() {
        let reports = parse_reports(
            br#"{"csp-report":{"document-uri":"https://admin.example.com/orders?token=secret","blocked-uri":"inline","violated-directive":"script-src-elem","script-sample":"secret()"}}"#,
        )
        .expect("legacy report");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].format, "legacy");
        assert_eq!(
            report_directive(&reports[0].violation),
            "script-src"
        );
        assert_eq!(
            sanitized_location(reports[0].violation.document_uri.as_deref()).as_deref(),
            Some("https://admin.example.com")
        );
    }

    #[test]
    fn legacy_browser_reports_accept_both_directives_and_prefer_effective() {
        let reports = parse_reports(
            br#"{"csp-report":{"violated-directive":"default-src","effective-directive":"script-src-elem"}}"#,
        )
        .expect("browser CSP report with both fields");
        assert_eq!(reports.len(), 1);
        assert_eq!(report_directive(&reports[0].violation), "script-src");
    }

    #[test]
    fn blank_effective_directive_falls_back_to_violated_directive() {
        let reports = parse_reports(
            br#"{"csp-report":{"effective-directive":" ","violated-directive":"img-src"}}"#,
        )
        .expect("legacy report");
        assert_eq!(report_directive(&reports[0].violation), "img-src");
    }

    #[test]
    fn parses_reporting_api_batch_and_ignores_other_report_types() {
        let reports = parse_reports(
            br#"[{"type":"deprecation","body":{}},{"type":"csp-violation","body":{"documentURL":"https://shop.example.com/cart","blockedURL":"https://cdn.example.net/app.js?key=secret","effectiveDirective":"connect-src","statusCode":200}}]"#,
        )
        .expect("reporting API batch");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].format, "reporting_api");
        assert_eq!(report_directive(&reports[0].violation), "connect-src");
        assert_eq!(
            sanitized_location(reports[0].violation.blocked_uri.as_deref()).as_deref(),
            Some("https://cdn.example.net")
        );
    }

    #[test]
    fn reporting_api_budget_counts_csp_reports_after_unrelated_entries() {
        let mut entries = (0..MAX_REPORTS_PER_REQUEST + 5)
            .map(|_| serde_json::json!({"type": "deprecation", "body": {}}))
            .collect::<Vec<_>>();
        entries.extend((0..MAX_REPORTS_PER_REQUEST + 3).map(|_| {
            serde_json::json!({
                "type": "csp-violation",
                "body": {"effectiveDirective": "script-src"}
            })
        }));
        let body = serde_json::to_vec(&entries).expect("reporting API JSON");
        let reports = parse_reports(&body).expect("CSP reports after unrelated entries");
        assert_eq!(reports.len(), MAX_REPORTS_PER_REQUEST);
        assert!(reports.iter().all(|report| report.format == "reporting_api"));
    }

    #[test]
    fn report_dispositions_are_bounded() {
        assert_eq!(normalized_disposition(None), "other");
        assert_eq!(normalized_disposition(Some("report")), "report");
        assert_eq!(normalized_disposition(Some("ENFORCE")), "enforce");
        assert_eq!(normalized_disposition(Some("attacker-controlled")), "other");
    }

    #[test]
    fn oversized_logged_origins_are_bounded() {
        let host = "a".repeat(MAX_LOGGED_ORIGIN_BYTES + 64);
        let url = format!("https://{host}/private?token=secret");
        assert_eq!(
            sanitized_location(Some(url.as_str())).as_deref(),
            Some("oversized")
        );
    }

    #[test]
    fn oversized_opaque_url_schemes_are_bounded() {
        let scheme = "a".repeat(MAX_LOGGED_ORIGIN_BYTES + 1);
        let url = format!("{scheme}:private");
        assert_eq!(sanitized_location(Some(&url)).as_deref(), Some("oversized"));
    }

    #[test]
    fn opaque_url_scheme_telemetry_retains_only_known_labels() {
        for (value, expected) in [
            ("data:text/plain,private", "data"),
            ("file:///private/path", "file"),
            ("about:blank", "about"),
            ("chrome-extension://private-id/path", "chrome-extension"),
            ("private-identifier:payload", "opaque"),
            ("unknown+scheme://private-host/path", "opaque"),
        ] {
            assert_eq!(sanitized_location(Some(value)).as_deref(), Some(expected));
        }
        assert_eq!(
            sanitized_location(Some("https://user:password@example.com/path?secret=1#private")).as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn directive_labels_are_bounded() {
        assert_eq!(normalized_directive(Some("script-src-attr")), "script-src");
        assert_eq!(normalized_directive(Some("style-src-elem")), "style-src");
        assert_eq!(normalized_directive(Some("unknown-directive")), "other");
        assert_eq!(normalized_directive(Some("script-src-unrelated")), "other");
        assert_eq!(normalized_directive(Some("script-src 'self'")), "script-src");
        assert_eq!(normalized_directive(Some("style-src-unrelated")), "other");
    }

    #[test]
    fn location_telemetry_drops_paths_queries_and_opaque_values() {
        assert_eq!(
            sanitized_location(Some("https://admin.example.com/orders?token=secret")).as_deref(),
            Some("https://admin.example.com")
        );
        assert_eq!(
            sanitized_location(Some("/private/path?token=secret")).as_deref(),
            Some("opaque")
        );
    }

    #[test]
    fn rejects_violation_bodies_without_object_shape_or_nonblank_directive() {
        for body in [
            r#"{"csp-report":{}}"#,
            r#"{"csp-report":{"effective-directive":" ","violated-directive":""}}"#,
            r#"{"csp-report":[null,null,"script-src",null,null,null,null,null,null]}"#,
            r#"[{"type":"csp-violation","body":{}}]"#,
            r#"[{"type":"csp-violation","body":[null,null,"script-src",null,null,null,null,null,null]}]"#,
        ] {
            assert_eq!(
                parse_reports(body.as_bytes()).expect_err("malformed violation body"),
                CspReportParseError::UnsupportedShape
            );
        }
    }

    #[test]
    fn accepts_unknown_nonblank_directives_without_misclassifying_them() {
        let reports = parse_reports(
            br#"{"csp-report":{"effective-directive":"script-src-future"}}"#,
        )
        .expect("unknown directive remains observable");
        assert_eq!(report_directive(&reports[0].violation), "other");
    }

    #[test]
    fn rejects_non_csp_json_shapes() {
        assert_eq!(
            parse_reports(br#"{"event":"not-csp"}"#).expect_err("unsupported"),
            CspReportParseError::UnsupportedShape
        );
    }
}
