include!("bulk_legacy.rs");
include!("bulk_bounded_execution.rs");
include!("bulk_io_bounded_execution.rs");
include!("bulk_io_bounded_compat.rs");
include!("sitemap_background.rs");

#[cfg(test)]
mod bulk_read_model {
    pub(super) use super::super::bulk_read_model::BulkReadProjection;
}
