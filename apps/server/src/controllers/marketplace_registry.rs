use axum::{
    body::Body,
    body::Bytes,
    extract::{Path, Query, State},
    http::{
        header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH},
        HeaderMap, HeaderName, HeaderValue, Response, StatusCode,
    },
    response::IntoResponse,
    routing::{get, post, put},
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

use crate::error::Error;
use crate::modules::{CatalogManifestModule, ManifestManager, ModulesManifest};
use crate::services::marketplace_catalog::{
    legacy_registry_catalog_module_path, legacy_registry_catalog_path,
    registry_catalog_from_modules, registry_catalog_module_path, registry_catalog_path,
    registry_owner_transfer_path, registry_publish_approve_path, registry_publish_artifact_path,
    registry_publish_path, registry_publish_reject_path, registry_publish_stage_report_path,
    registry_publish_status_path, registry_publish_validate_path, registry_yank_path,
    validate_registry_mutation_schema_version, RegistryCatalogModule, RegistryCatalogResponse,
    RegistryGovernanceAction, RegistryModerationPolicy, RegistryMutationResponse,
    RegistryOwnerTransferRequest, RegistryPublishDecisionRequest, RegistryPublishRequest,
    RegistryPublishStatusFollowUpGate, RegistryPublishStatusResponse,
    RegistryPublishStatusValidationStage, RegistryPublishValidationRequest,
    RegistryValidationStageReportRequest, RegistryYankRequest,
};
use crate::services::registry_governance::{
    moderation_policy_snapshot, release_status_label, request_status_label,
    validation_stage_allowed_terminal_reason_codes, validation_stage_status_label,
    RegistryArtifactUpload, RegistryFollowUpGateSnapshot, RegistryGovernanceService,
    RegistryValidationStageSnapshot, REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
    REGISTRY_OWNER_TRANSFER_REASON_CODES, REGISTRY_REJECT_REASON_CODES,
    REGISTRY_VALIDATION_STAGE_REASON_CODES, REGISTRY_YANK_REASON_CODES,
};

#[derive(Debug, Default, Deserialize, ToSchema, utoipa::IntoParams)]
struct RegistryCatalogListParams {
    search: Option<String>,
    category: Option<String>,
    tag: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

/// GET /v1/catalog - Reference read-only marketplace registry catalog
#[utoipa::path(
    get,
    path = "/v1/catalog",
    tag = "marketplace",
    params(
        RegistryCatalogListParams,
        ("If-None-Match" = Option<String>, Header, description = "Conditional request ETag")
    ),
    responses(
        (
            status = 200,
            description = "Schema-versioned reference catalog of first-party modules with optional filtering and paging",
            body = RegistryCatalogResponse,
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry"),
                ("x-total-count" = i64, description = "Total number of modules in the filtered collection before limit/offset")
            )
        ),
        (
            status = 304,
            description = "Catalog has not changed since the provided ETag",
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry"),
                ("x-total-count" = i64, description = "Total number of modules in the filtered collection before limit/offset")
            )
        )
    )
)]
async fn catalog(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(params): Query<RegistryCatalogListParams>,
) -> Result<Response<Body>, Error> {
    let first_party_modules = sort_catalog_modules(filter_catalog_modules(
        first_party_catalog_modules(&ctx).await?,
        &params,
    ));
    let (first_party_modules, total_count) = paginate_catalog_modules(first_party_modules, &params);
    let payload = registry_catalog_from_modules(first_party_modules);

    build_registry_response(&headers, &payload, Some(total_count))
}

/// GET /v1/catalog/{slug} - Reference read-only marketplace registry module detail
#[utoipa::path(
    get,
    path = "/v1/catalog/{slug}",
    tag = "marketplace",
    params(
        ("slug" = String, Path, description = "Module slug"),
        ("If-None-Match" = Option<String>, Header, description = "Conditional request ETag")
    ),
    responses(
        (
            status = 200,
            description = "Normalized first-party module detail from the reference registry catalog",
            body = RegistryCatalogModule,
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry")
            )
        ),
        (
            status = 304,
            description = "Module detail has not changed since the provided ETag",
            headers(
                ("etag" = String, description = "Current entity tag for conditional GET"),
                ("cache-control" = String, description = "Shared cache policy for the reference registry")
            )
        ),
        (
            status = 404,
            description = "Module is not present in the reference registry catalog"
        )
    )
)]
async fn catalog_module(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Result<Response<Body>, Error> {
    let module = first_party_catalog_modules(&ctx)
        .await?
        .into_iter()
        .find(|module| module.slug == slug)
        .map(RegistryCatalogModule::from_catalog_module)
        .ok_or(Error::NotFound)?;

    build_registry_response(&headers, &module, None)
}

/// POST /v2/catalog/publish - Registry publish request entrypoint
#[utoipa::path(
    post,
    path = "/v2/catalog/publish",
    tag = "marketplace",
    request_body = RegistryPublishRequest,
    responses(
        (
            status = 200,
            description = "Dry-run registry publish request accepted and normalized",
            body = RegistryMutationResponse
        ),
        (
            status = 202,
            description = "Live registry publish request created and awaiting artifact upload",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Publish request failed local contract validation"
        )
    )
)]
async fn publish(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(request): Json<RegistryPublishRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;

    let warnings = validate_publish_request_payload(&request)?;
    let moderation_policy = registry_moderation_policy_response(&request.module.ownership);

    if !request.dry_run {
        if !moderation_policy.live_publish_supported {
            return Err(Error::BadRequest(
                moderation_policy.restriction_reason.clone(),
            ));
        }

        let actor = request_actor_from_headers(&headers);
        let publisher = request_publisher_from_headers(&headers);
        let created = RegistryGovernanceService::new(ctx.db.clone())
            .create_publish_request(&request, &actor, publisher.as_deref(), &warnings)
            .await
            .map_err(|error| {
                Error::Message(format!(
                    "Failed to create registry publish request: {error}"
                ))
            })?;

        return Ok((
            StatusCode::ACCEPTED,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "publish".to_string(),
                dry_run: false,
                accepted: true,
                request_id: Some(created.id.clone()),
                status: Some(request_status_label(created.status).to_string()),
                slug: created.slug,
                version: created.version,
                warnings,
                errors: Vec::new(),
                next_step: Some(format!(
                    "Upload the module artifact via PUT {}",
                    registry_publish_artifact_path().replace("{request_id}", &created.id)
                )),
                moderation_policy: Some(moderation_policy.clone()),
            }),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "publish".to_string(),
            dry_run: true,
            accepted: true,
            request_id: None,
            status: Some("dry_run".to_string()),
            slug: request.module.slug.clone(),
            version: request.module.version.clone(),
            warnings,
            errors: Vec::new(),
            next_step: Some(
                "Dry-run preview only. Re-run with dry_run=false to create a publish request."
                    .to_string(),
            ),
            moderation_policy: Some(moderation_policy),
        }),
    ))
}

