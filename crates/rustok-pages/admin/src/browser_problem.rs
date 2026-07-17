use crate::browser_intent::PagesBrowserIntentError;
use crate::contribution_browser_intent::PagesBrowserIntentAccessError;
use fly_ui::EditorCapability;
use rustok_page_builder_admin::{
    BrowserCapabilityAccessError, BrowserIntentDispatchError, SsrDraftSessionError,
    BROWSER_CAPABILITY_DENIAL_CODE,
};
use serde::{Deserialize, Serialize};

const HTTP_BAD_REQUEST: u16 = 400;
const HTTP_FORBIDDEN: u16 = 403;
const HTTP_NOT_FOUND: u16 = 404;
const HTTP_CONFLICT: u16 = 409;
const HTTP_UNPROCESSABLE_ENTITY: u16 = 422;
const HTTP_BAD_GATEWAY: u16 = 502;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PagesBrowserIntentProblem {
    pub status: u16,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<EditorCapability>,
}

impl PagesBrowserIntentProblem {
    pub fn from_error(error: &PagesBrowserIntentAccessError) -> Self {
        if let Some(denial) = error.capability_denial() {
            return Self {
                status: HTTP_FORBIDDEN,
                error: error.to_string(),
                code: Some(BROWSER_CAPABILITY_DENIAL_CODE.to_string()),
                intent: Some(denial.intent.clone()),
                capability: Some(denial.capability),
            };
        }

        Self {
            status: status_for_error(error),
            error: error.to_string(),
            code: None,
            intent: None,
            capability: None,
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
        PagesBrowserIntentAccessError::Capability(BrowserCapabilityAccessError::Dispatch(error)) => {
            dispatch_status(error)
        }
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::PageNotFound) => {
            HTTP_NOT_FOUND
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
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Facade(error))
            if error.stable_code.as_deref() == Some("REVISION_CONFLICT") =>
        {
            HTTP_CONFLICT
        }
        PagesBrowserIntentAccessError::Pages(PagesBrowserIntentError::Transport(_)) => {
            HTTP_BAD_GATEWAY
        }
        _ => HTTP_UNPROCESSABLE_ENTITY,
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
    use rustok_page_builder_admin::BrowserCapabilityDenial;
    use serde_json::json;

    #[test]
    fn capability_denial_has_stable_problem_contract() {
        let error = PagesBrowserIntentAccessError::Capability(
            BrowserCapabilityDenial {
                intent: "save".to_string(),
                capability: EditorCapability::Publish,
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
        assert_eq!(problem.capability, Some(EditorCapability::Publish));
        assert_eq!(
            serde_json::to_value(&problem).expect("serialize problem"),
            json!({
                "status": 403,
                "error": "browser intent `save` requires editor capability `publish`",
                "code": "FLY_CAPABILITY_DENIED",
                "intent": "save",
                "capability": "publish"
            })
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
        assert!(problem.code.is_none());
        assert!(problem.intent.is_none());
        assert!(problem.capability.is_none());
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
        let error = PagesBrowserIntentAccessError::Capability(
            BrowserCapabilityAccessError::Dispatch(BrowserIntentDispatchError::Payload(
                "invalid key stroke".to_string(),
            )),
        );
        let problem = PagesBrowserIntentProblem::from(&error);
        assert_eq!(problem.status, HTTP_UNPROCESSABLE_ENTITY);
        assert!(problem.code.is_none());
    }
}
