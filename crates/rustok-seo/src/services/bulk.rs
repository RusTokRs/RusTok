include!("bulk_legacy.rs");
include!("bulk_batch_execution.rs");

#[cfg(test)]
mod bulk_read_model {
    pub(super) use super::super::bulk_read_model::BulkReadProjection;
}
