use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use rustok_index::{
    EntityKey, FieldName, IndexMutation, IndexRecord, IndexSource, IndexSourceCursor,
    IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceScanRequest, IndexValue,
};
use serde_json::json;
use uuid::Uuid;

use super::schema::schema_ref;

pub const ENTITY_ID: Uuid = Uuid::from_u128(1_201);
pub const FRESH_MARKER_ID: Uuid = Uuid::from_u128(2_201);
pub const STALE_MARKER_ID: Uuid = Uuid::from_u128(2_202);
pub const FRESH_UPSERT_EVENT_ID: Uuid = Uuid::from_u128(17_201);
pub const STALE_DELETE_EVENT_ID: Uuid = Uuid::from_u128(17_202);
pub const FRESH_DELETE_EVENT_ID: Uuid = Uuid::from_u128(17_203);
pub const STALE_UPSERT_EVENT_ID: Uuid = Uuid::from_u128(17_204);

#[derive(Clone)]
pub struct StaleVersionSource {
    calls: Arc<AtomicUsize>,
}

impl StaleVersionSource {
    pub fn new(calls: Arc<AtomicUsize>) -> Self {
        Self { calls }
    }
}

#[async_trait]
impl IndexSource for StaleVersionSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let offset = request
            .cursor()
            .and_then(|cursor| cursor.value().get("offset"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        let (mutation, next_offset) = match offset {
            0 => (fresh_upsert(request.tenant_id()), Some(1)),
            1 => (stale_delete(request.tenant_id()), Some(2)),
            2 => (fresh_delete(request.tenant_id()), Some(3)),
            3 => (stale_upsert(request.tenant_id()), None),
            _ => return Err(fixture_failure()),
        };
        let next_cursor = next_offset
            .map(|offset| {
                IndexSourceCursor::new(json!({ "offset": offset }))
                    .expect("fixture cursor must be valid")
            });
        IndexSourcePage::new(&request, vec![mutation], next_cursor)
            .map_err(|_| fixture_failure())
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}

pub fn fresh_fields() -> BTreeMap<FieldName, IndexValue> {
    fields(FRESH_MARKER_ID)
}

fn fields(marker_id: Uuid) -> BTreeMap<FieldName, IndexValue> {
    BTreeMap::from([
        (FieldName::new("id").unwrap(), IndexValue::Uuid(ENTITY_ID)),
        (
            FieldName::new("marker_id").unwrap(),
            IndexValue::Uuid(marker_id),
        ),
    ])
}

fn key(tenant_id: Uuid) -> EntityKey {
    EntityKey {
        tenant_id,
        schema: schema_ref(),
        entity_id: ENTITY_ID,
        locale: None,
    }
}

fn upsert(tenant_id: Uuid, event_id: Uuid, source_version: u64, marker_id: Uuid) -> IndexMutation {
    IndexMutation::Upsert {
        event_id,
        record: IndexRecord {
            key: key(tenant_id),
            source_version,
            fields: fields(marker_id),
            links: Vec::new(),
        },
    }
}

fn fresh_upsert(tenant_id: Uuid) -> IndexMutation {
    upsert(
        tenant_id,
        FRESH_UPSERT_EVENT_ID,
        3,
        FRESH_MARKER_ID,
    )
}

fn stale_delete(tenant_id: Uuid) -> IndexMutation {
    IndexMutation::Delete {
        event_id: STALE_DELETE_EVENT_ID,
        key: key(tenant_id),
        source_version: 2,
    }
}

fn fresh_delete(tenant_id: Uuid) -> IndexMutation {
    IndexMutation::Delete {
        event_id: FRESH_DELETE_EVENT_ID,
        key: key(tenant_id),
        source_version: 4,
    }
}

fn stale_upsert(tenant_id: Uuid) -> IndexMutation {
    upsert(
        tenant_id,
        STALE_UPSERT_EVENT_ID,
        3,
        STALE_MARKER_ID,
    )
}

fn fixture_failure() -> IndexSourceFailure {
    IndexSourceFailure::permanent("stale_version_guard_fixture_invalid")
        .expect("fixture failure code must be valid")
}
