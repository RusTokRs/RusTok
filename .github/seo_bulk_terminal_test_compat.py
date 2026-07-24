from pathlib import Path

path = Path("crates/rustok-seo/src/services/events.rs")
text = path.read_text()

import_block = """use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseTransaction, DbErr, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
"""
if text.count(import_block) != 1:
    raise SystemExit("events test transaction import marker mismatch")
text = text.replace(
    import_block,
    import_block + "#[cfg(test)]\nuse sea_orm::TransactionTrait;\n",
    1,
)

marker = "    pub async fn index_delivery_status("
if text.count(marker) != 1:
    raise SystemExit("events index delivery marker mismatch")
wrapper = '''    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    async fn publish_seo_bulk_completed_event(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        target_kind: &str,
        locale: &str,
        status: &str,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
    ) {
        let txn = self
            .db
            .begin()
            .await
            .expect("bulk terminal test transaction should begin");
        self.publish_seo_bulk_terminal_event_in_tx(
            &txn,
            tenant_id,
            job_id,
            target_kind,
            locale,
            status,
            processed_count,
            succeeded_count,
            failed_count,
        )
        .await
        .expect("bulk terminal test event should enqueue transactionally");
        txn.commit()
            .await
            .expect("bulk terminal test transaction should commit");
    }

'''
path.write_text(text.replace(marker, wrapper + marker, 1))
