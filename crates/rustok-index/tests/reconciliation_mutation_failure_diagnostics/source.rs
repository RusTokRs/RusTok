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
pub enum MutationFailureSource {
    InvalidRecord,
    BlockingValid {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    },
}

#[async_trait]
impl IndexSource for MutationFailureSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let mutation = match self {
            Self::InvalidRecord => invalid_mutation(request.tenant_id()),
            Self::BlockingValid { entered, release } => {
                entered.wait().await;
                release.wait().await;
                valid_mutation(request.tenant_id())
            }
        };
        IndexSourcePage::new(&request, vec![mutation], None).map_err(|_| fixture_failure())
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}

fn invalid_mutation(tenant_id: Uuid) -> IndexMutation {
    let entity_id = Uuid::from_u128(601);
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(13_601),
        record: IndexRecord {
            key: EntityKey {
                tenant_id,
                schema: schema_ref(),
                entity_id,
                locale: None,
            },
            source_version: 1,
            fields: BTreeMap::new(),
            links: Vec::new(),
        },
    }
}

fn valid_mutation(tenant_id: Uuid) -> IndexMutation {
    let entity_id = Uuid::from_u128(602);
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(13_602),
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
    IndexSourceFailure::permanent("mutation_failure_fixture_invalid")
        .expect("fixture failure code must be valid")
}
