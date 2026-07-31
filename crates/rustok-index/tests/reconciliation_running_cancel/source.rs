use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use rustok_index::{
    EntityKey, FieldName, IndexMutation, IndexRecord, IndexSource, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue,
};
use tokio::sync::Barrier;
use uuid::Uuid;

use super::fixture::schema_ref;

#[derive(Clone)]
pub struct BlockingSource {
    pub calls: Arc<AtomicUsize>,
    pub block_first: Arc<AtomicBool>,
    pub entered: Arc<Barrier>,
    pub release: Arc<Barrier>,
}

#[async_trait]
impl IndexSource for BlockingSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.block_first.swap(false, Ordering::SeqCst) {
            self.entered.wait().await;
            self.release.wait().await;
        }
        let mutation = mutation(request.tenant_id());
        IndexSourcePage::new(&request, vec![mutation], None)
            .map_err(|_| IndexSourceFailure::permanent("fixture_page_invalid").unwrap())
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}

fn mutation(tenant_id: Uuid) -> IndexMutation {
    let entity_id = Uuid::from_u128(100);
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(10_100),
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
