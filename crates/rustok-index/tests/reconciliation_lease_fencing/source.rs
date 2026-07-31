use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use rustok_index::{
    EntityKey, FieldName, IndexMutation, IndexRecord, IndexSource, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue,
};
use tokio::sync::Barrier;
use uuid::Uuid;

use super::schema::schema_ref;

#[derive(Clone)]
pub enum LeaseSource {
    Blocking {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    },
    Immediate,
}

#[async_trait]
impl IndexSource for LeaseSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        if let Self::Blocking { entered, release } = self {
            entered.wait().await;
            release.wait().await;
        }
        IndexSourcePage::new(&request, vec![mutation(request.tenant_id())], None)
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
    let entity_id = Uuid::from_u128(300);
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(10_300),
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
