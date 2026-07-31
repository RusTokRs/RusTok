use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_index::{
    EntityKey, FieldName, IndexMutation, IndexRecord, IndexSource, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue,
};
use uuid::Uuid;

use super::schema::schema_ref;

pub const DUPLICATE_EVENT_ID: Uuid = Uuid::from_u128(21_001);

#[derive(Debug, Clone, Copy)]
pub enum EventIdContractSource {
    NilSecond,
    DuplicateSecond,
}

#[async_trait]
impl IndexSource for EventIdContractSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let second_event_id = match self {
            Self::NilSecond => Uuid::nil(),
            Self::DuplicateSecond => DUPLICATE_EVENT_ID,
        };
        IndexSourcePage::new(
            &request,
            vec![
                valid_mutation(request.tenant_id(), 1_001, DUPLICATE_EVENT_ID),
                valid_mutation(request.tenant_id(), 1_002, second_event_id),
            ],
            None,
        )
        .map_err(|_| fixture_failure())
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}

fn valid_mutation(tenant_id: Uuid, entity: u128, event_id: Uuid) -> IndexMutation {
    let entity_id = Uuid::from_u128(entity);
    IndexMutation::Upsert {
        event_id,
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
    IndexSourceFailure::permanent("event_id_contract_fixture_invalid")
        .expect("fixture failure code must be valid")
}
