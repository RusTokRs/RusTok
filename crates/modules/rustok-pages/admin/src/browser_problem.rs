use crate::browser_intent::PagesBrowserIntentError;
use crate::contribution_browser_intent::PagesBrowserIntentAccessError;
use fly_ui::EditorCapability;
use rustok_page_builder_admin::{
    BROWSER_CAPABILITY_DENIAL_CODE, BrowserCapabilityAccessError, BrowserIntentDispatchError,
    PageBuilderAdminFacadeError, SsrDraftSessionError,
};
use serde::{Deserialize, Serialize};

const HTTP_BAD_REQUEST: u16 = 400;
const HTTP_FORBIDDEN: u16 = 403;
const HTTP_NOT_FOUND: u16 = 404;
const HTTP_CONFLICT: u16 = 409;
const HTTP_PAYLOAD_TOO_LARGE: u16 = 413;
const HTTP_UNPROCESSABLE_ENTITY: u16 = 422;
const HTTP_BAD_GATEWAY: u16 = 502;
const HTTP_SERVICE_UNAVAILABLE: u16 = 503;

pub const BROWSER_REVISION_CONFLICT_CODE: &str = "REVISION_CONFLICT";
pub const BROWSER_PROJECT_HASH_CONFLICT_CODE: &str = "PROJECT_HASH_CONFLICT";
pub const BROWSER_DRAFT_GENERATION_CONFLICT_CODE: &str = "DRAFT_GENERATION_CONFLICT";
/// Code of an intent for a page whose authoring document is withheld from the principal.
pub const BROWSER_DOCUMENT_UNAVAILABLE_CODE: &str = "DOCUMENT_UNAVAILABLE";
/// Code of a rejected save that carries no more specific stable code.
pub const BROWSER_SAVE_FAILED_CODE: &str = "SAVE_FAILED";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PagesBrowserIntentProblem {
    pub status: u16,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// Backward-compatible primary missing capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<EditorCapability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<EditorCapability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<EditorCapability>,
}

impl PagesBrowserIntentProblem {
    pub fn from_error(error: &PagesBrowserIntentAccessError) -> Self {
        if let Some(denial) = error.capability_denial() {
            return Self {
                status: HTTP_FORBIDDEN,
                error: error.to_string(),
                code: Some(BROWSER_CAPABILITY_DENIAL_CODE.to_string()),
                intent: Some(denial.intent.as_str().to_string()),
                capability: Some(denial.capability),
                required: denial.required.clone(),
                missing: denial.missing.clone(),
            };
        }

        Self {
            status: status_for_error(error),
            error: error.to_string(),
            code: stable_code_for_error(error).map(ToString::to_string),
            intent: None,
            capability: None,
            required: Vec::new(),
            missing: Vec::new(),
        }
    }

    /// Problem describing a save the consumer rejected, for the
    /// `save_failure` field of a successful intent response (audit §6.4).
    ///
    /// Status and code are the ones the same error would have produced as an
    /// HTTP problem, so clients handle both transports with one mapping; a
    /// failure without a stable code is reported as
    /// [`BROWSER_SAVE_FAILED_CODE`].
    pub fn from_save_failure(error: &PageBuilderAdminFacadeError) -> Self {
        let conflict = error.stable_code.as_deref() == Some(BROWSER_REVISION_CONFLICT_CODE);
        Self::save_problem(
            if conflict {
                HTTP_CONFLICT
            } else {
                HTTP_UNPROCESSABLE_ENTITY
            },
            error.stable_code.as_deref(),
            error.to_string(),
        )
    }

    /// Problem describing an autosave that could not be requested at all —
    /// for example because the edit left blocking validation diagnostics.
    /// Reported through `save_failure` like a save the consumer rejected.
    pub fn from_blocked_save(error: &BrowserIntentDispatchError) -> Self {
        Self::save_problem(dispatch_status(error), None, error.to_string())
    }

