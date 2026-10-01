//! Sandbox capability broker for artifact data objects.

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use bytes::Bytes;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityResponse, SandboxError, SandboxResult,
    SandboxSubject,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::capabilities::*;
use super::constants::*;
use super::error::*;
use super::objects::*;
use super::traits::*;
use super::types::*;
use super::upload::*;
use super::validation::*;
use super::*;

/// The `platform.data.objects` adapter for bounded binary object calls. It is
/// deliberately a distinct capability from structured `platform.data`, so
/// policies can grant object prefixes and operations without broadening JSON
/// value access.
#[derive(Clone)]
pub struct SeaOrmArtifactDataObjectCapabilityBroker<A> {
    objects: SeaOrmArtifactDataObjectBroker<A>,
    uploads: SeaOrmArtifactDataObjectUploadService<A>,
    scope: ArtifactDataScope,
}

impl<A> SeaOrmArtifactDataObjectCapabilityBroker<A>
where
    A: ArtifactDataAuthorizer + Clone,
{
    pub fn new(
        db: DatabaseConnection,
        storage: StorageRuntime,
        authorizer: A,
        scope: ArtifactDataScope,
    ) -> Self {
        Self::with_infrastructure(
            db,
            storage,
            authorizer,
            scope,
            ControlPlaneInfrastructure::default(),
        )
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        storage: StorageRuntime,
        authorizer: A,
        scope: ArtifactDataScope,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self::with_infrastructure_and_quota(
            db,
            storage,
            authorizer,
            scope,
            infrastructure,
            ArtifactDataQuota::default(),
        )
    }

    pub fn with_infrastructure_and_quota(
        db: DatabaseConnection,
        storage: StorageRuntime,
        authorizer: A,
        scope: ArtifactDataScope,
        infrastructure: ControlPlaneInfrastructure,
        quota: ArtifactDataQuota,
    ) -> Self {
        Self {
            objects: SeaOrmArtifactDataObjectBroker::with_infrastructure_and_quota(
                db.clone(),
                storage.clone(),
                authorizer.clone(),
                infrastructure.clone(),
                quota,
            ),
            uploads: SeaOrmArtifactDataObjectUploadService::with_infrastructure_and_quota(
                db,
                storage,
                authorizer,
                infrastructure,
                quota,
            ),
            scope,
        }
    }
}

#[async_trait]
impl<A> CapabilityBroker for SeaOrmArtifactDataObjectCapabilityBroker<A>
where
    A: ArtifactDataAuthorizer + Clone,
{
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if call.capability.as_str() != "platform.data.objects"
            || call.context.tenant_id != Some(self.scope.tenant_id)
            || !matches!(
                &call.subject,
                SandboxSubject::ModuleArtifact { slug, .. } if slug == &self.scope.module_slug
            )
        {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        match decode_object_data_capability_call(call)? {
            ObjectDataCapabilityCall::GetMetadata { name } => {
                let object = self
                    .objects
                    .get_object(&self.scope, &name)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "object": object }),
                })
            }
            ObjectDataCapabilityCall::Read { name } => {
                let object = self
                    .objects
                    .read_object(&self.scope, &name)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                let output = match object {
                    Some(content) if content.data.len() <= MAX_SANDBOX_ARTIFACT_OBJECT_BYTES => {
                        json!({
                            "object": content.object,
                            "data_base64": BASE64_STANDARD.encode(content.data),
                        })
                    }
                    Some(_) => {
                        return Err(data_capability_constraint(
                            call,
                            "object exceeds the sandbox transfer limit",
                        ));
                    }
                    None => json!({ "object": Value::Null }),
                };
                Ok(CapabilityResponse { output })
            }
            ObjectDataCapabilityCall::Put { upload } => {
                let object = self
                    .objects
                    .put_object(&self.scope, upload)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "object": object }),
                })
            }
            ObjectDataCapabilityCall::Delete { request } => {
                let result = self
                    .objects
                    .delete_object(&self.scope, request)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "deletion": result }),
                })
            }
            ObjectDataCapabilityCall::BeginUpload { request } => {
                let session = self
                    .uploads
                    .begin(&self.scope, request)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "session": session }),
                })
            }
            ObjectDataCapabilityCall::AppendChunk { chunk } => {
                self.uploads
                    .append_chunk(&self.scope, chunk)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse { output: json!({}) })
            }
            ObjectDataCapabilityCall::CompleteUpload { request } => {
                let object = self
                    .uploads
                    .complete(&self.scope, request)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "object": object }),
                })
            }
            ObjectDataCapabilityCall::List { page } => {
                let page = self
                    .objects
                    .list_objects(&self.scope, page)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({
                        "objects": page.objects,
                        "next_after_name": page.next_after_name,
                    }),
                })
            }
        }
    }
}

/// Production resolver for the bounded `platform.data.objects` owner. The
/// storage service is deployment-provided, but artifact scope and every
/// physical object key remain owner-controlled.
#[derive(Clone)]
pub struct SeaOrmArtifactDataObjectCapabilityBrokerResolver {
    db: DatabaseConnection,
    storage: StorageRuntime,
    infrastructure: ControlPlaneInfrastructure,
    quota_policy: Arc<dyn ArtifactDataQuotaPolicy>,
}

