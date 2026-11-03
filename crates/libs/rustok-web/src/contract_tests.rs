#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Role",
        "## Public modules",
        "## Primary public types and functions",
        "## Contract invariants",
        "## Errors",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn http_error_constructors_map_to_canonical_status_codes() {
    use crate::HttpError;
    use axum::http::StatusCode;

    let bad_req = HttpError::bad_request("bad", "bad req");
    assert_eq!(bad_req.status, StatusCode::BAD_REQUEST);

    let unauth = HttpError::unauthorized("unauth", "unauthorized");
    assert_eq!(unauth.status, StatusCode::UNAUTHORIZED);

    let forb = HttpError::forbidden("forbidden", "forbidden access");
    assert_eq!(forb.status, StatusCode::FORBIDDEN);

    let not_found = HttpError::not_found("not_found", "item not found");
    assert_eq!(not_found.status, StatusCode::NOT_FOUND);

    let conflict = HttpError::conflict("conflict", "item conflict");
    assert_eq!(conflict.status, StatusCode::CONFLICT);

    let unavail = HttpError::service_unavailable("unavail", "service down");
    assert_eq!(unavail.status, StatusCode::SERVICE_UNAVAILABLE);

    let timeout = HttpError::gateway_timeout("timeout", "gateway timeout");
    assert_eq!(timeout.status, StatusCode::GATEWAY_TIMEOUT);

    let internal = HttpError::internal("internal boom");
    assert_eq!(internal.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(internal.code, "internal_error");
}

#[test]
fn json_response_creates_valid_ok_response() {
    use crate::json_response;
    use axum::http::StatusCode;

    let resp = json_response(serde_json::json!({"status": "ok"}));
    assert_eq!(resp.status(), StatusCode::OK);
}