    fn save_problem(status: u16, stable_code: Option<&str>, error: String) -> Self {
        Self {
            status,
            error,
            code: Some(stable_code.unwrap_or(BROWSER_SAVE_FAILED_CODE).to_string()),
            intent: Some("save".to_string()),
            capability: None,
            required: Vec::new(),
            missing: Vec::new(),
        }
    }
}

impl From<&PagesBrowserIntentAccessError> for PagesBrowserIntentProblem {
    fn from(error: &PagesBrowserIntentAccessError) -> Self {
        Self::from_error(error)
    }
}

fn status_for_error(error: &PagesBrowserIntentAccessError) -> u16 {
    match error {
        PagesBrowserIntentAccessError::Capability(BrowserCapabilityAccessError::Denied(_)) => {
            HTTP_FORBIDDEN
        }
        PagesBrowserIntentAccessError::Capability(BrowserCapabilityAccessError::Dispatch(
            error,
        )) => dispatch_status(error),
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::PageNotFound) => {
            HTTP_NOT_FOUND
        }
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::DocumentUnavailable) => {
            HTTP_FORBIDDEN
        }
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::PageMismatch { .. }) => {
            HTTP_BAD_REQUEST
        }
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Dispatch(error)) => {
            dispatch_status(error)
        }
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::GenerationConflict { .. }
            | SsrDraftSessionError::PageMismatch { .. },
        )) => HTTP_CONFLICT,
        // The draft session belongs to another user/tenant: never leak which
        // one (audit §2.5).
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::OwnerMismatch,
        )) => HTTP_FORBIDDEN,
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::RecordTooLarge { .. },
        )) => HTTP_PAYLOAD_TOO_LARGE,
        // Draft storage that is busy or unusable (for example a session
        // directory the server could not secure) is the server's condition,
        // not a request the client could correct.
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::LockTimeout(_) | SsrDraftSessionError::Io(_),
        )) => HTTP_SERVICE_UNAVAILABLE,
        // A record written by a newer build is a deployment condition, not a bad request: the
        // browser cannot fix it by retrying, and answering 422 would read as a client error.
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::UnsupportedFormat { .. },
        )) => HTTP_SERVICE_UNAVAILABLE,
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Facade(error))
            if error.stable_code.as_deref() == Some(BROWSER_REVISION_CONFLICT_CODE) =>
        {
            HTTP_CONFLICT
        }
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Transport(_)) => {
            HTTP_BAD_GATEWAY
        }
        _ => HTTP_UNPROCESSABLE_ENTITY,
    }
}

fn stable_code_for_error(error: &PagesBrowserIntentAccessError) -> Option<&str> {
    match error {
        PagesBrowserIntentAccessError::Capability(BrowserCapabilityAccessError::Dispatch(
            BrowserIntentDispatchError::RevisionConflict { .. },
        ))
        | PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Dispatch(
            BrowserIntentDispatchError::RevisionConflict { .. },
        )) => Some(BROWSER_REVISION_CONFLICT_CODE),
        PagesBrowserIntentAccessError::Capability(BrowserCapabilityAccessError::Dispatch(
            BrowserIntentDispatchError::ProjectHashConflict { .. },
        ))
        | PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Dispatch(
            BrowserIntentDispatchError::ProjectHashConflict { .. },
        )) => Some(BROWSER_PROJECT_HASH_CONFLICT_CODE),
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::GenerationConflict { .. },
        )) => Some(BROWSER_DRAFT_GENERATION_CONFLICT_CODE),
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Facade(error)) => {
            error.stable_code.as_deref()
        }
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::DocumentUnavailable) => {
            Some(BROWSER_DOCUMENT_UNAVAILABLE_CODE)
        }
        _ => None,
    }
}