/// GET /v2/catalog/publish/{request_id} - Registry publish request lifecycle status
#[utoipa::path(
    get,
    path = "/v2/catalog/publish/{request_id}",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    responses(
        (
            status = 200,
            description = "Current lifecycle state of a registry publish request",
            body = RegistryPublishStatusResponse
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn publish_status(
    State(ctx): State<AppContext>,
    Path(request_id): Path<String>,
) -> Result<Json<RegistryPublishStatusResponse>, Error> {
    let governance = RegistryGovernanceService::new(ctx.db.clone());
    let request = governance
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let follow_up = governance
        .publish_request_follow_up_snapshot(&request)
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Failed to load registry publish request follow-up stages: {error}"
            ))
        })?;
    let lifecycle = governance
        .lifecycle_snapshot(&request.slug, &request.ownership)
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Failed to load registry publish request lifecycle actions: {error}"
            ))
        })?;
    let mut warnings = deserialize_message_list(&request.validation_warnings);
    let next_step =
        publish_request_status_next_step(&request, &request_id, &follow_up.validation_stages);
    if follow_up.approval_override_required {
        warnings.push(approval_override_warning_message(
            &follow_up.validation_stages,
        ));
    }

    Ok(Json(RegistryPublishStatusResponse {
        schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
        request_id: request.id,
        slug: request.slug,
        version: request.version,
        status: request_status_label(request.status.clone()).to_string(),
        accepted: publish_request_accepted(&request.status),
        warnings,
        errors: deserialize_message_list(&request.validation_errors),
        follow_up_gates: follow_up
            .follow_up_gates
            .into_iter()
            .map(publish_status_follow_up_gate)
            .collect(),
        validation_stages: follow_up
            .validation_stages
            .iter()
            .map(publish_status_validation_stage)
            .collect(),
        approval_override_required: follow_up.approval_override_required,
        approval_override_reason_codes: REGISTRY_APPROVE_OVERRIDE_REASON_CODES
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        governance_actions: lifecycle
            .map(|snapshot| {
                snapshot
                    .governance_actions
                    .into_iter()
                    .map(registry_governance_action_response)
                    .collect()
            })
            .unwrap_or_default(),
        next_step,
        moderation_policy: registry_moderation_policy_response(&request.ownership),
    }))
}