impl SeaOrmArtifactDataObjectCapabilityBrokerResolver {
    pub fn new(db: DatabaseConnection, storage: StorageRuntime) -> Self {
        Self::with_infrastructure_and_quota_policy(
            db,
            storage,
            ControlPlaneInfrastructure::default(),
            Arc::new(FixedArtifactDataQuotaPolicy::default()),
        )
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        storage: StorageRuntime,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self::with_infrastructure_and_quota_policy(
            db,
            storage,
            infrastructure,
            Arc::new(FixedArtifactDataQuotaPolicy::default()),
        )
    }

    pub fn with_infrastructure_and_quota_policy(
        db: DatabaseConnection,
        storage: StorageRuntime,
        infrastructure: ControlPlaneInfrastructure,
        quota_policy: Arc<dyn ArtifactDataQuotaPolicy>,
    ) -> Self {
        Self {
            db,
            storage,
            infrastructure,
            quota_policy,
        }
    }
}

#[async_trait]
impl ArtifactCapabilityBrokerResolver for SeaOrmArtifactDataObjectCapabilityBrokerResolver {
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &rustok_sandbox::CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        if capability.as_str() != "platform.data.objects" {
            return Err(SandboxError::CapabilityDenied(capability.clone()));
        }
        let installation =
            resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        let scope = artifact_data_scope_for_execution(&installation, execution, capability)?;
        let quota = self
            .quota_policy
            .quota_for(&scope)
            .await
            .map_err(|error| data_capability_error(capability, error))?;
        let authorizer = ExactArtifactDataAuthorizer {
            scope: scope.clone(),
        };
        Ok(Arc::new(
            SeaOrmArtifactDataObjectCapabilityBroker::with_infrastructure_and_quota(
                self.db.clone(),
                self.storage.clone(),
                authorizer,
                scope,
                self.infrastructure.clone(),
                quota,
            ),
        ))
    }
}

pub(crate) enum ObjectDataCapabilityCall {
    GetMetadata {
        name: String,
    },
    Read {
        name: String,
    },
    Put {
        upload: ArtifactDataObjectUpload,
    },
    Delete {
        request: ArtifactDataObjectDeleteRequest,
    },
    BeginUpload {
        request: ArtifactDataObjectUploadSessionRequest,
    },
    AppendChunk {
        chunk: ArtifactDataObjectUploadChunk,
    },
    CompleteUpload {
        request: ArtifactDataObjectUploadCompleteRequest,
    },
    List {
        page: ArtifactDataPageRequest,
    },
}

