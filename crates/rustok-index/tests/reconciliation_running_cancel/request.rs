use std::time::Duration;

use rustok_index::IndexReconciliationRunRequest;
use uuid::Uuid;

use super::schema::schema_ref;

pub fn request(tenant_id: Uuid, worker_id: &str) -> IndexReconciliationRunRequest {
    IndexReconciliationRunRequest::new(
        tenant_id,
        schema_ref(),
        worker_id,
        1,
        1,
        1,
        1,
        Duration::from_secs(60),
    )
    .expect("fixture request must be valid")
}