/// PUT /v2/catalog/publish/{request_id}/artifact - Upload a registry publish artifact
#[utoipa::path(
    put,
    path = "/v2/catalog/publish/{request_id}/artifact",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body(
        content = String,
        content_type = "application/octet-stream",
        description = "Opaque module publish artifact bytes"
    ),
    responses(
        (
            status = 202,
            description = "Artifact uploaded and queued for validation",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Artifact upload failed local validation"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn upload_publish_artifact(
    State(ctx): State<AppContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, Error> {
    if body.is_empty() {
        return Err(Error::BadRequest(
            "Registry publish artifact upload requires a non-empty request body".to_string(),
        ));
    }

    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("application/octet-stream")
        .to_string();
    let actor = request_actor_from_headers(&headers);

    let request = RegistryGovernanceService::new(ctx.db.clone())
        .upload_publish_artifact(
            &request_id,
            &actor,
            RegistryArtifactUpload {
                content_type,
                bytes: body,
            },
        )
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "publish".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&request.status),
            request_id: Some(request.id.clone()),
            status: Some(request_status_label(request.status.clone()).to_string()),
            slug: request.slug,
            version: request.version,
            warnings: deserialize_message_list(&request.validation_warnings),
            errors: deserialize_message_list(&request.validation_errors),
            next_step: publish_request_next_step(&request.status, &request_id),
            moderation_policy: None,
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/validate - Run publish artifact validation outside the upload path
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/validate",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryPublishValidationRequest,
    responses(
        (
            status = 200,
            description = "Publish request validation already completed or returned the current terminal lifecycle state",
            body = RegistryMutationResponse
        ),
        (
            status = 202,
            description = "Publish request validation was accepted and queued as a background lifecycle step",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Validation request failed lifecycle or governance checks"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn validate_publish_request_step(
    State(ctx): State<AppContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RegistryPublishValidationRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let existing = RegistryGovernanceService::new(ctx.db.clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let actor = request_actor_from_headers(&headers);

    if request.dry_run {
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "validate".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some("dry_run".to_string()),
                slug: existing.slug,
                version: existing.version,
                warnings: vec!["Dry-run preview only. Re-run with dry_run=false to execute publish validation outside the upload path.".to_string()],
                errors: Vec::new(),
                next_step: Some("Use the same endpoint with dry_run=false after artifact upload completes.".to_string()),
                moderation_policy: None,
            }),
        ));
    }

    let governance = RegistryGovernanceService::new(ctx.db.clone());
    let validation = governance
        .validate_publish_request(&request_id, &actor)
        .await
        .map_err(map_registry_governance_error)?;
    if validation.queued {
        let db = ctx.db.clone();
        let request_id = validation.request.id.clone();
        let validation_job_id = validation.validation_job_id.clone().ok_or_else(|| {
            Error::Message(format!(
                "Validation was queued for publish request '{request_id}', but no validation job id was returned"
            ))
        })?;
        let actor = actor.clone();
        tokio::spawn(async move {
            if let Err(error) = RegistryGovernanceService::new(db)
                .run_publish_validation_job(&validation_job_id, &actor)
                .await
            {
                tracing::error!(
                    error = %error,
                    validation_job_id = %validation_job_id,
                    request_id = %request_id,
                    actor = %actor,
                    "Background registry publish validation failed"
                );
            }
        });
    }

    let validated = validation.request;
    let status_code = if validated.status
        == crate::models::registry_publish_request::RegistryPublishRequestStatus::Validating
    {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };

    Ok((
        status_code,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "validate".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&validated.status),
            request_id: Some(validated.id.clone()),
            status: Some(request_status_label(validated.status.clone()).to_string()),
            slug: validated.slug,
            version: validated.version,
            warnings: deserialize_message_list(&validated.validation_warnings),
            errors: deserialize_message_list(&validated.validation_errors),
            next_step: publish_request_next_step(&validated.status, &validated.id),
            moderation_policy: None,
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/stages - Record or requeue an external validation stage
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/stages",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryValidationStageReportRequest,
    responses(
        (
            status = 200,
            description = "Dry-run preview or live validation stage update accepted",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Validation stage update failed lifecycle or governance checks"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn report_validation_stage(
    State(ctx): State<AppContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RegistryValidationStageReportRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    validate_validation_stage_report_request(&request)?;
    let existing = RegistryGovernanceService::new(ctx.db.clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let actor = request_actor_from_headers(&headers);

    if request.dry_run {
        let mut warnings = Vec::new();
        let normalized_status = request.status.trim().to_ascii_lowercase();
        if matches!(normalized_status.as_str(), "passed" | "failed" | "blocked")
            && request
                .reason_code
                .as_deref()
                .map(str::trim)
                .is_none_or(|value| value.is_empty())
        {
            warnings.push(format!(
                "Live validation stage status '{}' should include reason_code ({}).",
                normalized_status,
                REGISTRY_VALIDATION_STAGE_REASON_CODES.join(", ")
            ));
        }
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "validation_stage".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some(normalized_status),
                slug: existing.slug,
                version: existing.version,
                warnings,
                errors: Vec::new(),
                next_step: Some(
                    "Dry-run preview only. Re-run with dry_run=false to persist the validation stage update."
                        .to_string(),
                ),
                moderation_policy: None,
            }),
        ));
    }

    let result = RegistryGovernanceService::new(ctx.db.clone())
        .report_validation_stage(
            &request_id,
            &actor,
            &request.stage,
            &request.status,
            request.detail.as_deref(),
            request.reason_code.as_deref(),
            request.requeue,
        )
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "validation_stage".to_string(),
            dry_run: false,
            accepted: true,
            request_id: Some(result.request.id.clone()),
            status: Some(validation_stage_status_label(result.stage.status).to_string()),
            slug: result.request.slug,
            version: result.request.version,
            warnings: Vec::new(),
            errors: Vec::new(),
            next_step: Some(format!(
                "Inspect {} for the updated publish lifecycle and follow-up validation stages.",
                registry_publish_status_path().replace("{request_id}", &request_id)
            )),
            moderation_policy: None,
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/approve - Finalize a validated publish request
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/approve",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryPublishDecisionRequest,
    responses(
        (
            status = 200,
            description = "Validated publish request approved and projected into the published registry release trail",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Approve request failed governance validation"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn approve_publish_request(
    State(ctx): State<AppContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RegistryPublishDecisionRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    validate_publish_approve_request(&request)?;
    let governance = RegistryGovernanceService::new(ctx.db.clone());
    let existing = governance
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let follow_up = governance
        .publish_request_follow_up_snapshot(&existing)
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Failed to load registry publish request follow-up stages: {error}"
            ))
        })?;
    let actor = request_actor_from_headers(&headers);

    if request.dry_run {
        let mut warnings = vec![String::from(
            "Dry-run preview only. Re-run with dry_run=false to finalize the publish request.",
        )];
        let next_step = if follow_up.approval_override_required {
            warnings.push(approval_override_warning_message(
                &follow_up.validation_stages,
            ));
            Some(approval_override_next_step(
                &existing.id,
                &follow_up.validation_stages,
            ))
        } else {
            Some(
                "Use the same endpoint with dry_run=false after artifact validation succeeds."
                    .to_string(),
            )
        };
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "approve".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some("dry_run".to_string()),
                slug: existing.slug,
                version: existing.version,
                warnings,
                errors: Vec::new(),
                next_step,
                moderation_policy: None,
            }),
        ));
    }

    let publisher = request_publisher_from_headers(&headers);
    let approved = RegistryGovernanceService::new(ctx.db.clone())
        .approve_publish_request(
            &request_id,
            &actor,
            publisher.as_deref(),
            request.reason.as_deref(),
            request.reason_code.as_deref(),
        )
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "approve".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&approved.status),
            request_id: Some(approved.id.clone()),
            status: Some(request_status_label(approved.status.clone()).to_string()),
            slug: approved.slug,
            version: approved.version,
            warnings: deserialize_message_list(&approved.validation_warnings),
            errors: deserialize_message_list(&approved.validation_errors),
            next_step: publish_request_next_step(&approved.status, &approved.id),
            moderation_policy: None,
        }),
    ))
}

