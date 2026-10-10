use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::model::FormSubmission;

pub type ApiError = GraphqlHttpError;

const FORM_SUBMISSIONS_QUERY: &str = "query FormSubmissions { formSubmissions { id formId locale pageId payload state createdAt handledAt handledBy } }";
const UPDATE_FORM_SUBMISSION_STATE_MUTATION: &str = "mutation UpdateFormSubmissionState($id: UUID!, $input: UpdateFormSubmissionStateInput!) { updateFormSubmissionState(id: $id, input: $input) { id formId locale pageId payload state createdAt handledAt handledBy } }";

#[derive(Debug, Deserialize)]
struct FormSubmissionsResponse {
    #[serde(rename = "formSubmissions")]
    form_submissions: Vec<FormSubmission>,
}

#[derive(Debug, Deserialize)]
struct UpdateFormSubmissionStateResponse {
    #[serde(rename = "updateFormSubmissionState")]
    updated: FormSubmission,
}

#[derive(Debug, Serialize)]
struct UpdateFormSubmissionStateVariables {
    id: String,
    input: UpdateFormSubmissionStateInput,
}

#[derive(Debug, Serialize)]
struct UpdateFormSubmissionStateInput {
    state: String,
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
}

pub async fn fetch_form_submissions(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<FormSubmission>, ApiError> {
    let response: FormSubmissionsResponse = request(
        FORM_SUBMISSIONS_QUERY,
        serde_json::json!({}),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.form_submissions)
}

pub async fn set_form_submission_state(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: Uuid,
    state: &str,
) -> Result<FormSubmission, ApiError> {
    let response: UpdateFormSubmissionStateResponse = request(
        UPDATE_FORM_SUBMISSION_STATE_MUTATION,
        UpdateFormSubmissionStateVariables {
            id: id.to_string(),
            input: UpdateFormSubmissionStateInput {
                state: state.to_string(),
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.updated)
}