fn dispatch_status(error: &BrowserIntentDispatchError) -> u16 {
    match error {
        BrowserIntentDispatchError::RevisionConflict { .. }
        | BrowserIntentDispatchError::ProjectHashConflict { .. } => HTTP_CONFLICT,
        _ => HTTP_UNPROCESSABLE_ENTITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::BrowserIntentKind;
    use rustok_page_builder_admin::{BrowserCapabilityDenial, PageBuilderAdminFacadeError};
    use serde_json::json;

    #[test]
    fn capability_denial_has_stable_problem_contract() {
        let error = PagesBrowserIntentAccessError::Capability(
            BrowserCapabilityDenial {
                intent: BrowserIntentKind::Save,
                capability: EditorCapability::Edit,
                required: vec![EditorCapability::Edit],
                missing: vec![EditorCapability::Edit],
            }
            .into(),
        );
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_FORBIDDEN);
        assert_eq!(
            problem.code.as_deref(),
            Some(BROWSER_CAPABILITY_DENIAL_CODE)
        );
        assert_eq!(problem.intent.as_deref(), Some("save"));
        assert_eq!(problem.capability, Some(EditorCapability::Edit));
        assert_eq!(problem.required, vec![EditorCapability::Edit]);
        assert_eq!(problem.missing, vec![EditorCapability::Edit]);
        assert_eq!(
            serde_json::to_value(&problem).expect("serialize problem"),
            json!({
                "status": 403,
                "error": "browser intent `save` requires editor capability `edit`",
                "code": "FLY_CAPABILITY_DENIED",
                "intent": "save",
                "capability": "edit",
                "required": ["edit"],
                "missing": ["edit"]
            })
        );
    }

    #[test]
    fn multiple_missing_capabilities_are_preserved_for_clients() {
        let error = PagesBrowserIntentAccessError::Capability(
            BrowserCapabilityDenial {
                intent: BrowserIntentKind::SelectAsset,
                capability: EditorCapability::Properties,
                required: vec![EditorCapability::Properties, EditorCapability::Assets],
                missing: vec![EditorCapability::Properties, EditorCapability::Assets],
            }
            .into(),
        );
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_FORBIDDEN);
        assert_eq!(problem.intent.as_deref(), Some("select_asset"));
        assert_eq!(
            problem.required,
            vec![EditorCapability::Properties, EditorCapability::Assets]
        );
        assert_eq!(
            problem.missing,
            vec![EditorCapability::Properties, EditorCapability::Assets]
        );
    }

    #[test]
    fn revision_conflict_maps_to_conflict_without_capability_fields() {
        let error = PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Dispatch(
            BrowserIntentDispatchError::RevisionConflict {
                expected: "rev-2".to_string(),
                actual: "rev-1".to_string(),
            },
        ));
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_CONFLICT);
        assert_eq!(
            problem.code.as_deref(),
            Some(BROWSER_REVISION_CONFLICT_CODE)
        );
        assert!(problem.intent.is_none());
        assert!(problem.capability.is_none());
        assert!(problem.required.is_empty());
        assert!(problem.missing.is_empty());
    }

    #[test]
    fn consumer_revision_conflict_preserves_facade_stable_code() {
        let error = PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Facade(
            PageBuilderAdminFacadeError::with_stable_code(
                "Page Builder revision conflict",
                BROWSER_REVISION_CONFLICT_CODE,
            ),
        ));
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_CONFLICT);
        assert_eq!(
            problem.code.as_deref(),
            Some(BROWSER_REVISION_CONFLICT_CODE)
        );
    }

    #[test]
    fn save_failure_problem_matches_the_http_mapping_of_the_same_error() {
        for facade_error in [
            PageBuilderAdminFacadeError::with_stable_code(
                "Page Builder revision conflict",
                BROWSER_REVISION_CONFLICT_CODE,
            ),
            PageBuilderAdminFacadeError::with_stable_code("quota exceeded", "QUOTA_EXCEEDED"),
        ] {
            let structured = PagesBrowserIntentProblem::from_save_failure(&facade_error);
            let http = PagesBrowserIntentProblem::from(&PagesBrowserIntentAccessError::Pages(
                PagesBrowserIntentError::Facade(facade_error),
            ));
            assert_eq!(structured.status, http.status);
            assert_eq!(structured.code, http.code);
            assert_eq!(structured.error, http.error);
            assert_eq!(structured.intent.as_deref(), Some("save"));
        }

        // Without a stable code the HTTP problem has none; the structured
        // one always names the failure so clients can key on it.
        let plain = PagesBrowserIntentProblem::from_save_failure(
            &PageBuilderAdminFacadeError::new("Pages rejected the document"),
        );
        assert_eq!(plain.status, HTTP_UNPROCESSABLE_ENTITY);
        assert_eq!(plain.code.as_deref(), Some(BROWSER_SAVE_FAILED_CODE));
    }

    #[test]
    fn blocked_autosave_is_reported_as_a_failed_save() {
        let problem = PagesBrowserIntentProblem::from_blocked_save(
            &BrowserIntentDispatchError::Payload("save blocked".to_string()),
        );
        assert_eq!(problem.status, HTTP_UNPROCESSABLE_ENTITY);
        assert_eq!(problem.code.as_deref(), Some(BROWSER_SAVE_FAILED_CODE));
        assert_eq!(problem.intent.as_deref(), Some("save"));
        assert_eq!(
            problem.error,
            "browser intent payload is invalid: save blocked"
        );
    }

    #[test]
    fn unsupported_request_batch_is_unprocessable() {
        let error = PagesBrowserIntentAccessError::Pages(
            PagesBrowserIntentError::UnsupportedRequestBatch { count: 2 },
        );
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_UNPROCESSABLE_ENTITY);
        assert!(problem.error.contains("2 consumer requests"));
    }

    #[test]
    fn withheld_document_is_forbidden_with_a_stable_browser_code() {
        let error =
            PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::DocumentUnavailable);
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_FORBIDDEN);
        assert_eq!(
            problem.code.as_deref(),
            Some(BROWSER_DOCUMENT_UNAVAILABLE_CODE)
        );
        assert_eq!(problem.error, crate::core::DOCUMENT_UNAVAILABLE_MESSAGE);
    }

    #[test]
    fn draft_generation_conflict_has_a_stable_browser_code() {
        let error = PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::GenerationConflict {
                expected: 2,
                actual: 1,
            },
        ));
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_CONFLICT);
        assert_eq!(
            problem.code.as_deref(),
            Some(BROWSER_DRAFT_GENERATION_CONFLICT_CODE)
        );
    }

    /// A record of a newer build parses, so it must not be reported as a damaged request (422);
    /// it is a deployment condition the browser cannot fix.
    #[test]
    fn a_draft_record_of_a_newer_build_is_service_unavailable() {
        let error = PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::UnsupportedFormat {
                found: 2,
                supported: 1,
            },
        ));
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_SERVICE_UNAVAILABLE);
        assert_eq!(problem.code, None);
    }

    #[test]
    fn unusable_draft_storage_is_service_unavailable() {
        for error in [
            SsrDraftSessionError::Io("draft session directory is not usable".to_string()),
            SsrDraftSessionError::LockTimeout("token".to_string()),
        ] {
            let error = PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(error));
            let problem = PagesBrowserIntentProblem::from(&error);
            assert_eq!(problem.status, HTTP_SERVICE_UNAVAILABLE);
        }
    }

    #[test]
    fn draft_owner_mismatch_maps_to_forbidden_without_leaking_ids() {
        let error = PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Draft(
            SsrDraftSessionError::OwnerMismatch,
        ));
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_FORBIDDEN);
        // The detail is opaque: no tenant or user identifiers.
        assert_eq!(problem.error, "SSR draft session belongs to another owner");
    }

    #[test]
    fn page_not_found_maps_to_not_found() {
        let error = PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::PageNotFound);
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_NOT_FOUND);
        assert_eq!(problem.error, "Pages document was not found");
    }

    #[test]
    fn malformed_capability_preflight_payload_stays_unprocessable() {
        let error =
            PagesBrowserIntentAccessError::Capability(BrowserCapabilityAccessError::Dispatch(
                BrowserIntentDispatchError::Payload("invalid key stroke".to_string()),
            ));
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_UNPROCESSABLE_ENTITY);
        assert!(problem.code.is_none());
        assert!(problem.required.is_empty());
    }
}