/// POST /v2/catalog/publish/{request_id}/reject - Reject a publish request with a governance reason
#[utoipa::path(
    post,
    path = "/v2/catalog/publish/{request_id}/reject",
    tag = "marketplace",
    params(
        ("request_id" = String, Path, description = "Registry publish request identifier")
    ),
    request_body = RegistryPublishDecisionRequest,
    responses(
        (
            status = 200,
            description = "Publish request rejected with surfaced governance reason",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Reject request failed governance validation"
        ),
        (
            status = 404,
            description = "Registry publish request was not found"
        )
    )
)]
async fn reject_publish_request(
    State(ctx): State<AppContext>,
    Path(request_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RegistryPublishDecisionRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let warnings = validate_publish_reject_request(&request)?;
    let existing = RegistryGovernanceService::new(ctx.db.clone())
        .get_publish_request(&request_id)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to load registry publish request: {error}"))
        })?
        .ok_or(Error::NotFound)?;
    let actor = request_actor_from_headers(&headers);

    if request.dry_run {
        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "reject".to_string(),
                dry_run: true,
                accepted: true,
                request_id: Some(request_id),
                status: Some("dry_run".to_string()),
                slug: existing.slug,
                version: existing.version,
                warnings: warnings
                    .into_iter()
                    .chain(std::iter::once(
                        "Dry-run preview only. Re-run with dry_run=false to persist the governance rejection."
                            .to_string(),
                    ))
                    .collect(),
                errors: Vec::new(),
                next_step: Some(format!(
                    "Use the same endpoint with dry_run=false, a non-empty reason, and a supported reason_code ({}) to reject the publish request.",
                    REGISTRY_REJECT_REASON_CODES.join(", ")
                )),
                moderation_policy: None,
            }),
        ));
    }
    let reason = request
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live registry publish reject requires a non-empty reason for the governance audit trail"
                    .to_string(),
            )
        })?;
    let reason_code = request
        .reason_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::BadRequest(
                "Live registry publish reject requires a non-empty reason_code for the policy audit trail"
                    .to_string(),
            )
        })?;

    let rejected = RegistryGovernanceService::new(ctx.db.clone())
        .reject_publish_request(&request_id, &actor, reason, reason_code)
        .await
        .map_err(map_registry_governance_error)?;

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "reject".to_string(),
            dry_run: false,
            accepted: publish_request_accepted(&rejected.status),
            request_id: Some(rejected.id.clone()),
            status: Some(request_status_label(rejected.status.clone()).to_string()),
            slug: rejected.slug,
            version: rejected.version,
            warnings,
            errors: deserialize_message_list(&rejected.validation_errors),
            next_step: publish_request_next_step(&rejected.status, &rejected.id),
            moderation_policy: None,
        }),
    ))
}

/// POST /v2/catalog/yank - Registry release lifecycle yank contract
#[utoipa::path(
    post,
    path = "/v2/catalog/yank",
    tag = "marketplace",
    request_body = RegistryYankRequest,
    responses(
        (
            status = 200,
            description = "Registry yank request accepted and normalized",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Yank request failed local contract validation"
        ),
        (
            status = 404,
            description = "Published release was not found"
        )
    )
)]
async fn yank(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(request): Json<RegistryYankRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let warnings = validate_yank_request(&request)?;

    if !request.dry_run {
        let reason = request
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::BadRequest(
                    "Live registry yank requires a non-empty reason for the audit trail"
                        .to_string(),
                )
            })?;
        let reason_code = request
            .reason_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::BadRequest(
                    "Live registry yank requires a non-empty reason_code for the policy audit trail"
                        .to_string(),
                )
            })?;
        let actor = request_actor_from_headers(&headers);
        let release = RegistryGovernanceService::new(ctx.db.clone())
            .yank_release(&request.slug, &request.version, reason, reason_code, &actor)
            .await
            .map_err(map_registry_governance_error)?;

        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "yank".to_string(),
                dry_run: false,
                accepted: true,
                request_id: release.request_id,
                status: Some(release_status_label(release.status).to_string()),
                slug: request.slug,
                version: request.version,
                warnings,
                errors: Vec::new(),
                next_step: None,
                moderation_policy: None,
            }),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "yank".to_string(),
            dry_run: true,
            accepted: true,
            request_id: None,
            status: Some("dry_run".to_string()),
            slug: request.slug.clone(),
            version: request.version.clone(),
            warnings,
            errors: Vec::new(),
            next_step: Some(
                "Dry-run preview only. Re-run with dry_run=false, a non-empty reason, and a supported reason_code to yank the published release."
                    .to_string(),
            ),
            moderation_policy: None,
        }),
    ))
}

