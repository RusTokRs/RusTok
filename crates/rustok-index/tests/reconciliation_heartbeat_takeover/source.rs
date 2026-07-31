use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use rustok_index::{
    EntityKey, FieldName, IndexMutation, IndexRecord, IndexSource, IndexSourceCursor,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue,
};
use serde_json::json;
use tokio::sync::Barrier;
use uuid::Uuid;

use super::schema::schema_ref;

#[derive(Clone)]
pub enum HeartbeatSource {
    Blocking {
        first_entered: Arc<Barrier>,
        first_release: Arc<Barrier>,
        second_entered: Arc<Barrier>,
        second_release: Arc<Barrier>,
    },
    Immediate,
}

#[async_trait]
impl IndexSource for HeartbeatSource {
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
                if let Self::Blocking {
                    first_entered,
                    first_release,
                    ..
                } = self
                {
                    first_entered.wait().await;
                    first_release.wait().await;
                }
                let cursor = IndexSourceCursor::new(json!({ "offset": 1 }))
                    .expect("fixture cursor must be valid");
                IndexSourcePage::new(
                    &request,
                    vec![mutation(request.tenant_id(), 401, 11_401)],
                    Some(cursor),
                )
                .map_err(|_| fixture_failure())
            }
            1 => {
                if let Self::Blocking {
                    second_entered,
                    second_release,
                    ..
                } = self
                {
                    second_entered.wait().await;
                    second_release.wait().await;
                }
                IndexSourcePage::new(
                    &request,
                    vec![mutation(request.tenant_id(), 402, 11_402)],
                    None,
                )
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

fn mutation(tenant_id: Uuid, entity: u128, event: u128) -> IndexMutation {
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

fn fixture_failure() -> IndexSourceFailure {
    IndexSourceFailure::permanent("heartbeat_takeover_fixture_invalid")
        .expect("fixture failure code must be valid")
}
