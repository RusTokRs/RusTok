use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{SandboxError, SandboxExecutorKind, SandboxRequest, SandboxResult, SandboxSubject};

/// Deployment-scoped limits applied before an execution enters an executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxAdmissionLimits {
    pub global: u32,
    pub per_executor: u32,
    pub per_tenant: u32,
    pub per_artifact: u32,
}

impl Default for SandboxAdmissionLimits {
    fn default() -> Self {
        Self {
            global: 64,
            per_executor: 32,
            per_tenant: 16,
            per_artifact: 8,
        }
    }
}

#[derive(Clone)]
pub(crate) struct AdmissionController(Arc<AdmissionState>);

struct AdmissionState {
    limits: SandboxAdmissionLimits,
    counts: Mutex<AdmissionCounts>,
}

#[derive(Default)]
struct AdmissionCounts {
    global: u32,
    executors: HashMap<SandboxExecutorKind, u32>,
    tenants: HashMap<Uuid, u32>,
    artifacts: HashMap<String, u32>,
}

pub(crate) struct AdmissionPermit {
    state: Arc<AdmissionState>,
    executor: SandboxExecutorKind,
    tenant: Option<Uuid>,
    artifact: Option<String>,
}

impl AdmissionController {
    pub(crate) fn new(limits: SandboxAdmissionLimits) -> Self {
        Self(Arc::new(AdmissionState {
            limits,
            counts: Mutex::new(AdmissionCounts::default()),
        }))
    }

    pub(crate) fn admit(&self, request: &SandboxRequest) -> SandboxResult<AdmissionPermit> {
        let executor = request.payload.executor;
        let tenant = request.context.tenant_id;
        let artifact = match &request.subject {
            SandboxSubject::ModuleArtifact { digest, .. } => Some(digest.clone()),
            SandboxSubject::AlloyDraft { .. } => None,
        };
        let mut counts = self.0.counts.lock().map_err(|_| {
            SandboxError::Internal("sandbox admission state lock is poisoned".to_string())
        })?;
        check_limit(counts.global, self.0.limits.global, "concurrency_global")?;
        check_limit(
            *counts.executors.get(&executor).unwrap_or(&0),
            self.0.limits.per_executor,
            "concurrency_executor",
        )?;
        if let Some(tenant) = tenant {
            check_limit(
                *counts.tenants.get(&tenant).unwrap_or(&0),
                self.0.limits.per_tenant,
                "concurrency_tenant",
            )?;
        }
        if let Some(digest) = &artifact {
            check_limit(
                *counts.artifacts.get(digest).unwrap_or(&0),
                self.0.limits.per_artifact,
                "concurrency_artifact",
            )?;
        }
        counts.global += 1;
        *counts.executors.entry(executor).or_default() += 1;
        if let Some(tenant) = tenant {
            *counts.tenants.entry(tenant).or_default() += 1;
        }
        if let Some(digest) = &artifact {
            *counts.artifacts.entry(digest.clone()).or_default() += 1;
        }
        Ok(AdmissionPermit {
            state: Arc::clone(&self.0),
            executor,
            tenant,
            artifact,
        })
    }
}

impl Drop for AdmissionPermit {
    fn drop(&mut self) {
        let Ok(mut counts) = self.state.counts.lock() else {
            return;
        };
        counts.global = counts.global.saturating_sub(1);
        decrement(&mut counts.executors, &self.executor);
        if let Some(tenant) = self.tenant {
            decrement(&mut counts.tenants, &tenant);
        }
        if let Some(artifact) = &self.artifact {
            decrement(&mut counts.artifacts, artifact);
        }
    }
}

fn check_limit(current: u32, limit: u32, resource: &str) -> SandboxResult<()> {
    if current >= limit {
        return Err(SandboxError::LimitExceeded {
            resource: resource.to_string(),
            limit: limit.into(),
        });
    }
    Ok(())
}

fn decrement<Key>(counts: &mut HashMap<Key, u32>, key: &Key)
where
    Key: std::cmp::Eq + std::hash::Hash,
{
    let remove = if let Some(count) = counts.get_mut(key) {
        *count = count.saturating_sub(1);
        *count == 0
    } else {
        false
    };
    if remove {
        counts.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExecutionPhase, SandboxContext, SandboxPayload, SandboxPolicy};

    fn request() -> SandboxRequest {
        SandboxRequest {
            subject: SandboxSubject::ModuleArtifact {
                slug: "sample".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: SandboxContext::new(ExecutionPhase::Test),
            payload: SandboxPayload {
                executor: SandboxExecutorKind::Rhai,
                media_type: "application/test".to_string(),
                digest: "sha256:sample".to_string(),
                entrypoint: "main".to_string(),
                bytes: Vec::new(),
            },
            input: serde_json::Value::Null,
            policy: SandboxPolicy::default(),
        }
    }

    fn request_with(tenant_id: Option<Uuid>, digest: &str) -> SandboxRequest {
        let mut request = request();
        request.context.tenant_id = tenant_id;
        request.subject = SandboxSubject::ModuleArtifact {
            slug: "sample".to_string(),
            version: "1.0.0".to_string(),
            digest: digest.to_string(),
        };
        request.payload.digest = digest.to_string();
        request
    }

    #[test]
    fn admission_rejects_the_artifact_scope_and_releases_on_drop() {
        let controller = AdmissionController::new(SandboxAdmissionLimits {
            global: 2,
            per_executor: 2,
            per_tenant: 2,
            per_artifact: 1,
        });
        let permit = controller.admit(&request()).expect("first admission");
        let error = match controller.admit(&request()) {
            Err(error) => error,
            Ok(_) => panic!("artifact limit must reject the second admission"),
        };
        assert_eq!(
            error,
            SandboxError::LimitExceeded {
                resource: "concurrency_artifact".to_string(),
                limit: 1,
            }
        );
        drop(permit);
        controller.admit(&request()).expect("released admission");
    }

    #[test]
    fn admission_enforces_global_executor_and_tenant_scopes() {
        let tenant = Uuid::new_v4();
        let global = AdmissionController::new(SandboxAdmissionLimits {
            global: 1,
            per_executor: 2,
            per_tenant: 2,
            per_artifact: 2,
        });
        let _permit = global
            .admit(&request_with(None, "sha256:first"))
            .expect("first global admission");
        assert_limit(
            global.admit(&request_with(None, "sha256:second")),
            "concurrency_global",
        );

        let executor = AdmissionController::new(SandboxAdmissionLimits {
            global: 2,
            per_executor: 1,
            per_tenant: 2,
            per_artifact: 2,
        });
        let _permit = executor
            .admit(&request_with(None, "sha256:first"))
            .expect("first executor admission");
        assert_limit(
            executor.admit(&request_with(None, "sha256:second")),
            "concurrency_executor",
        );

        let tenant_controller = AdmissionController::new(SandboxAdmissionLimits {
            global: 2,
            per_executor: 2,
            per_tenant: 1,
            per_artifact: 2,
        });
        let _permit = tenant_controller
            .admit(&request_with(Some(tenant), "sha256:first"))
            .expect("first tenant admission");
        assert_limit(
            tenant_controller.admit(&request_with(Some(tenant), "sha256:second")),
            "concurrency_tenant",
        );
    }

    fn assert_limit(result: SandboxResult<AdmissionPermit>, resource: &str) {
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("admission limit must reject the second request"),
        };
        assert_eq!(
            error,
            SandboxError::LimitExceeded {
                resource: resource.to_string(),
                limit: 1,
            }
        );
    }
}
