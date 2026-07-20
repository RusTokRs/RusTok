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

#[derive(Debug, Deserialize)]
struct CspViolation {
    #[serde(default, alias = "document-uri", alias = "documentURL")]
    document_uri: Option<String>,
    #[serde(default, alias = "blocked-uri", alias = "blockedURL")]
    blocked_uri: Option<String>,
    #[serde(
        default,
        alias = "effective-directive",
        alias = "effectiveDirective",
        alias = "violated-directive"
    )]
    effective_directive: Option<String>,
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
        Err(error) => {
            record_invalid_report("payload_too_large");
            tracing::warn!(
                target: "rustok.security.csp",
                %error,
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
    let Some(value) = request.headers().get(CONTENT_TYPE) else {
        return true;
    };
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
        let violation = serde_json::from_value(report.clone())
            .map_err(|_| CspReportParseError::UnsupportedShape)?;
        return Ok(vec![ParsedCspReport {
            format: "legacy",
            violation,
        }]);
    }

    let Some(entries) = value.as_array() else {
        return Err(CspReportParseError::UnsupportedShape);
    };

    let mut reports = Vec::new();
    for entry in entries.iter().take(MAX_REPORTS_PER_REQUEST) {
        if entry.get("type").and_then(Value::as_str) != Some("csp-violation") {
            continue;
        }
        let body = entry
            .get("body")
            .cloned()
            .ok_or(CspReportParseError::UnsupportedShape)?;
        let violation =
            serde_json::from_value(body).map_err(|_| CspReportParseError::UnsupportedShape)?;
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

fn record_report(report: ParsedCspReport) {
    let directive = normalized_directive(report.violation.effective_directive.as_deref());
    rustok_telemetry::metrics::record_module_error("security", directive, "warning");

    tracing::warn!(
        target: "rustok.security.csp",
        report_format = report.format,
        directive,
        disposition = report.violation.disposition.as_deref().unwrap_or("report"),
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

fn normalized_directive(value: Option<&str>) -> &'static str {
    let value = value.unwrap_or_default().trim().to_ascii_lowercase();
    if value.starts_with("script-src") {
        "script-src"
    } else if value.starts_with("style-src") {
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
        return Some(if origin == "null" {
            parsed.scheme().to_string()
        } else {
            origin
        });
    }

    Some("opaque".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_csp_report_without_script_sample() {
        let reports = parse_reports(
            br#"{"csp-report":{"document-uri":"https://admin.example.com/orders?token=secret","blocked-uri":"inline","violated-directive":"script-src-elem","script-sample":"secret()"}}"#,
        )
        .expect("legacy report");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].format, "legacy");
        assert_eq!(
            normalized_directive(reports[0].violation.effective_directive.as_deref()),
            "script-src"
        );
        assert_eq!(
            sanitized_location(reports[0].violation.document_uri.as_deref()).as_deref(),
            Some("https://admin.example.com")
        );
    }

    #[test]
    fn parses_reporting_api_batch_and_ignores_other_report_types() {
        let reports = parse_reports(
            br#"[{"type":"deprecation","body":{}},{"type":"csp-violation","body":{"documentURL":"https://shop.example.com/cart","blockedURL":"https://cdn.example.net/app.js?key=secret","effectiveDirective":"connect-src","statusCode":200}}]"#,
        )
        .expect("reporting API batch");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].format, "reporting_api");
        assert_eq!(
            sanitized_location(reports[0].violation.blocked_uri.as_deref()).as_deref(),
            Some("https://cdn.example.net")
        );
    }

    #[test]
    fn directive_labels_are_bounded() {
        assert_eq!(normalized_directive(Some("script-src-attr")), "script-src");
        assert_eq!(normalized_directive(Some("style-src-elem")), "style-src");
        assert_eq!(normalized_directive(Some("unknown-directive")), "other");
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
    fn rejects_non_csp_json_shapes() {
        assert_eq!(
            parse_reports(br#"{"event":"not-csp"}"#).expect_err("unsupported"),
            CspReportParseError::UnsupportedShape
        );
    }
}