/// POST /v2/catalog/owner-transfer - Registry owner transfer governance contract
#[utoipa::path(
    post,
    path = "/v2/catalog/owner-transfer",
    tag = "marketplace",
    request_body = RegistryOwnerTransferRequest,
    responses(
        (
            status = 200,
            description = "Registry owner transfer request accepted and normalized",
            body = RegistryMutationResponse
        ),
        (
            status = 400,
            description = "Owner transfer request failed local contract or governance validation"
        ),
        (
            status = 404,
            description = "Registry owner binding was not found"
        )
    )
)]
async fn transfer_owner(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(request): Json<RegistryOwnerTransferRequest>,
) -> Result<impl IntoResponse, Error> {
    validate_registry_mutation_schema_version(request.schema_version)
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let warnings = validate_owner_transfer_request(&request)?;

    if !request.dry_run {
        let reason = request
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::BadRequest(
                    "Live registry owner transfer requires a non-empty reason for the governance audit trail"
                        .to_string(),
                )
            })?;
        let reason_code = request
            .reason_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::BadRequest(
                    "Live registry owner transfer requires a non-empty reason_code for the policy audit trail"
                        .to_string(),
                )
            })?;
        let actor = request_actor_from_headers(&headers);
        let binding = RegistryGovernanceService::new(ctx.db.clone())
            .transfer_registry_slug_owner(
                &request.slug,
                &request.new_owner_actor,
                reason,
                reason_code,
                &actor,
            )
            .await
            .map_err(map_registry_governance_error)?;

        return Ok((
            StatusCode::OK,
            Json(RegistryMutationResponse {
                schema_version:
                    crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
                action: "owner_transfer".to_string(),
                dry_run: false,
                accepted: true,
                request_id: None,
                status: Some("owner_transferred".to_string()),
                slug: binding.slug,
                version: String::new(),
                warnings,
                errors: Vec::new(),
                next_step: None,
                moderation_policy: None,
            }),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(RegistryMutationResponse {
            schema_version: crate::services::marketplace_catalog::REGISTRY_MUTATION_SCHEMA_VERSION,
            action: "owner_transfer".to_string(),
            dry_run: true,
            accepted: true,
            request_id: None,
            status: Some("dry_run".to_string()),
            slug: request.slug.clone(),
            version: String::new(),
            warnings,
            errors: Vec::new(),
            next_step: Some(
                format!(
                    "Dry-run preview only. Re-run with dry_run=false, a non-empty reason, and a supported reason_code ({}) to transfer the persisted owner binding.",
                    REGISTRY_OWNER_TRANSFER_REASON_CODES.join(", ")
                ),
            ),
            moderation_policy: None,
        }),
    ))
}

pub fn routes() -> Routes {
    read_only_routes()
        .add(registry_publish_path(), post(publish))
        .add(registry_publish_status_path(), get(publish_status))
        .add(
            registry_publish_artifact_path(),
            put(upload_publish_artifact),
        )
        .add(
            registry_publish_validate_path(),
            post(validate_publish_request_step),
        )
        .add(
            registry_publish_stage_report_path(),
            post(report_validation_stage),
        )
        .add(
            registry_publish_approve_path(),
            post(approve_publish_request),
        )
        .add(registry_publish_reject_path(), post(reject_publish_request))
        .add(registry_owner_transfer_path(), post(transfer_owner))
        .add(registry_yank_path(), post(yank))
}

pub fn read_only_routes() -> Routes {
    Routes::new()
        .add(registry_catalog_path(), get(catalog))
        .add(legacy_registry_catalog_path(), get(catalog))
        .add(registry_catalog_module_path(), get(catalog_module))
        .add(legacy_registry_catalog_module_path(), get(catalog_module))
}

async fn first_party_catalog_modules(
    ctx: &AppContext,
) -> Result<Vec<CatalogManifestModule>, Error> {
    let manifest = ManifestManager::load().unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Failed to load modules manifest for registry catalog; falling back to builtin catalog"
        );
        ModulesManifest::default()
    });
    let modules = catalog_modules_with_builtin_fallback(&manifest)
        .map_err(|error| Error::Message(format!("Failed to build marketplace catalog: {error}")))?;

    let first_party_modules = modules
        .into_iter()
        .filter(|module| module.ownership == "first_party")
        .collect::<Vec<_>>();

    RegistryGovernanceService::new(ctx.db.clone())
        .apply_catalog_projection(first_party_modules)
        .await
        .map_err(|error| {
            Error::Message(format!(
                "Failed to project registry releases into catalog: {error}"
            ))
        })
}

fn filter_catalog_modules(
    modules: Vec<CatalogManifestModule>,
    params: &RegistryCatalogListParams,
) -> Vec<CatalogManifestModule> {
    let search = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let category = params
        .category
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let tag = params
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    modules
        .into_iter()
        .filter(|module| {
            search.is_none_or(|search| {
                let search = search.to_ascii_lowercase();
                module.slug.to_ascii_lowercase().contains(&search)
                    || module
                        .name
                        .as_deref()
                        .is_some_and(|name| name.to_ascii_lowercase().contains(&search))
                    || module.description.as_deref().is_some_and(|description| {
                        description.to_ascii_lowercase().contains(&search)
                    })
            })
        })
        .filter(|module| {
            category.is_none_or(|category| {
                module
                    .category
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(category))
            })
        })
        .filter(|module| {
            tag.is_none_or(|tag| {
                module
                    .tags
                    .iter()
                    .any(|value| value.eq_ignore_ascii_case(tag))
            })
        })
        .collect()
}

