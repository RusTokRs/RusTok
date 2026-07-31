use async_trait::async_trait;
use rustok_index::{
    IndexSource, IndexSourceCursor, IndexSourceFailure, IndexSourceLoadBatch,
    IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
};
use serde_json::json;

use super::schema::mutation;

pub struct ProcessRestartSource;

#[async_trait]
impl IndexSource for ProcessRestartSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let after = request
            .cursor()
            .and_then(|cursor| cursor.value().as_u64())
            .unwrap_or(0) as u128;
        let visible = [100_u128, 200_u128]
            .into_iter()
            .filter(|id| *id > after)
            .collect::<Vec<_>>();
        let selected = visible
            .iter()
            .copied()
            .take(request.limit())
            .collect::<Vec<_>>();
        let next_cursor = if visible.len() > selected.len() {
            selected.last().copied().map(|id| {
                IndexSourceCursor::new(json!(id as u64))
                    .expect("fixture cursor must remain bounded")
            })
        } else {
            None
        };
        let mutations = selected
            .into_iter()
            .map(|id| mutation(request.tenant_id(), id))
            .collect();
        IndexSourcePage::new(&request, mutations, next_cursor)
            .map_err(|_| IndexSourceFailure::permanent("fixture_page_invalid").unwrap())
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}
