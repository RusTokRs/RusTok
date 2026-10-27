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
/// Bounds per-session chunk rows independently of payload bytes; otherwise
/// one-byte chunks could create tens of millions of database rows under the
/// staging-byte quota alone.
pub(crate) const MAX_ARTIFACT_OBJECT_UPLOAD_CHUNKS_PER_SESSION: u64 = 1_024;
pub(crate) const MAX_ARTIFACT_OBJECT_CONTENT_TYPE_BYTES: usize = 128;
pub(crate) const MAX_SANDBOX_ARTIFACT_OBJECT_BYTES: usize = 44 * 1024;
pub(crate) const MAX_ARTIFACT_OBJECT_GC_BATCH_SIZE: u32 = 100;
/// Bounds one platform tenant-discovery scan for object GC; the
/// maintenance worker repeats the scan each pass instead of paging.
pub(crate) const MAX_ARTIFACT_OBJECT_GC_TENANT_SCAN: u32 = 256;
/// Grace window between a data object becoming a GC candidate and its
/// bytes becoming retention-eligible (unless a durable hold retains the
/// whole namespace). The referencing row is already gone at queue time,
/// so this window is operator/undo grace, not a correctness delay.
pub(crate) const ARTIFACT_DATA_OBJECT_GC_RETENTION_WINDOW_SECONDS: i64 = 7 * 24 * 60 * 60;
/// A data-object GC claim is committed before the external byte delete. If
/// the worker disappears after that intent, another sweep may reclaim and
/// retry the idempotent delete only after this lease.
pub(crate) const ARTIFACT_OBJECT_GC_CLAIM_LEASE_SECONDS: i64 = 300;
