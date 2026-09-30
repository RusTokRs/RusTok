//! Artifact data limits and configuration constants.

pub(crate) const MAX_ARTIFACT_DATA_KEY_BYTES: usize = 256;
pub(crate) const MAX_ARTIFACT_DATA_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_ARTIFACT_DATA_PAGE_SIZE: u32 = 100;
pub(crate) const MAX_ARTIFACT_DATA_BATCH_SIZE: usize = 32;
pub(crate) const MAX_ARTIFACT_DATA_INDEX_VALUE_BYTES: usize = 256;
pub(crate) const MAX_ARTIFACT_OBJECT_BYTES: u64 = 32 * 1024 * 1024;
pub(crate) const MAX_ARTIFACT_DATA_NAMESPACE_RECORDS: u64 = 10_000;
pub(crate) const MAX_ARTIFACT_DATA_NAMESPACE_VALUE_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_ARTIFACT_DATA_NAMESPACE_OBJECTS: u64 = 1_024;
pub(crate) const MAX_ARTIFACT_DATA_NAMESPACE_OBJECT_BYTES: u64 = 256 * 1024 * 1024;
pub(crate) const MAX_ARTIFACT_DATA_NAMESPACE_UPLOAD_SESSIONS: u64 = 16;
pub(crate) const MAX_ARTIFACT_DATA_NAMESPACE_STAGING_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_ARTIFACT_OBJECT_CONTENT_TYPE_BYTES: usize = 128;
pub(crate) const MAX_SANDBOX_ARTIFACT_OBJECT_BYTES: usize = 44 * 1024;
pub(crate) const MAX_ARTIFACT_OBJECT_GC_BATCH_SIZE: u32 = 100;