fn catalog_modules_with_builtin_fallback(
    manifest: &ModulesManifest,
) -> Result<Vec<CatalogManifestModule>, crate::modules::ManifestError> {
    match ManifestManager::catalog_modules(manifest) {
        Ok(modules) => Ok(modules),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Registry catalog generation fell back to builtin first-party module catalog"
            );
            ManifestManager::catalog_modules(&ModulesManifest::default())
        }
    }
}

fn sort_catalog_modules(mut modules: Vec<CatalogManifestModule>) -> Vec<CatalogManifestModule> {
    modules.sort_by(|left, right| {
        left.slug
            .cmp(&right.slug)
            .then_with(|| left.crate_name.cmp(&right.crate_name))
    });
    modules
}

fn paginate_catalog_modules(
    modules: Vec<CatalogManifestModule>,
    params: &RegistryCatalogListParams,
) -> (Vec<CatalogManifestModule>, usize) {
    let total_count = modules.len();
    let offset = params.offset.unwrap_or(0).min(total_count);
    let limit = params.limit.map(|value| value.min(100));

    let modules = modules
        .into_iter()
        .skip(offset)
        .take(limit.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();

    (modules, total_count)
}

fn build_registry_response<T>(
    headers: &HeaderMap,
    payload: &T,
    total_count: Option<usize>,
) -> Result<Response<Body>, Error>
where
    T: serde::Serialize,
{
    let etag = registry_etag(payload)?;
    let etag_header = HeaderValue::from_str(&etag)
        .map_err(|err| Error::Message(format!("Failed to build registry ETag header: {err}")))?;
    let total_count_header = total_count.map(registry_total_count_header).transpose()?;
    if request_matches_etag(headers, &etag) {
        let mut builder = Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(CACHE_CONTROL, registry_cache_control())
            .header(ETAG, etag_header.clone());
        if let Some(total_count_header) = total_count_header.as_ref() {
            builder = builder.header(registry_total_count_header_name(), total_count_header);
        }
        return builder.body(Body::empty()).map_err(|err| {
            Error::Message(format!("Failed to build registry 304 response: {err}"))
        });
    }

    let mut response = Json(payload).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, registry_cache_control());
    response.headers_mut().insert(ETAG, etag_header);
    if let Some(total_count_header) = total_count_header {
        response
            .headers_mut()
            .insert(registry_total_count_header_name(), total_count_header);
    }

    Ok(response)
}

fn registry_cache_control() -> HeaderValue {
    HeaderValue::from_static("public, max-age=60")
}

fn registry_total_count_header_name() -> HeaderName {
    HeaderName::from_static("x-total-count")
}

fn registry_total_count_header(total_count: usize) -> Result<HeaderValue, Error> {
    HeaderValue::from_str(&total_count.to_string()).map_err(|err| {
        Error::Message(format!(
            "Failed to build registry total-count header: {err}"
        ))
    })
}

fn registry_etag<T>(payload: &T) -> Result<String, Error>
where
    T: serde::Serialize,
{
    let body = serde_json::to_vec(payload)
        .map_err(|err| Error::Message(format!("Failed to serialize registry payload: {err}")))?;
    let hash = Sha256::digest(body);
    Ok(format!("\"{}\"", hex::encode(hash)))
}

fn request_matches_etag(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .any(|candidate| candidate == "*" || candidate == etag)
        })
        .unwrap_or(false)
}

fn validate_publish_request_payload(
    request: &RegistryPublishRequest,
) -> Result<Vec<String>, Error> {
    validate_registry_slug(&request.module.slug)?;
    validate_registry_version(&request.module.version)?;

    if request.module.crate_name.trim().is_empty() {
        return Err(Error::BadRequest(
            "Registry publish request must include module.crate_name".to_string(),
        ));
    }
    if request.module.name.trim().is_empty() {
        return Err(Error::BadRequest(
            "Registry publish request must include module.name".to_string(),
        ));
    }
    if request.module.description.trim().len() < 20 {
        return Err(Error::BadRequest(
            "Registry publish request requires module.description >= 20 characters".to_string(),
        ));
    }
    if request.module.license.trim().is_empty() {
        return Err(Error::BadRequest(
            "Registry publish request must include module.license".to_string(),
        ));
    }

    let mut warnings = Vec::new();
    if request.module.ui_packages.admin.is_none() && request.module.ui_packages.storefront.is_none()
    {
        warnings.push(
            "No publishable admin/storefront UI packages declared; only backend contract would be published."
                .to_string(),
        );
    }
    let moderation_policy = moderation_policy_snapshot(&request.module.ownership);
    if moderation_policy.manual_review_required {
        warnings.push(format!(
            "{}{}",
            moderation_policy.restriction_reason,
            moderation_policy
                .restriction_reason_code
                .as_deref()
                .map(|code| format!(" ({code})"))
                .unwrap_or_default()
        ));
    }

    Ok(warnings)
}

fn validate_yank_request(request: &RegistryYankRequest) -> Result<Vec<String>, Error> {
    validate_registry_slug(&request.slug)?;
    validate_registry_version(&request.version)?;

    let mut warnings = Vec::new();
    if request
        .reason
        .as_deref()
        .map(str::trim)
        .is_none_or(|reason| reason.is_empty())
    {
        warnings.push(
            "No yank reason supplied; live yank requires a non-empty reason for the governance audit trail."
                .to_string(),
        );
    }
    if request
        .reason_code
        .as_deref()
        .map(str::trim)
        .is_none_or(|value| value.is_empty())
    {
        warnings.push(
            format!(
                "No yank reason_code supplied; live yank requires one of {} for the policy audit trail.",
                REGISTRY_YANK_REASON_CODES.join(", ")
            ),
        );
    } else if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !REGISTRY_YANK_REASON_CODES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry yank reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_YANK_REASON_CODES.join(", ")
            )));
        }
    }

    Ok(warnings)
}

