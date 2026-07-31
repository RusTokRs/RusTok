use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_index::{
    EntityKey, FieldName, IndexMutation, IndexRecord, IndexSource, IndexSourceCursor,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue,
};
use serde_json::json;
use uuid::Uuid;

use super::schema::schema_ref;

#[derive(Clone, Copy)]
pub enum FailedProgressSource {
    FailSecondPage,
    RecoverSecondPage,
}

#[async_trait]
impl IndexSource for FailedProgressSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let offset = request
            .cursor()
            .and_then(|cursor| cursor.value().get("offset"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        match offset {
            0 => {
                let cursor = IndexSourceCursor::new(json!({ "offset": 1 }))
                    .expect("fixture cursor must be valid");
                IndexSourcePage::new(
                    &request,
                    vec![valid_mutation(request.tenant_id(), 601, 13_601)],
                    Some(cursor),
                )
                .map_err(|_| fixture_failure())
            }
            1 => {
                let mutation = match self {
                    Self::FailSecondPage => invalid_mutation(request.tenant_id(), 602, 13_602),
                    Self::RecoverSecondPage => valid_mutation(request.tenant_id(), 602, 13_602),
                };
                IndexSourcePage::new(&request, vec![mutation], None)
                    .map_err(|_| fixture_failure())
            }
            _ => Err(fixture_failure()),
        }
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}

fn valid_mutation(tenant_id: Uuid, entity: u128, event: u128) -> IndexMutation {
    let entity_id = Uuid::from_u128(entity);
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(event),
        record: IndexRecord {
            key: EntityKey {
                tenant_id,
                schema: schema_ref(),
                entity_id,
                locale: None,
            },
            source_version: 1,
            fields: BTreeMap::from([(
                FieldName::new("id").unwrap(),
                IndexValue::Uuid(entity_id),
            )]),
            links: Vec::new(),
        },
    }
}

fn invalid_mutation(tenant_id: Uuid, entity: u128, event: u128) -> IndexMutation {
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(event),
        record: IndexRecord {
            key: EntityKey {
                tenant_id,
                schema: schema_ref(),
                entity_id: Uuid::from_u128(entity),
                locale: None,
            },
            source_version: 1,
            fields: BTreeMap::new(),
            links: Vec::new(),
        },
    }
}

fn fixture_failure() -> IndexSourceFailure {
    IndexSourceFailure::permanent("failed_progress_recovery_fixture_invalid")
        .expect("fixture failure code must be valid")
}