pub(crate) fn decode_object_data_capability_call(
    call: &CapabilityCall,
) -> SandboxResult<ObjectDataCapabilityCall> {
    let input = call
        .input
        .as_object()
        .ok_or_else(|| data_capability_constraint(call, "object-data input must be an object"))?;
    match call.operation.as_str() {
        "get_metadata" => {
            reject_data_capability_fields(call, input, &["name"])?;
            Ok(ObjectDataCapabilityCall::GetMetadata {
                name: required_data_capability_string(call, input, "name")?.to_string(),
            })
        }
        "read" => {
            reject_data_capability_fields(call, input, &["name"])?;
            Ok(ObjectDataCapabilityCall::Read {
                name: required_data_capability_string(call, input, "name")?.to_string(),
            })
        }
        "put" => {
            reject_data_capability_fields(
                call,
                input,
                &[
                    "name",
                    "content_type",
                    "data_base64",
                    "expected_revision",
                    "idempotency_key",
                ],
            )?;
            let data = BASE64_STANDARD
                .decode(required_data_capability_string(call, input, "data_base64")?)
                .map_err(|_| data_capability_constraint(call, "object data_base64 is invalid"))?;
            if data.is_empty() || data.len() > MAX_SANDBOX_ARTIFACT_OBJECT_BYTES {
                return Err(data_capability_constraint(
                    call,
                    "object exceeds the sandbox transfer limit",
                ));
            }
            let expected_revision = input
                .get("expected_revision")
                .map(|value| {
                    value
                        .as_u64()
                        .filter(|revision| *revision > 0)
                        .ok_or_else(|| {
                            data_capability_constraint(
                                call,
                                "object expected_revision must be a positive integer",
                            )
                        })
                })
                .transpose()?;
            let idempotency_key = Uuid::parse_str(required_data_capability_string(
                call,
                input,
                "idempotency_key",
            )?)
            .map_err(|_| {
                data_capability_constraint(call, "object idempotency_key must be a UUID")
            })?;
            Ok(ObjectDataCapabilityCall::Put {
                upload: ArtifactDataObjectUpload {
                    name: required_data_capability_string(call, input, "name")?.to_string(),
                    content_type: required_data_capability_string(call, input, "content_type")?
                        .to_string(),
                    data: Bytes::from(data),
                    expected_revision,
                    idempotency_key,
                },
            })
        }
        "delete" => {
            reject_data_capability_fields(
                call,
                input,
                &["name", "expected_revision", "idempotency_key"],
            )?;
            let expected_revision = input
                .get("expected_revision")
                .and_then(Value::as_u64)
                .filter(|revision| *revision > 0)
                .ok_or_else(|| {
                    data_capability_constraint(
                        call,
                        "object expected_revision must be a positive integer",
                    )
                })?;
            let idempotency_key = Uuid::parse_str(required_data_capability_string(
                call,
                input,
                "idempotency_key",
            )?)
            .map_err(|_| {
                data_capability_constraint(call, "object idempotency_key must be a UUID")
            })?;
            Ok(ObjectDataCapabilityCall::Delete {
                request: ArtifactDataObjectDeleteRequest {
                    name: required_data_capability_string(call, input, "name")?.to_string(),
                    expected_revision,
                    idempotency_key,
                },
            })
        }
        "begin_upload" => {
            reject_data_capability_fields(
                call,
                input,
                &[
                    "name",
                    "content_type",
                    "expected_revision",
                    "idempotency_key",
                ],
            )?;
            let expected_revision = input
                .get("expected_revision")
                .map(|value| {
                    value
                        .as_u64()
                        .filter(|revision| *revision > 0)
                        .ok_or_else(|| {
                            data_capability_constraint(
                                call,
                                "object expected_revision must be a positive integer",
                            )
                        })
                })
                .transpose()?;
            let idempotency_key = Uuid::parse_str(required_data_capability_string(
                call,
                input,
                "idempotency_key",
            )?)
            .map_err(|_| {
                data_capability_constraint(call, "object idempotency_key must be a UUID")
            })?;
            Ok(ObjectDataCapabilityCall::BeginUpload {
                request: ArtifactDataObjectUploadSessionRequest {
                    name: required_data_capability_string(call, input, "name")?.to_owned(),
                    content_type: required_data_capability_string(call, input, "content_type")?
                        .to_owned(),
                    expected_revision,
                    idempotency_key,
                },
            })
        }
        "append_chunk" => {
            reject_data_capability_fields(call, input, &["session_id", "sequence", "data_base64"])?;
            let data = BASE64_STANDARD
                .decode(required_data_capability_string(call, input, "data_base64")?)
                .map_err(|_| {
                    data_capability_constraint(call, "object chunk data_base64 is invalid")
                })?;
            if data.is_empty() || data.len() > MAX_SANDBOX_ARTIFACT_OBJECT_BYTES {
                return Err(data_capability_constraint(
                    call,
                    "object chunk exceeds the sandbox transfer limit",
                ));
            }
            let session_id =
                Uuid::parse_str(required_data_capability_string(call, input, "session_id")?)
                    .map_err(|_| {
                        data_capability_constraint(call, "object session_id must be a UUID")
                    })?;
            let sequence = input
                .get("sequence")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    data_capability_constraint(call, "object sequence must be an integer")
                })?;
            Ok(ObjectDataCapabilityCall::AppendChunk {
                chunk: ArtifactDataObjectUploadChunk {
                    session_id,
                    sequence,
                    data: Bytes::from(data),
                },
            })
        }
        "complete_upload" => {
            reject_data_capability_fields(
                call,
                input,
                &["session_id", "size_bytes", "digest_sha256"],
            )?;
            let session_id =
                Uuid::parse_str(required_data_capability_string(call, input, "session_id")?)
                    .map_err(|_| {
                        data_capability_constraint(call, "object session_id must be a UUID")
                    })?;
            let size_bytes = input
                .get("size_bytes")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    data_capability_constraint(call, "object size_bytes must be an integer")
                })?;
            Ok(ObjectDataCapabilityCall::CompleteUpload {
                request: ArtifactDataObjectUploadCompleteRequest {
                    session_id,
                    size_bytes,
                    digest_sha256: required_data_capability_string(call, input, "digest_sha256")?
                        .to_owned(),
                },
            })
        }
        "list" => {
            reject_data_capability_fields(call, input, &["prefix", "after_name", "limit"])?;
            let after_key = input
                .get("after_name")
                .map(|value| {
                    value.as_str().map(str::to_string).ok_or_else(|| {
                        data_capability_constraint(call, "object after_name must be a string")
                    })
                })
                .transpose()?;
            let limit = input
                .get("limit")
                .and_then(Value::as_u64)
                .filter(|limit| (1..=100).contains(limit))
                .ok_or_else(|| {
                    data_capability_constraint(call, "object list limit must be between 1 and 100")
                })?;
            let page = ArtifactDataPageRequest {
                prefix: required_data_capability_string(call, input, "prefix")?.to_string(),
                after_key,
                limit: u32::try_from(limit).map_err(|_| {
                    data_capability_constraint(call, "object list limit must fit u32")
                })?,
            };
            validate_page_request(&page)
                .map_err(|_| data_capability_constraint(call, "object list page is invalid"))?;
            Ok(ObjectDataCapabilityCall::List { page })
        }
        _ => Err(data_capability_constraint(
            call,
            "object-data operation is unsupported",
        )),
    }
}