fn validate_publish_reject_request(
    request: &RegistryPublishDecisionRequest,
) -> Result<Vec<String>, Error> {
    let mut warnings = Vec::new();
    if request
        .reason
        .as_deref()
        .map(str::trim)
        .is_none_or(|reason| reason.is_empty())
    {
        warnings.push(
            "No reject reason supplied; live reject requires a non-empty reason for the governance audit trail."
                .to_string(),
        );
    }
    if request
        .reason_code
        .as_deref()
        .map(str::trim)
        .is_none_or(|value| value.is_empty())
    {
        warnings.push(format!(
            "No reject reason_code supplied; live reject requires one of {} for the policy audit trail.",
            REGISTRY_REJECT_REASON_CODES.join(", ")
        ));
    } else if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !REGISTRY_REJECT_REASON_CODES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry publish reject reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_REJECT_REASON_CODES.join(", ")
            )));
        }
    }

    Ok(warnings)
}

fn validate_publish_approve_request(request: &RegistryPublishDecisionRequest) -> Result<(), Error> {
    if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !reason_code.is_empty()
            && !REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry publish approval override reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
            )));
        }
    }

    Ok(())
}

fn validate_validation_stage_report_request(
    request: &RegistryValidationStageReportRequest,
) -> Result<(), Error> {
    let stage = request.stage.trim();
    if stage.is_empty() {
        return Err(Error::BadRequest(
            "Registry validation stage report must include a non-empty stage".to_string(),
        ));
    }

    let status = request.status.trim().to_ascii_lowercase();
    let allowed = ["queued", "running", "passed", "failed", "blocked"];
    if !allowed.iter().any(|candidate| *candidate == status) {
        return Err(Error::BadRequest(format!(
            "Registry validation stage report status '{}' is not supported; expected one of {}",
            request.status.trim(),
            allowed.join(", ")
        )));
    }

    if request.requeue && status != "queued" {
        return Err(Error::BadRequest(
            "Registry validation stage requeue requires status='queued'".to_string(),
        ));
    }
    if !request.dry_run
        && matches!(status.as_str(), "passed" | "failed" | "blocked")
        && request
            .reason_code
            .as_deref()
            .map(str::trim)
            .is_none_or(|value| value.is_empty())
    {
        return Err(Error::BadRequest(format!(
            "Live registry validation stage status '{}' requires reason_code; expected one of {}",
            status,
            validation_stage_allowed_terminal_reason_codes(stage).join(", ")
        )));
    }
    if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !reason_code.is_empty()
            && !REGISTRY_VALIDATION_STAGE_REASON_CODES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry validation stage reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_VALIDATION_STAGE_REASON_CODES.join(", ")
            )));
        }
        if matches!(status.as_str(), "passed" | "failed" | "blocked")
            && !validation_stage_allowed_terminal_reason_codes(stage)
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry validation stage '{}' reason_code '{}' is not supported for terminal status '{}'; expected one of {}",
                stage,
                reason_code,
                status,
                validation_stage_allowed_terminal_reason_codes(stage).join(", ")
            )));
        }
    }

    Ok(())
}

fn validate_owner_transfer_request(
    request: &RegistryOwnerTransferRequest,
) -> Result<Vec<String>, Error> {
    validate_registry_slug(&request.slug)?;

    let new_owner_actor = request.new_owner_actor.trim();
    if new_owner_actor.is_empty() {
        return Err(Error::BadRequest(
            "Registry owner transfer must include a non-empty new_owner_actor".to_string(),
        ));
    }

    let mut warnings = Vec::new();
    if request
        .reason
        .as_deref()
        .map(str::trim)
        .is_none_or(|reason| reason.is_empty())
    {
        warnings.push(
            "No transfer reason supplied; live owner transfer requires a non-empty reason for the governance audit trail."
                .to_string(),
        );
    }
    if request
        .reason_code
        .as_deref()
        .map(str::trim)
        .is_none_or(|value| value.is_empty())
    {
        warnings.push(format!(
            "No transfer reason_code supplied; live owner transfer requires one of {} for the policy audit trail.",
            REGISTRY_OWNER_TRANSFER_REASON_CODES.join(", ")
        ));
    } else if let Some(reason_code) = request.reason_code.as_deref().map(str::trim) {
        if !REGISTRY_OWNER_TRANSFER_REASON_CODES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(reason_code))
        {
            return Err(Error::BadRequest(format!(
                "Registry owner transfer reason_code '{}' is not supported; expected one of {}",
                reason_code,
                REGISTRY_OWNER_TRANSFER_REASON_CODES.join(", ")
            )));
        }
    }

    Ok(warnings)
}

fn deserialize_message_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect()
}

fn request_actor_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("x-rustok-actor")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("anonymous")
        .to_string()
}

fn request_publisher_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-rustok-publisher")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn publish_request_accepted(
    status: &crate::models::registry_publish_request::RegistryPublishRequestStatus,
) -> bool {
    !matches!(
        status,
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Rejected
    )
}

