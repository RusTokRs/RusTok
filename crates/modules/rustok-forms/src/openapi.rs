use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::submit_form_json,
        crate::controllers::submit_form_urlencoded,
        crate::controllers::list_form_submissions,
        crate::controllers::update_form_submission_state,
    ),
    components(
        schemas(
            crate::dto::SubmitFormResponse,
            crate::dto::FormSubmissionResponse,
            crate::dto::FormSubmissionState,
            crate::dto::ListFormSubmissionsFilter,
            crate::dto::UpdateFormSubmissionStateInput,
        )
    ),
    tags((name = "forms", description = "Form submission intake and triage endpoints"))
)]
pub struct FormsApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    FormsApiDoc::openapi()
}