fn publish_request_next_step(
    status: &crate::models::registry_publish_request::RegistryPublishRequestStatus,
    request_id: &str,
) -> Option<String> {
    match status {
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Draft => {
            Some(registry_publish_artifact_path().replace("{request_id}", request_id))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::ArtifactUploaded
        | crate::models::registry_publish_request::RegistryPublishRequestStatus::Submitted => {
            Some(format!(
                "Trigger artifact validation via POST {}",
                registry_publish_validate_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Validating => {
            Some(format!(
                "Poll {} for the latest publish lifecycle status.",
                registry_publish_status_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved => {
            Some(format!(
                "Finalize the validated publish request via POST {}",
                registry_publish_approve_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Rejected => {
            Some(format!(
                "If the request was rejected by automated validation, fix the artifact and retry via POST {}; otherwise create a new publish request after governance review resolves the rejection.",
                registry_publish_validate_path().replace("{request_id}", request_id)
            ))
        }
        crate::models::registry_publish_request::RegistryPublishRequestStatus::Published => None,
    }
}

fn publish_request_status_next_step(
    request: &crate::models::registry_publish_request::Model,
    request_id: &str,
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Option<String> {
    if request.status
        == crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved
        && validation_stages
            .iter()
            .any(|stage| !stage.status.eq_ignore_ascii_case("passed"))
    {
        return Some(approval_override_next_step(request_id, validation_stages));
    }

    publish_request_next_step(&request.status, request_id)
}

fn approval_override_next_step(
    request_id: &str,
    validation_stages: &[RegistryValidationStageSnapshot],
) -> String {
    format!(
        "Mark the remaining follow-up stages as passed via POST {} or approve with an explicit override reason plus reason_code ({}). Pending stages: {}.",
        registry_publish_stage_report_path().replace("{request_id}", request_id),
        REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", "),
        approval_override_stage_labels(validation_stages).join(", ")
    )
}

fn approval_override_warning_message(
    validation_stages: &[RegistryValidationStageSnapshot],
) -> String {
    format!(
        "Approval override is required because these follow-up validation stages are not passed yet: {}. Live approve must include both reason and reason_code ({}).",
        approval_override_stage_labels(validation_stages).join(", "),
        REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join(", ")
    )
}

fn approval_override_stage_labels(
    validation_stages: &[RegistryValidationStageSnapshot],
) -> Vec<String> {
    validation_stages
        .iter()
        .filter(|stage| !stage.status.eq_ignore_ascii_case("passed"))
        .map(|stage| format!("{} ({})", stage.key, stage.status.to_ascii_lowercase()))
        .collect()
}

fn publish_status_follow_up_gate(
    gate: RegistryFollowUpGateSnapshot,
) -> RegistryPublishStatusFollowUpGate {
    RegistryPublishStatusFollowUpGate {
        key: gate.key,
        status: gate.status,
        detail: gate.detail,
        updated_at: gate.updated_at,
    }
}

fn publish_status_validation_stage(
    stage: &RegistryValidationStageSnapshot,
) -> RegistryPublishStatusValidationStage {
    RegistryPublishStatusValidationStage {
        key: stage.key.clone(),
        status: stage.status.clone(),
        detail: stage.detail.clone(),
        attempt_number: stage.attempt_number,
        updated_at: stage.updated_at.clone(),
        started_at: stage.started_at.clone(),
        finished_at: stage.finished_at.clone(),
        execution_mode: stage.execution_mode.clone(),
        runnable: stage.runnable,
        requires_manual_confirmation: stage.requires_manual_confirmation,
        allowed_terminal_reason_codes: stage.allowed_terminal_reason_codes.clone(),
        suggested_pass_reason_code: stage.suggested_pass_reason_code.clone(),
        suggested_failure_reason_code: stage.suggested_failure_reason_code.clone(),
        suggested_blocked_reason_code: stage.suggested_blocked_reason_code.clone(),
    }
}

fn registry_moderation_policy_response(ownership: &str) -> RegistryModerationPolicy {
    let snapshot = moderation_policy_snapshot(ownership);
    RegistryModerationPolicy {
        mode: snapshot.mode,
        live_publish_supported: snapshot.live_publish_supported,
        live_governance_supported: snapshot.live_governance_supported,
        manual_review_required: snapshot.manual_review_required,
        restriction_reason_code: snapshot.restriction_reason_code,
        restriction_reason: snapshot.restriction_reason,
    }
}

fn registry_governance_action_response(
    action: crate::services::registry_governance::RegistryGovernanceActionSnapshot,
) -> RegistryGovernanceAction {
    RegistryGovernanceAction {
        key: action.key,
        enabled: action.enabled,
        reason: action.reason,
        supported_reason_codes: action.supported_reason_codes,
    }
}

fn map_registry_governance_error(error: anyhow::Error) -> Error {
    let message = error.to_string();
    if message.contains("was not found") {
        Error::NotFound
    } else {
        Error::BadRequest(message)
    }
}

fn validate_registry_slug(slug: &str) -> Result<(), Error> {
    let slug = slug.trim();
    if slug.is_empty() {
        return Err(Error::BadRequest(
            "Registry mutation request must include a non-empty slug".to_string(),
        ));
    }
    if !slug
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err(Error::BadRequest(format!(
            "Registry mutation slug '{slug}' may contain only lowercase ASCII letters, digits, and hyphen"
        )));
    }
    Ok(())
}

fn validate_registry_version(version: &str) -> Result<(), Error> {
    Version::parse(version.trim()).map_err(|error| {
        Error::BadRequest(format!(
            "Registry mutation version '{version}' is not valid semver: {error}"
        ))
    })?;
    Ok(())
}
